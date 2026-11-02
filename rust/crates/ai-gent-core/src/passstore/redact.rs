//! The secret-holding type and the process-wide redaction registry (spec 037 FR-A3).
//!
//! Every string that leaves the process (stdout, stderr, logs, errors, JSON) goes through
//! [`redact`] after any secret was read. [`Secret`] never prints its value, not even in `Debug`.
use std::sync::Mutex;

static REGISTRY: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A secret value. `Debug` and `Display` print a placeholder; only [`Secret::expose`] gives the text.
// Only Clone is derived on purpose: a test reads this file and fails if a Debug derive is added.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// Wrap a value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value. Call only where it goes to its one sink (a child's environment, a header).
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Make [`redact`] hide this value from now on.
    pub fn register(&self) {
        register(&self.0);
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<token>")
    }
}

/// Remember a value (and the Basic-auth form of it) so [`redact`] hides it. Values under
/// four characters are ignored, as in the Python runners.
pub fn register(value: &str) {
    if value.len() < 4 {
        return;
    }
    let mut r = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for v in [
        value.to_string(),
        crate::b64::encode(format!(":{value}").as_bytes()),
    ] {
        if !r.contains(&v) {
            r.push(v);
        }
    }
}

/// Remember a derived header value (for example `base64(user:secret)`) verbatim.
pub fn register_derived(value: &str) {
    if value.len() < 4 {
        return;
    }
    let mut r = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !r.iter().any(|v| v == value) {
        r.push(value.to_string());
    }
}

/// Replace every registered value in `text` with `<token>`.
#[must_use]
pub fn redact(text: &str) -> String {
    let r = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut out = text.to_string();
    // Longest first, so a value that contains another is hidden whole.
    let mut vals: Vec<&String> = r.iter().collect();
    vals.sort_by_key(|v| std::cmp::Reverse(v.len()));
    for v in vals {
        out = out.replace(v.as_str(), "<token>");
    }
    out
}

/// Forget everything (tests).
pub fn clear() {
    REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_and_display_hide_the_value() {
        let s = Secret::new("tok-3f9a1c77e0");
        assert!(!format!("{s:?}").contains("3f9a"));
        assert!(!format!("{s}").contains("3f9a"));
        assert_eq!(s.expose(), "tok-3f9a1c77e0");
    }

    #[test]
    fn secret_never_derives_debug() {
        let src = include_str!("redact.rs");
        let pos = src.find("pub struct Secret").expect("struct present");
        let before = &src[..pos];
        let attr_block = before.rsplit("\n\n").next().unwrap_or("");
        assert!(
            !attr_block.contains("derive(")
                || !attr_block.split("derive(").skip(1).any(|d| d
                    .split(')')
                    .next()
                    .unwrap_or("")
                    .contains("Debug")),
            "Secret must not derive Debug"
        );
    }

    #[test]
    fn redacts_value_and_basic_form() {
        // The registry is process-wide: use a value no other test registers.
        register("canary-redact-test-value");
        let basic = crate::b64::encode(b":canary-redact-test-value");
        let text = format!("a canary-redact-test-value b {basic} c");
        assert_eq!(redact(&text), "a <token> b <token> c");
        assert_eq!(redact("short"), "short");
    }
}
