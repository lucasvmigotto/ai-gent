//! Bounded filesystem primitives shared by snapshot publication and explicit recovery.
use super::journal::{Store, check_path, options, sync_dir};
use super::{MAX_RECORD, Result, corrupt, io_error, limit};
use std::io::{Read, Write};
use std::path::Path;

pub(super) const MAX_LOG: u64 = 64 * 1024 * 1024;

pub(super) fn read(root: &Path, path: &Path, maximum: u64) -> Result<Option<Vec<u8>>> {
    check_path(root, path, false)?;
    let file = match options().read(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(error)),
    };
    if !file.metadata().map_err(io_error)?.is_file() {
        return Err(corrupt());
    }
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > maximum {
        return Err(limit());
    }
    Ok(Some(bytes))
}

/// Atomically create a new immutable name; an existing name must contain identical bytes.
/// Hard-link publication provides no-clobber semantics; unsupported filesystems fail closed.
pub(super) fn publish(store: &Store, path: &Path, bytes: &[u8]) -> Result<()> {
    check_path(&store.root, path, false)?;
    let parent = path.parent().ok_or_else(super::invalid)?;
    if let Some(existing) = read(&store.root, path, MAX_LOG)? {
        if existing != bytes {
            return Err(corrupt());
        }
        options()
            .write(true)
            .open(path)
            .and_then(|file| file.sync_all())
            .map_err(io_error)?;
        return sync_dir(parent);
    }
    let temporary = parent.join(format!(".pending-{}", uuid::Uuid::new_v4()));
    let mut file = options()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(io_error)?;
    boundary("publication_write")?;
    file.write_all(bytes).map_err(io_error)?;
    boundary("publication_flush")?;
    file.sync_all().map_err(io_error)?;
    boundary("publication_link")?;
    match std::fs::hard_link(&temporary, path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if read(&store.root, path, MAX_LOG)?.as_deref() != Some(bytes) {
                return Err(corrupt());
            }
        }
        Err(error) => return Err(io_error(error)),
    }
    boundary("publication_directory_sync")?;
    sync_dir(parent)?;
    std::fs::remove_file(temporary).map_err(io_error)?;
    sync_dir(parent)
}

pub(super) fn append(file: &mut std::fs::File, bytes: &[u8]) -> Result<()> {
    #[cfg(test)]
    if fault_is("append_partial") || checkpoint_matches("append_partial") {
        file.write_all(&bytes[..bytes.len() / 2])
            .map_err(io_error)?;
        boundary("append_partial")?;
    }
    boundary("append_write")?;
    file.write_all(bytes).map_err(io_error)?;
    boundary("append_flush")?;
    file.sync_all().map_err(io_error)
}

#[cfg_attr(
    not(test),
    allow(
        clippy::unnecessary_wraps,
        reason = "test-only failures share the production filesystem boundary signature"
    )
)]
pub(super) fn boundary(name: &'static str) -> Result<()> {
    #[cfg(test)]
    {
        wait_checkpoint(name)?;
        if fault_is(name) {
            FAULT.with(|fault| *fault.borrow_mut() = None);
            return Err(io_error(std::io::Error::other(
                "synthetic I/O boundary failure",
            )));
        }
    }
    let _ = name;
    Ok(())
}
#[cfg(test)]
thread_local! { static FAULT: std::cell::RefCell<Option<&'static str>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
fn fault_is(name: &str) -> bool {
    FAULT.with(|fault| *fault.borrow() == Some(name))
}
#[cfg(test)]
pub(super) fn fail_once(name: &'static str) {
    FAULT.with(|fault| *fault.borrow_mut() = Some(name));
}

pub(super) fn record_limit(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_RECORD {
        return Err(limit());
    }
    Ok(())
}

// This control exists only in the unit-test binary, never in a runtime CLI.
#[cfg(test)]
thread_local! {
    static CHECKPOINT: std::cell::RefCell<Option<(String, std::path::PathBuf)>> = const { std::cell::RefCell::new(None) };
}
#[cfg(all(test, unix))]
pub(super) fn checkpoint_once(name: &str, marker: std::path::PathBuf) {
    CHECKPOINT.with(|checkpoint| *checkpoint.borrow_mut() = Some((name.into(), marker)));
}
#[cfg(test)]
fn checkpoint_matches(name: &str) -> bool {
    CHECKPOINT.with(|checkpoint| {
        checkpoint
            .borrow()
            .as_ref()
            .is_some_and(|(expected, _)| expected == name)
    })
}
#[cfg(test)]
fn wait_checkpoint(name: &str) -> Result<()> {
    let marker = CHECKPOINT.with(|checkpoint| {
        checkpoint
            .borrow()
            .as_ref()
            .filter(|(expected, _)| expected == name)
            .map(|(_, marker)| marker.clone())
    });
    let Some(marker) = marker else {
        return Ok(());
    };
    let mut file = std::fs::File::create_new(marker).map_err(io_error)?;
    file.write_all(name.as_bytes()).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    // Parent waits for this exact marker, then kills the actual child process.
    // If its parent disappears, a bounded fallback prevents an orphan fixture.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    Err(io_error(std::io::Error::other(
        "synthetic checkpoint parent disappeared",
    )))
}
