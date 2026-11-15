//! Documented, stable exit codes (`specs/032-rust-workspace-and-release/contracts/cli.md`).

/// Exit code of an `ai-gent` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// The operation succeeded.
    Success,
    /// The operation failed at run time.
    Runtime,
    /// Bad arguments (clap's default).
    Usage,
    /// Nothing to do, or not applicable.
    NothingToDo,
    /// Refused by policy or verification (a checksum, an attestation, a guard hard stop).
    Refused,
    /// Internal error (a panic).
    Internal,
}

impl Exit {
    /// The process exit status.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Runtime => 1,
            Self::Usage => 2,
            Self::NothingToDo => 3,
            Self::Refused => 4,
            Self::Internal => 101,
        }
    }
}
