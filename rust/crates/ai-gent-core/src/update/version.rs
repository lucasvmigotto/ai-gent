//! Version comparison identical to `_ai_gent_ver_gt` in `scripts/update-check.sh`.
use std::cmp::Ordering;

/// Whether `s` is `X.Y.Z` with digits only (what `install.sh` accepts and release tags are).
#[must_use]
pub fn is_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Three numeric fields: a leading `v` is dropped, missing parts are 0, a non-numeric suffix on a
/// part is ignored, a fourth part is ignored.
fn fields(v: &str) -> [u64; 3] {
    let v = v.strip_prefix('v').unwrap_or(v);
    let mut out = [0; 3];
    for (slot, part) in out.iter_mut().zip(v.split('.')) {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        *slot = digits.parse().unwrap_or(0);
    }
    out
}

/// Order two versions.
#[must_use]
pub fn compare(a: &str, b: &str) -> Ordering {
    fields(a).cmp(&fields(b))
}

/// Whether `a` is newer than `b`.
#[must_use]
pub fn ver_gt(a: &str, b: &str) -> bool {
    compare(a, b) == Ordering::Greater
}

#[cfg(test)]
mod tests {
    use super::*;

    // The cases of `scripts/test-update-check.sh` "version comparison orders triples".
    #[test]
    fn orders_triples_like_the_shell() {
        assert!(ver_gt("1.3.0", "1.2.0"));
        assert!(ver_gt("1.10.0", "1.9.9"));
        assert!(ver_gt("v2.0.0", "1.9.9"));
        assert!(!ver_gt("1.2.0", "1.2.0"));
        assert!(!ver_gt("1.2.0", "1.3.0"));
        assert!(!ver_gt("1.2", "1.2.0"));
        assert!(ver_gt("1.2.1", "1.2"));
    }

    #[test]
    fn suffixes_and_empty_parts_are_zero_or_ignored() {
        assert!(!ver_gt("1.2.0-rc1", "1.2.0"));
        assert!(ver_gt("1.2.1-rc1", "1.2.0"));
        assert!(!ver_gt("", "0.0.0"));
        assert!(!ver_gt("1.2.3.4", "1.2.3"));
        assert!(ver_gt("1.0.0", "0.99.99"));
    }

    #[test]
    fn release_versions() {
        assert!(is_version("2.28.0"));
        assert!(!is_version("2.28"));
        assert!(!is_version("v2.28.0"));
        assert!(!is_version("2.28.0.1"));
        assert!(!is_version("2..0"));
        assert!(!is_version("a.b.c"));
    }
}
