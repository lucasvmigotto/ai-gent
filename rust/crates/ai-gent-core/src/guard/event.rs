//! The hook event, the decision and the guard ids.
use serde_json::Value;

/// The five guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardId {
    /// `git:workflow`.
    Git,
    /// Database access and credentials files.
    Db,
    /// Secret values in pass.
    Secrets,
    /// Changes to a platform through reporun.
    Repo,
    /// Container resource removal.
    Containers,
}

impl GuardId {
    /// Every guard, in the order `evaluate_all` consults them.
    pub const ALL: [Self; 5] = [
        Self::Git,
        Self::Db,
        Self::Secrets,
        Self::Repo,
        Self::Containers,
    ];

    /// The id used on the command line, in fixtures and in reasons.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Db => "db",
            Self::Secrets => "secrets",
            Self::Repo => "repo",
            Self::Containers => "containers",
        }
    }

    /// Parse a guard id.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.name() == s)
    }
}

/// How a rule decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// No decision: the normal permission flow continues.
    None,
    /// Ask the user.
    Ask,
    /// Refuse.
    Deny,
}

impl Level {
    /// `deny`, `ask` or `none`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }
}

/// A guard's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// deny, ask or none.
    pub level: Level,
    /// Marker plus text, byte-identical to the shell guards; empty for `none`.
    pub reason: String,
    /// Reason id (`git.push`), empty for `none`.
    pub rule: &'static str,
}

impl Decision {
    /// No decision.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            level: Level::None,
            reason: String::new(),
            rule: "",
        }
    }

    /// The hook JSON line, or `None` when there is nothing to print.
    #[must_use]
    pub fn hook_line(&self) -> Option<String> {
        if self.level == Level::None {
            return None;
        }
        Some(
            serde_json::json!({"hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": self.level.name(),
                "permissionDecisionReason": self.reason,
            }})
            .to_string(),
        )
    }
}

/// A `PreToolUse` event.
#[derive(Debug, Clone)]
pub struct Event {
    /// `Bash`, `PowerShell`, `Read`, ...
    pub tool_name: String,
    /// The session's working directory.
    pub cwd: String,
    /// The tool's input object.
    pub tool_input: Value,
}

impl Event {
    /// Parse the hook JSON.
    ///
    /// # Errors
    /// When the text is not a JSON object.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let v: Value =
            serde_json::from_str(text).map_err(|e| format!("invalid event JSON: {e}"))?;
        if !v.is_object() {
            return Err("the event is not a JSON object".into());
        }
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Ok(Self {
            tool_name: s("tool_name"),
            cwd: s("cwd"),
            tool_input: v.get("tool_input").cloned().unwrap_or(Value::Null),
        })
    }

    /// Whether the tool runs a shell command (`Bash` or `PowerShell`).
    #[must_use]
    pub fn is_shell(&self) -> bool {
        self.tool_name == "Bash" || self.tool_name == "PowerShell"
    }

    /// The shell command, when there is a non-empty one.
    #[must_use]
    pub fn command(&self) -> Option<&str> {
        self.tool_input
            .get("command")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
    }
}
