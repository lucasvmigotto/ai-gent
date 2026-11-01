//! Parity gate for `ai-gent tokens` (spec 035, D10): the cases in `rust/fixtures/tokens/*.json` are
//! the Python tool's scenarios and outputs (`scripts/test-parity-tokens.py --bless` writes them from
//! `token_report.py`); each is materialised in a temp directory, run through the binary, and compared
//! on exit code, stdout and every file the run created or changed.
#![allow(clippy::cast_possible_wrap)]
use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/tokens");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    v.sort();
    v
}

fn civil(epoch: i64) -> String {
    let days = epoch.div_euclid(86_400);
    let rem = epoch.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn expand(text: &str, now: i64) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("@NOW-") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 5..];
        let n = tail.find('@').expect("unterminated @NOW-..@");
        out.push_str(&civil(now - tail[..n].parse::<i64>().unwrap()));
        rest = &tail[n + 1..];
    }
    out.push_str(rest);
    out
}

fn normalise(text: &str, root: &str) -> String {
    let t = text
        .replace(root, "<ROOT>")
        .replace(&root.replace('/', "\\"), "<ROOT>");
    let mut out = String::new();
    let mut rest = t.as_str();
    while let Some(i) = rest.find("Generated ") {
        out.push_str(&rest[..i + 10]);
        let tail = &rest[i + 10..];
        let b = tail.as_bytes();
        let stamp = b.len() >= 16
            && b[..4].iter().all(u8::is_ascii_digit)
            && b[4] == b'-'
            && b[7] == b'-'
            && b[10] == b' '
            && b[13] == b':';
        if stamp {
            out.push_str("<TS>");
            rest = &tail[16..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                walk(&p, root, out);
            } else {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn run_case(path: &Path) -> Vec<String> {
    let case: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let name = case["name"].as_str().unwrap().to_string();
    let tmp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(tmp.path()).unwrap();
    // Windows canonicalises to `\\?\C:\...`, which refuses `/`; use the plain drive path, forward slashes.
    let plain = root
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    let rs = plain.replace('\\', "/");
    let root = PathBuf::from(&plain);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    for (rel, text) in case["files"].as_object().unwrap() {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(
            p,
            expand(&text.as_str().unwrap().replace("<ROOT>", &rs), now),
        )
        .unwrap();
    }
    std::fs::create_dir_all(root.join("work")).unwrap();
    std::fs::create_dir_all(root.join("ai-home")).unwrap();
    let before = tree(&root);
    let mut problems = Vec::new();
    for (i, run) in case["runs"].as_array().unwrap().iter().enumerate() {
        let argv: Vec<String> = run["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a.as_str().unwrap().replace("<ROOT>", &rs))
            .collect();
        let mut cmd = Command::cargo_bin("ai-gent").unwrap();
        cmd.arg("tokens")
            .args(&argv)
            .current_dir(root.join("work"))
            .env_clear()
            .env("HOME", root.join("home"))
            .env("AI_GENT_HOME", root.join("ai-home"))
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("NO_COLOR", "1");
        if let Some(s) = run["stdin"].as_str() {
            cmd.write_stdin(s.to_string());
        }
        let out = cmd.output().unwrap();
        let stdout = normalise(&String::from_utf8_lossy(&out.stdout), &rs);
        let want_exit = run["exit"].as_i64().unwrap();
        let got_exit = i64::from(out.status.code().unwrap_or(-1));
        if want_exit != got_exit {
            problems.push(format!(
                "{name} run {i} ({}): exit {got_exit}, expected {want_exit}",
                argv.join(" ")
            ));
        }
        let want = run["stdout"].as_str().unwrap();
        if want != stdout {
            problems.push(format!(
                "{name} run {i} ({}): stdout differs\n--- expected\n{want}\n--- got\n{stdout}",
                argv.join(" ")
            ));
        }
    }
    let after = tree(&root);
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for (rel, data) in &after {
        if before.get(rel) != Some(data) {
            files.insert(rel.clone(), normalise(&String::from_utf8_lossy(data), &rs));
        }
    }
    let expect: BTreeMap<String, String> = case["expect_files"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
        .collect();
    for rel in expect.keys().chain(files.keys()) {
        if expect.get(rel) != files.get(rel) {
            problems.push(format!(
                "{name}: file {rel} differs\n--- expected\n{}\n--- got\n{}",
                expect.get(rel).map_or("<absent>", String::as_str),
                files.get(rel).map_or("<absent>", String::as_str)
            ));
        }
    }
    problems.dedup();
    problems
}

#[test]
fn every_python_scenario_gives_the_same_output_and_files() {
    let all = fixtures();
    assert!(
        all.len() >= 50,
        "expected the blessed fixtures, found {}",
        all.len()
    );
    let problems: Vec<String> = all.iter().flat_map(|p| run_case(p)).collect();
    assert!(
        problems.is_empty(),
        "{} difference(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn the_cases_cover_every_command() {
    let mut seen = std::collections::BTreeSet::new();
    for p in fixtures() {
        let case: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        for r in case["runs"].as_array().unwrap() {
            seen.insert(r["argv"][0].as_str().unwrap().to_string());
        }
    }
    for c in [
        "report",
        "tasks",
        "estimate",
        "index",
        "seed",
        "budget",
        "snapshot",
        "backfill-git",
    ] {
        assert!(seen.contains(c), "no parity case runs `{c}`");
    }
}
