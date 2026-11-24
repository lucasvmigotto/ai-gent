//! Registered execution identities (046 US1). This module does not collect tokens or accept tasks.
mod contributions;
mod durable;
mod identity;
mod journal;
mod normalize;
mod observations;
mod receipts;
mod recovery;
mod transfer;

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
    observations::replay(root)
}

#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod tests;

/// Normalize and durably record a source observation for an existing execution scope.
///
/// # Errors
/// Rejects scope conflicts, inconsistent counters, ambiguous coverage, corrupt journals,
/// unsupported attestation claims and bounded I/O failures. Never certifies a provider bill.
pub fn ingest(root: &Path, input: &[u8]) -> Result<Value> {
    if input.len() > MAX_RECORD {
        return Err(limit());
    }
    let request: normalize::IngestRequest = observations::parse(input)?;
    if request.schema_version != 1 {
        return Err(Error::new(
            "unsupported_version",
            "only usage ingest version 1 is supported",
            2,
        ));
    }
    if request.request_id.is_nil()
        || request.repository_id.is_nil()
        || request.operation != "ingest"
    {
        return Err(invalid());
    }
    observations::ingest(root, &request)
}

/// Publish an immutable local usage snapshot after flushing its journal evidence.
///
/// # Errors
/// Rejects stale sequences/predecessors, conflicting requests, corruption and I/O failure.
/// This snapshot never accepts a task or certifies a provider bill.
pub fn snapshot(root: &Path, input: &[u8]) -> Result<Value> {
    receipts::snapshot(root, input)
}

/// Plan or explicitly apply digest-pinned isolation of incomplete trailing journal frames.
///
/// # Errors
/// Refuses interior corruption, changed evidence, missing registered prefixes and I/O failure.
/// Dry runs create no files; apply preserves original damaged evidence before truncation.
pub fn recover(root: &Path, input: Option<&[u8]>, dry_run: bool) -> Result<Value> {
    recovery::recover(root, input, dry_run)
}

/// Export a sanitized portable evidence bundle from validated local journals.
///
/// # Errors
/// Rejects corruption, missing local evidence, unsafe paths and bounded I/O failures.
pub fn export(root: &Path) -> Result<Value> {
    transfer::export(root)
}

/// Validate or archive portable evidence without contributing to local measured totals.
///
/// # Errors
/// Rejects unsupported formats, contradictory evidence, conflicts, corruption and I/O failures.
pub fn import(root: &Path, input: &[u8], dry_run: bool) -> Result<Value> {
    transfer::import(root, input, dry_run)
}

#[cfg(all(test, unix))]
mod process_crash_tests;
