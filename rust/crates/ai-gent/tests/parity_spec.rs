//! Parity gate for `ai-gent spec` (spec 037, D10): the cases in `rust/fixtures/spec/*.json` are the
//! scenarios of `test_specsync.py` plus this repository's own `specs/` (live corpus), blessed from
//! `specsync.py` by `scripts/test-parity-spec.py --bless`.
mod parity_support;
use parity_support::{fixtures, run_spec_case};

#[test]
fn every_python_scenario_gives_the_same_output_and_files() {
    let all = fixtures("spec");
    assert!(
        all.len() >= 16,
        "expected the blessed fixtures, found {}",
        all.len()
    );
    let problems: Vec<String> = all.iter().flat_map(|p| run_spec_case(p)).collect();
    assert!(
        problems.is_empty(),
        "{} difference(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn the_live_corpus_is_among_the_cases() {
    assert!(
        fixtures("spec")
            .iter()
            .any(|p| p.file_stem().is_some_and(|s| s == "live-corpus"))
    );
}
