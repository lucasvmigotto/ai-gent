//! Source verification (spec 041, T012a, trial 1 finding 1): a delegate can state a conclusion whose
//! cited sources, read fully, say otherwise. For a claim that decides a verdict the supervisor
//! re-fetches the cited sources and compares.
//!
//! The comparison is deliberately narrow: it takes the claim's checkable terms (backticked or
//! double-quoted text, numbers, versions, identifiers such as `RUSTSEC-2020-0010`, paths and crate
//! names) and asks whether each appears in a fetched source, with the surrounding words so a person
//! sees how the source phrases it. It cannot tell a claim from its negation: present terms are not
//! proof of support, absent ones are proof of a gap. The supervisor still reads the excerpts.
//!
//! Fetching is a local file or, for `http(s)://`, the `curl` binary; the comparison core is pure.
use super::{Error, Res};
use regex::Regex;
use serde::Serialize;
use std::process::Command;
use std::sync::OnceLock;

/// Largest source read, in bytes.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// What the comparison found for one claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// Every term appears in at least one source.
    Confirmed,
    /// Some terms appear, some do not.
    Partial,
    /// No term appears in any source.
    Unsupported,
    /// The claim has no checkable term, so nothing can be compared.
    NoTerms,
    /// No source could be fetched.
    Unfetched,
}

/// A source after fetching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The URL or path as cited.
    pub source: String,
    /// Its text, or why it could not be read.
    pub text: Result<String, String>,
}

/// Where one term was seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hit {
    /// The source.
    pub source: String,
    /// The words around the first occurrence.
    pub excerpt: String,
}

/// One checkable term of the claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TermResult {
    /// The term as the claim wrote it.
    pub term: String,
    /// Where it appears (empty: nowhere).
    pub found_in: Vec<Hit>,
}

/// The comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The outcome.
    pub outcome: Outcome,
    /// Each term of the claim.
    pub terms: Vec<TermResult>,
    /// Sources that could not be fetched, with the reason.
    pub unreachable: Vec<String>,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("static pattern"))
}

/// The checkable terms of a claim, in order of first appearance, without duplicates.
#[must_use]
pub fn terms(claim: &str) -> Vec<String> {
    static QUOTED: OnceLock<Regex> = OnceLock::new();
    static IDENT: OnceLock<Regex> = OnceLock::new();
    static NUM: OnceLock<Regex> = OnceLock::new();
    let mut found: Vec<(usize, String)> = Vec::new();
    for c in re(&QUOTED, r#"`([^`\n]+)`|"([^"\n]+)""#).captures_iter(claim) {
        if let Some(m) = c.get(1).or_else(|| c.get(2)) {
            found.push((m.start(), m.as_str().trim().to_string()));
        }
    }
    for m in re(&IDENT, r"[A-Za-z0-9]+(?:(?:[-_./]|::)[A-Za-z0-9]+)+").find_iter(claim) {
        found.push((m.start(), m.as_str().trim_end_matches(".x").to_string()));
    }
    for m in re(&NUM, r"[0-9]+(?:[.,][0-9]+)*%?").find_iter(claim) {
        found.push((
            m.start(),
            m.as_str().trim_end_matches(['.', ',']).to_string(),
        ));
    }
    found.sort_by_key(|(i, _)| *i);
    let mut out: Vec<String> = Vec::new();
    for (_, t) in found {
        if !t.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&t)) {
            out.push(t);
        }
    }
    // A term wholly inside a longer one (a version inside an advisory id) adds nothing.
    let longer = out.clone();
    out.retain(|t| {
        !longer
            .iter()
            .any(|o| o.len() > t.len() && o.to_lowercase().contains(&t.to_lowercase()))
    });
    out
}

fn excerpt(text: &str, at: usize, len: usize) -> String {
    let mut lo = at.saturating_sub(60);
    let mut hi = (at + len + 60).min(text.len());
    while !text.is_char_boundary(lo) {
        lo -= 1;
    }
    while !text.is_char_boundary(hi) {
        hi += 1;
    }
    text[lo..hi]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The first place `term` stands alone (not inside a longer word or number) in `text`.
fn find_term(text: &str, term: &str) -> Option<usize> {
    let hay = text.to_lowercase();
    let needle = term.to_lowercase();
    // Lowercasing can change byte offsets for exotic text; compare only when the lengths agree.
    if hay.len() != text.len() {
        return None;
    }
    let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
    let mut from = 0;
    while let Some(i) = hay[from..].find(&needle) {
        let at = from + i;
        let before = hay[..at].chars().next_back();
        let after = hay[at + needle.len()..].chars().next();
        // "0.3" must not match inside "10.35"; a trailing "." or "," is prose, not part of it.
        let after_ok = edge(after)
            || (matches!(after, Some('.' | ',' | '-'))
                && edge(hay[at + needle.len() + 1..].chars().next()));
        if edge(before) && after_ok {
            return Some(at);
        }
        from = at + needle.len().max(1);
    }
    None
}

/// Compare a claim with the fetched sources.
#[must_use]
pub fn compare(claim: &str, sources: &[Fetched]) -> Report {
    let unreachable: Vec<String> = sources
        .iter()
        .filter_map(|s| s.text.as_ref().err().map(|e| format!("{}: {e}", s.source)))
        .collect();
    let readable: Vec<(&str, &str)> = sources
        .iter()
        .filter_map(|s| s.text.as_deref().ok().map(|t| (s.source.as_str(), t)))
        .collect();
    let ts = terms(claim);
    let results: Vec<TermResult> = ts
        .into_iter()
        .map(|term| TermResult {
            found_in: readable
                .iter()
                .filter_map(|(src, text)| {
                    find_term(text, &term).map(|at| Hit {
                        source: (*src).to_string(),
                        excerpt: excerpt(text, at, term.len()),
                    })
                })
                .collect(),
            term,
        })
        .collect();
    let seen = results.iter().filter(|t| !t.found_in.is_empty()).count();
    let outcome = if readable.is_empty() {
        Outcome::Unfetched
    } else if results.is_empty() {
        Outcome::NoTerms
    } else if seen == results.len() {
        Outcome::Confirmed
    } else if seen == 0 {
        Outcome::Unsupported
    } else {
        Outcome::Partial
    };
    Report {
        outcome,
        terms: results,
        unreachable,
    }
}

/// Fetch one cited source: an `http(s)://` URL through `curl`, otherwise a local path (a `file://`
/// prefix is accepted). Never follows a redirect to another scheme and never reads more than
/// [`MAX_BYTES`].
#[must_use]
pub fn fetch(source: &str) -> Fetched {
    let text = if source.starts_with("http://") || source.starts_with("https://") {
        fetch_url(source)
    } else {
        let path = source.strip_prefix("file://").unwrap_or(source);
        read_file(path)
    };
    Fetched {
        source: source.to_string(),
        text,
    }
}

fn read_file(path: &str) -> Result<String, String> {
    use std::io::Read;
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.take(MAX_BYTES)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn fetch_url(url: &str) -> Result<String, String> {
    let out = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--max-time",
            "30",
            "--max-filesize",
            &MAX_BYTES.to_string(),
            "--",
            url,
        ])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr)
            .lines()
            .next()
            .unwrap_or("fetch failed")
            .to_string())
    }
}

/// Fetch every source and compare.
///
/// # Errors
/// [`Error::Invalid`] when no source is given.
pub fn verify(claim: &str, sources: &[String]) -> Res<Report> {
    if sources.is_empty() {
        return Err(Error::Invalid("verify-sources needs a source".into()));
    }
    let fetched: Vec<Fetched> = sources.iter().map(|s| fetch(s)).collect();
    Ok(compare(claim, &fetched))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(name: &str, text: &str) -> Fetched {
        Fetched {
            source: name.into(),
            text: Ok(text.into()),
        }
    }

    #[test]
    fn terms_are_the_checkable_parts_of_a_claim() {
        let t = terms(
            "`tiberius` is unmaintained and archived, citing RUSTSEC-2020-0010 (0.3.x only), 12% of crates; see \"last push\" in docs/spike.md",
        );
        assert_eq!(
            t,
            [
                "tiberius",
                "RUSTSEC-2020-0010",
                "0.3",
                "12%",
                "last push",
                "docs/spike.md"
            ]
        );
    }

    #[test]
    fn a_claim_whose_terms_all_appear_is_confirmed_with_excerpts() {
        let r = compare(
            "RUSTSEC-2020-0010 covers 0.3 only",
            &[src(
                "a.md",
                "Advisory RUSTSEC-2020-0010 affects versions 0.3 and earlier only.",
            )],
        );
        assert_eq!(r.outcome, Outcome::Confirmed);
        assert!(r.terms[0].found_in[0].excerpt.contains("affects versions"));
    }

    #[test]
    fn a_term_the_sources_lack_makes_it_partial_or_unsupported() {
        let s = [src("a.md", "tiberius was last pushed in 2026-09-25")];
        let r = compare("`tiberius` repository is archived since 2021", &s);
        assert_eq!(r.outcome, Outcome::Partial);
        assert!(
            r.terms
                .iter()
                .any(|t| t.term == "2021" && t.found_in.is_empty())
        );
        let r = compare("`mysql` crate 9.9", &s);
        assert_eq!(r.outcome, Outcome::Unsupported);
    }

    #[test]
    fn numbers_match_whole_only() {
        let s = [src("a", "version 10.35 and 20.3x")];
        assert_eq!(compare("0.3", &s).outcome, Outcome::Unsupported);
        assert_eq!(compare("10.35", &s).outcome, Outcome::Confirmed);
        assert_eq!(
            compare("got 7.", &[src("a", "got 7.")]).outcome,
            Outcome::Confirmed
        );
    }

    #[test]
    fn a_claim_without_terms_or_sources_cannot_be_compared() {
        assert_eq!(
            compare("it is fine", &[src("a", "x")]).outcome,
            Outcome::NoTerms
        );
        let bad = Fetched {
            source: "https://x".into(),
            text: Err("down".into()),
        };
        let r = compare("v1.2", &[bad]);
        assert_eq!(r.outcome, Outcome::Unfetched);
        assert_eq!(r.unreachable, ["https://x: down"]);
        assert!(verify("v1.2", &[]).is_err());
    }

    #[test]
    fn a_missing_file_is_unreachable_not_an_error() {
        let r = verify("v1.2", &["/no/such/file".into()]).unwrap();
        assert_eq!(r.outcome, Outcome::Unfetched);
        assert_eq!(r.unreachable.len(), 1);
    }
}
