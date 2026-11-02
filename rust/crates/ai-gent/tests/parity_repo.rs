//! Parity gate for `ai-gent repo` (spec 037, D10): the cases in `rust/fixtures/repo/*.json` are the
//! scenarios of `test_reporun.py` (git repo, pass store, fake tools, loopback stub) blessed from
//! `reporun.py` by `scripts/test-parity-repo.py --bless`. Each run also checks the canary (FR-A5): the
//! fake token never reaches stdout, stderr or a created file. Network: only the loopback stub.
mod parity_support;
use parity_support::{fixtures, repo::run_repo_case};

#[test]
fn every_python_scenario_gives_the_same_output_requests_and_files() {
    let all = fixtures("repo");
    assert!(
        all.len() >= 50,
        "expected the blessed fixtures, found {}",
        all.len()
    );
    let problems: Vec<String> = all.iter().flat_map(|p| run_repo_case(p)).collect();
    assert!(
        problems.is_empty(),
        "{} difference(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}
