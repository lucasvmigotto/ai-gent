//! The `gopass` backend (spec 037 US2): used when `pass` is not on `PATH`, mainly on Windows.
//!
//! ASSUMPTION (open question 2, task T006): `gopass show -f ENTRY` prints the whole secret without
//! prompting on stdin, and `gopass insert -m ENTRY` reads a multiline value from stdin. Neither was
//! verified on a real Windows install; the unit tests use a fake executable and assert these argv.

/// Arguments of the read command.
pub(super) fn show_args(entry: &str) -> Vec<String> {
    vec!["show".into(), "-f".into(), entry.into()]
}

/// Arguments of the write command.
pub(super) fn insert_args(entry: &str) -> Vec<String> {
    vec!["insert".into(), "-m".into(), entry.into()]
}
