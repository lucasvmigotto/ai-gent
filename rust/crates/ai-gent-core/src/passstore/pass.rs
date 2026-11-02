//! The `pass` backend: `pass show`, `pass insert --multiline`.
use super::{Backend, Config, StoreError};
use crate::exec;
use std::process::Command;

/// Run `pass show ENTRY` and return its stdout lines, or the reason it failed.
pub(super) fn show(cfg: &Config, backend: &Backend, entry: &str) -> Result<Vec<String>, String> {
    let mut cmd = Command::new(&backend.path);
    cmd.args(backend.show_args(entry));
    let out = exec::run(cmd, &cfg.child_env(), None, cfg.read_timeout);
    let out = match out {
        Ok(o) => o,
        Err(e) => return Err(format!("cannot run {}: {}", backend.name, e.kind())),
    };
    if out.timed_out {
        return Err("timed out waiting for the passphrase".to_string());
    }
    if out.code != 0 {
        let err = String::from_utf8_lossy(&out.stderr);
        let last = err.trim().lines().last().unwrap_or("locked or missing");
        return Err(last.chars().take(200).collect());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

/// Run the backend's insert with `value` on stdin.
pub(super) fn insert(
    cfg: &Config,
    backend: &Backend,
    entry: &str,
    value: &str,
) -> Result<(), StoreError> {
    let mut cmd = Command::new(&backend.path);
    cmd.args(backend.insert_args(entry));
    let out = exec::run(
        cmd,
        &cfg.child_env(),
        Some(format!("{value}\n").as_bytes()),
        cfg.write_timeout,
    )
    .map_err(|e| {
        StoreError::new(format!(
            "{} insert {entry} failed: {}",
            backend.name,
            e.kind()
        ))
    })?;
    if out.code != 0 || !super::has_with(cfg, entry) {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        let tail: String = {
            let n = err.chars().count();
            err.chars().skip(n.saturating_sub(300)).collect()
        };
        return Err(StoreError::new(format!(
            "{} insert {entry} failed: {tail}",
            backend.name
        )));
    }
    Ok(())
}
