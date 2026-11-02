//! Token entries: names under the prefix, notes, and the read that registers the secret for redaction.
use super::platforms::platform;
use super::{Ctx, Failed, R};
use crate::passstore::{self, Secret};
use std::collections::BTreeMap;

/// `ai-gent/<platform entry>` under the configured prefix.
#[must_use]
pub fn default_entry(ctx: &Ctx, plat: &str) -> String {
    let p = platform(plat).map_or("", |p| p.entry);
    format!("{}/{p}", ctx.prefix())
}

/// A platform token entry: under the prefix, never a `db/` entry, which is dbrun's.
///
/// # Errors
/// An invalid name (exit 1) or one outside the platform entries (refused).
pub fn check_entry(ctx: &Ctx, entry: &str) -> R<String> {
    let name = passstore::check(entry).map_err(|e| Failed::err(e.to_string()))?;
    let prefix = ctx.prefix();
    if !name.starts_with(&format!("{prefix}/")) || name.starts_with(&format!("{prefix}/db/")) {
        return Err(Failed::refused(format!(
            "{entry} isn't a platform token entry: use one under {prefix}/<platform>/ (db/ is dbrun's)"
        )));
    }
    Ok(entry.to_string())
}

/// Whether the store holds the entry: a file check, nothing decrypted.
#[must_use]
pub fn pass_has(ctx: &Ctx, entry: &str) -> bool {
    passstore::has(&ctx.pass, entry).unwrap_or(false)
}

fn note_key(k: &str) -> bool {
    let mut c = k.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic())
        && c.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

/// The secret (first line) and the `key: value` notes of the later lines.
///
/// # Errors
/// When the entry is not a platform entry or the read fails.
pub fn read_entry(ctx: &Ctx, entry: &str) -> R<(Secret, BTreeMap<String, String>)> {
    check_entry(ctx, entry)?;
    let (secret, rest) = passstore::read(&ctx.pass, entry)
        .map_err(|e| Failed::err(format!("{e}; see secrets:pass")))?;
    let mut notes = BTreeMap::new();
    for line in rest {
        if let Some((k, v)) = line.split_once(':') {
            if note_key(k.trim()) {
                notes.insert(k.trim().to_lowercase(), v.trim().to_string());
            }
        }
    }
    Ok((secret, notes))
}
