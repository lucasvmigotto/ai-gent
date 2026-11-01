//! Shared by the spec and repo parity runners (spec 037): materialise a fixture case in a temp
//! directory, run the binary, compare exit code, stdout and every file the run created or changed.
#![allow(dead_code)]
use assert_cmd::Command;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn fixtures(dir: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(dir);
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    v.sort();
    v
}

pub fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
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

/// A temp root as a plain path (Windows canonicalises to `\\?\C:\...`, which refuses `/`).
pub fn plain_root(tmp: &Path) -> (PathBuf, String) {
    let root = std::fs::canonicalize(tmp).unwrap();
    let plain = root
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_string();
    let fwd = plain.replace('\\', "/");
    (PathBuf::from(&plain), fwd)
}

/// Replace the root by `<ROOT>` and the `Last sync` date by `<DATE>`.
pub fn normalise(text: &str, root: &str) -> String {
    let t = text
        .replace(root, "<ROOT>")
        .replace(&root.replace('/', "\\"), "<ROOT>");
    let mut out = String::new();
    let mut rest = t.as_str();
    while let Some(i) = rest.find("Last sync: ") {
        out.push_str(&rest[..i + 11]);
        let tail = &rest[i + 11..];
        let b = tail.as_bytes();
        let stamp = b.len() >= 10
            && b[..4].iter().all(u8::is_ascii_digit)
            && b[4] == b'-'
            && b[7] == b'-'
            && b[5..7].iter().all(u8::is_ascii_digit)
            && b[8..10].iter().all(u8::is_ascii_digit);
        if stamp {
            out.push_str("<DATE>");
            rest = &tail[10..];
        } else {
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

/// Run one spec case through `ai-gent spec`.
pub fn run_spec_case(path: &Path) -> Vec<String> {
    let case: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let name = case["name"].as_str().unwrap().to_string();
    let tmp = tempfile::tempdir().unwrap();
    let (root, rs) = plain_root(tmp.path());
    for (rel, text) in case["files"].as_object().unwrap() {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text.as_str().unwrap().replace("<ROOT>", &rs)).unwrap();
    }
    std::fs::create_dir_all(root.join("work")).unwrap();
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
        cmd.arg("spec")
            .args(&argv)
            .current_dir(root.join("work"))
            .env_clear()
            .env("HOME", root.join("home"))
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("NO_COLOR", "1");
        let out = cmd.output().unwrap();
        let stdout = normalise(&String::from_utf8_lossy(&out.stdout), &rs);
        let want_exit = run["exit"].as_i64().unwrap();
        let got_exit = i64::from(out.status.code().unwrap_or(-1));
        let label = argv.iter().take(3).cloned().collect::<Vec<_>>().join(" ");
        if want_exit != got_exit {
            problems.push(format!(
                "{name} run {i} ({label}): exit {got_exit}, expected {want_exit}"
            ));
        }
        let want = run["stdout"].as_str().unwrap();
        if want != stdout {
            problems.push(format!(
                "{name} run {i} ({label}): stdout differs\n--- expected\n{want}\n--- got\n{stdout}"
            ));
        }
    }
    problems.extend(compare_files(&name, &case, &before, &root, &rs));
    problems
}

pub fn compare_files(
    name: &str,
    case: &Value,
    before: &BTreeMap<String, Vec<u8>>,
    root: &Path,
    rs: &str,
) -> Vec<String> {
    let after = tree(root);
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for (rel, data) in &after {
        if before.get(rel) != Some(data) {
            files.insert(rel.clone(), normalise(&String::from_utf8_lossy(data), rs));
        }
    }
    let expect: BTreeMap<String, String> = case["expect_files"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
        .collect();
    let mut problems = Vec::new();
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
