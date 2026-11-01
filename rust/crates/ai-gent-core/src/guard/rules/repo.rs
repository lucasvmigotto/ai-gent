//! Port of `plugins/repo/hooks/guard.sh`.
use super::{attributed, re};
use crate::guard::Decision;
use crate::guard::reasons::decide;
use crate::guard::segment::View;

/// Evaluate the repo guard on a shell command.
#[must_use]
pub fn evaluate(v: &View) -> Decision {
    let runner = re(
        r"^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)*((python3?|uv[[:space:]]+run([[:space:]]+python3?)?)[[:space:]]+)?([^[:space:]]*/)?reporun(\.py)?[[:space:]]",
    );
    let write = re(r"[[:space:]]--write([[:space:]]|$)");
    if !v
        .segments
        .iter()
        .any(|s| runner.is_match(s) && write.is_match(s))
    {
        return Decision::none();
    }
    if attributed(&v.raw) {
        return decide("repo.attrib", &[]);
    }
    decide("repo.write", &[])
}
