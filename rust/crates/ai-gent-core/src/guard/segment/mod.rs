//! Per-shell segmentation: how a command line becomes simple commands.
pub mod sh;
pub mod win;

/// The shell syntax a command is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    /// POSIX `sh` (and bash, zsh under Git Bash).
    Sh,
    /// Windows PowerShell and `pwsh`.
    PowerShell,
    /// `cmd.exe`.
    Cmd,
}

impl Shell {
    /// Parse `sh`, `powershell` or `cmd`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "sh" => Some(Self::Sh),
            "powershell" => Some(Self::PowerShell),
            "cmd" => Some(Self::Cmd),
            _ => None,
        }
    }

    /// The name used in fixtures and the shadow log.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sh => "sh",
            Self::PowerShell => "powershell",
            Self::Cmd => "cmd",
        }
    }
}

/// A command prepared for the rules.
#[derive(Debug, Clone)]
pub struct View {
    /// The syntax the command was read in.
    pub shell: Shell,
    /// The text command-wide patterns scan (JSON-escaped for `sh`, as the shell guards see it).
    pub raw: String,
    /// One simple command per element, quoted text blanked.
    pub segments: Vec<String>,
    /// The raw text with quoted prose blanked (the db credentials word scan).
    pub words: String,
}

/// Prepare `cmd` for `shell`. `containers` selects the containers guard's variant of the pipeline.
#[must_use]
pub fn view(shell: Shell, cmd: &str, containers: bool) -> View {
    match shell {
        Shell::Sh => {
            let raw = sh::escape(cmd);
            View {
                shell,
                segments: sh::segments(&raw, containers),
                words: sh::blank_prose(&raw),
                raw,
            }
        }
        Shell::PowerShell | Shell::Cmd => {
            let scanned = win::scan(shell, cmd, containers);
            View {
                shell,
                raw: cmd.to_string(),
                segments: scanned.segments,
                words: scanned.words,
            }
        }
    }
}
