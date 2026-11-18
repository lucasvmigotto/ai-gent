//! Registered execution identities (046 US1). This module does not collect tokens or accept tasks.
mod identity;
mod journal;

use serde::Serialize;
use serde_json::{Value, json};
use std::path::Path;

/// Largest accepted request or journal frame, in bytes.
pub const MAX_RECORD: usize = 1_048_576;

/// Sanitized protocol error; carries no request text or private filesystem path.
#[derive(Debug, thiserror::Error, Serialize)]
#[error("{message}")]
pub struct Error {
    /// Stable v1 error code.
    pub code: &'static str,
    /// Whether retrying the same request can help.
    pub retryable: bool,
    /// Safe diagnostic.
    pub message: &'static str,
    #[serde(skip)]
    exit: u8,
}
impl Error {
    pub(super) const fn new(code: &'static str, message: &'static str, exit: u8) -> Self {
        Self {
            code,
            retryable: false,
            message,
            exit,
        }
    }
    /// Protocol v1 exit status, independent of legacy CLI exit semantics.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.exit
    }
}
type Result<T> = std::result::Result<T, Error>;
fn invalid() -> Error {
    Error::new("invalid_schema", "invalid registration request", 2)
}
fn conflict() -> Error {
    Error::new(
        "identity_conflict",
        "identity already belongs to different content or scope",
        4,
    )
}
fn corrupt() -> Error {
    Error::new(
        "evidence_corrupt",
        "registration journal is invalid; preserve it for recovery",
        5,
    )
}
fn io_error(_: std::io::Error) -> Error {
    Error::new("io_failure", "registration storage operation failed", 5)
}
fn limit() -> Error {
    Error::new(
        "limit_exceeded",
        "registration input or storage limit exceeded",
        2,
    )
}

/// Apply a v1 registration, segment switch or execution transition durably and idempotently.
///
/// # Errors
/// Rejects malformed/oversized requests, unknown tasks/parents, scope or generation conflicts,
/// unsafe paths, corrupt storage and I/O failures. Does not authorize a worker or certify usage.
pub fn execute(root: &Path, operation: &str, input: &[u8]) -> Result<Value> {
    if input.len() > MAX_RECORD {
        return Err(limit());
    }
    let request: identity::Request = serde_json::from_slice(input).map_err(|_| invalid())?;
    request.validate(operation)?;
    let record = journal::execute(root, &request)?;
    Ok(json!({"schema_version":1,"operation":operation,"ok":true,
        "generation":record.generation,"result":record,"warnings":[]}))
}

/// Read and verify the registration journal. Never interprets execution state as task acceptance.
///
/// # Errors
/// Rejects unsafe paths, invalid frames, integrity/scope violations, lock timeout or I/O failure.
pub fn replay(root: &Path) -> Result<Value> {
    let state = journal::replay(root)?;
    Ok(json!({"schema_version":1,"operation":"replay","ok":true,
        "result":{"repository_id":state.repository_id,"sequence":state.sequence,
        "runs":state.runs.values().collect::<Vec<_>>()},"warnings":[]}))
}

#[cfg(test)]
mod tests;
