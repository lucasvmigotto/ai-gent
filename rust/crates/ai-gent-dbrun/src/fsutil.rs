//! Bounded file reads for configuration the runner takes from disk (profiles files, CA files).
use std::io::Read;
use std::path::Path;

/// Reads a regular file of at most `cap` bytes as UTF-8.
///
/// Refused, with a message that names the file but never its content: a path that is not a regular
/// file (a device or a FIFO could block or never end), a file larger than `cap`, bytes that are not UTF-8,
/// and a NUL byte.
///
/// # Errors
/// The refusal reason.
pub(crate) fn read_text_bounded(path: &Path, cap: u64) -> Result<String, String> {
    let name = path.file_name().map_or_else(
        || "the file".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let bytes = read_bytes_bounded(path, cap)?;
    let text = String::from_utf8(bytes).map_err(|_| format!("{name} is not valid UTF-8"))?;
    if text.contains('\0') {
        return Err(format!("{name} holds a NUL byte"));
    }
    Ok(text)
}

/// Like [`read_text_bounded`], without the UTF-8 and NUL checks.
///
/// # Errors
/// The refusal reason.
pub(crate) fn read_bytes_bounded(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let name = path.file_name().map_or_else(
        || "the file".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    // Open first (without blocking on a FIFO that someone swapped in), then ask the open file: the check
    // and the read are about the same file.
    let file = open_nonblocking(path).map_err(|e| format!("cannot read {name}: {}", e.kind()))?;
    let meta = file
        .metadata()
        .map_err(|e| format!("cannot read {name}: {}", e.kind()))?;
    if !meta.is_file() {
        return Err(format!("{name} is not a regular file"));
    }
    if meta.len() > cap {
        return Err(format!("{name} is larger than {cap} bytes"));
    }
    let mut buf = Vec::new();
    file.take(cap + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("cannot read {name}: {}", e.kind()))?;
    if buf.len() as u64 > cap {
        return Err(format!("{name} is larger than {cap} bytes"));
    }
    Ok(buf)
}

#[cfg(unix)]
fn open_nonblocking(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_nonblocking(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}
