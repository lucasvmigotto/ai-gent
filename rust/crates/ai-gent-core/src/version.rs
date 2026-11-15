//! Version information: crate version, release, target triple, commit, profile.
use serde::Serialize;

/// Target triple this library was built for.
pub const TARGET: &str = env!("AI_GENT_TARGET");

/// Output of `ai-gent version --json` (`data`).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionInfo {
    /// Version of the binary crate.
    #[serde(rename = "crate")]
    pub crate_version: String,
    /// Release version (`AI_GENT_RELEASE` at build time), `dev` outside the release workflow.
    pub release: String,
    /// Target triple.
    pub target: String,
    /// Short commit, `null` when unavailable.
    pub commit: Option<String>,
    /// `release` or `debug`.
    pub profile: &'static str,
}

impl VersionInfo {
    /// Information for the binary crate whose version is `crate_version`.
    #[must_use]
    pub fn current(crate_version: &str) -> Self {
        let commit = option_env!("AI_GENT_COMMIT")
            .filter(|c| !c.is_empty())
            .map(str::to_string);
        Self {
            crate_version: crate_version.to_string(),
            release: option_env!("AI_GENT_RELEASE").unwrap_or("dev").to_string(),
            target: TARGET.to_string(),
            commit,
            profile: if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
        }
    }

    /// The `--version` line: `ai-gent <X.Y.Z> (<triple>)`.
    #[must_use]
    pub fn line(&self) -> String {
        format!("ai-gent {} ({})", self.crate_version, self.target)
    }
}
