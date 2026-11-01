//! Parity gate for `ai-gent wt` (spec 036, D10): the cases in `rust/fixtures/wt/*.json` are the Python
//! tool's scenarios and outputs (`scripts/test-parity-wt.py --bless` writes them from `wt.py`); each
//! runs in a temp git repository against the binary with the fake container engine, and every step
//! is compared on exit code, stdout, stderr, engine state and files.
#![allow(
    clippy::too_many_lines,
    clippy::cast_possible_truncation,
    clippy::format_collect
)]
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/wt");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    v.sort();
    v
}

/// The temp root as git prints it: forward slashes, no `\\?\` prefix, symlinks resolved.
fn clean(p: &Path) -> String {
    let s = std::fs::canonicalize(p)
        .unwrap()
        .to_string_lossy()
        .to_string();
    s.strip_prefix(r"\\?\").unwrap_or(&s).replace('\\', "/")
}

fn norm(text: &str, root: &str) -> String {
    text.replace("\r\n", "\n")
        .replace(&root.replace('/', "\\"), "<ROOT>")
        .replace(root, "<ROOT>")
}

fn git(args: &[&str], cwd: &Path) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The fake engine example, built next to the test binaries (`target/<profile>/examples/`).
fn fake_engine() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap().parent().unwrap().join("examples");
    let p = dir.join(format!(
        "ai-gent-fake-engine{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(p.exists(), "{} (cargo test builds examples)", p.display());
    p
}

fn state_view(path: &Path) -> Value {
    let st: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let col = |k: &str, f: &str| -> Vec<Value> {
        st[k]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x[f].clone())
            .collect()
    };
    json!({"containers": col("containers", "id"), "volumes": col("volumes", "name"),
           "images": col("images", "id"), "calls": st["calls"]})
}

fn scoped(path: &Path) -> Vec<String> {
    let st: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut problems = Vec::new();
    for call in st["calls"].as_array().unwrap() {
        let c: Vec<&str> = call
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        if c.contains(&"prune") {
            problems.push(format!("prune in {c:?}"));
        }
        let listing = c.first() == Some(&"ps")
            || c.first() == Some(&"images")
            || c.starts_with(&["volume", "ls"]);
        if listing && !c.contains(&"--external") && !c.iter().any(|a| a.starts_with("label=")) {
            problems.push(format!("unfiltered listing {c:?}"));
        }
    }
    problems
}

fn run_case(file: &Path) {
    let case: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let name = case["name"].as_str().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let root = clean(tmp.path());
    let repo = PathBuf::from(&root).join("shop");
    git(
        &["init", "-q", "-b", "main", repo.to_str().unwrap()],
        Path::new(&root),
    );
    std::fs::create_dir(repo.join("specs")).unwrap();
    let readme = case["readme"].as_str().unwrap();
    std::fs::write(repo.join("specs/README.md"), readme).unwrap();
    git(&["add", "."], &repo);
    git(&["commit", "-q", "-m", "chore: initial commit"], &repo);
    git(&["branch", "dev"], &repo);
    let state_path = PathBuf::from(&root).join("engine.json");
    let write_state = |extra: Value| {
        let mut st = json!({"containers": [], "volumes": [], "images": [], "calls": []});
        for (k, v) in extra.as_object().unwrap() {
            st[k] = v.clone();
        }
        std::fs::write(&state_path, st.to_string()).unwrap();
    };
    write_state(json!({}));
    let (mut usage, mut builds) = ([500u64, 200u64], json!({"mine": [], "other": []}));
    let no_engine = case.get("engine") == Some(&json!(false));
    let fake = fake_engine();
    for (i, step) in case["steps"].as_array().unwrap().iter().enumerate() {
        let at = format!("{name} step {i}: {step}");
        let cwd = repo.join(step.get("cwd").and_then(Value::as_str).unwrap_or("."));
        let expect = step.get("expect");
        let strs = |k: &str| -> Vec<String> {
            step[k]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect()
        };
        if step.get("wt").is_some() {
            let mut c = Command::new(assert_cmd::cargo::cargo_bin("ai-gent"));
            c.arg("wt").args(strs("wt")).current_dir(&cwd);
            // `--json` is global in the binary; the Python flag position is kept as recorded
            for (k, v) in [
                ("GIT_AUTHOR_NAME", "t"),
                ("GIT_AUTHOR_EMAIL", "t@t"),
                ("GIT_COMMITTER_NAME", "t"),
                ("GIT_COMMITTER_EMAIL", "t@t"),
                ("GIT_CONFIG_NOSYSTEM", "1"),
            ] {
                c.env(k, v);
            }
            c.env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env(
                "CONTAINER_ENGINE",
                if no_engine {
                    String::new()
                } else {
                    fake.to_string_lossy().to_string()
                },
            )
            .env("FAKE_ENGINE_STATE", &state_path)
            .env(
                "AI_GENT_WT_FAKE_USAGE",
                format!(
                    "{},{},{root}",
                    usage[0] * 1_000_000_000,
                    usage[1] * 1_000_000_000
                ),
            )
            .env("AI_GENT_WT_FAKE_BUILDS", builds.to_string())
            .env("NO_COLOR", "1");
            if no_engine {
                c.env("AI_GENT_WT_NO_ENGINE", "1");
            }
            let out = c.output().unwrap();
            let got = json!({"code": out.status.code().unwrap(),
                "stdout": norm(&String::from_utf8_lossy(&out.stdout), &root),
                "stderr": norm(&String::from_utf8_lossy(&out.stderr), &root)});
            assert_eq!(&got, expect.unwrap(), "{at}");
        } else if let Some(g) = step.get("git") {
            let a: Vec<String> = g
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect();
            git(&a.iter().map(String::as_str).collect::<Vec<_>>(), &cwd);
        } else if let Some(w) = step.get("write") {
            for (rel, text) in w.as_object().unwrap() {
                let p = repo.join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, text.as_str().unwrap()).unwrap();
            }
        } else if let Some(r) = step.get("rm") {
            std::fs::remove_file(repo.join(r.as_str().unwrap())).unwrap();
        } else if let Some(s) = step.get("engine_state") {
            write_state(s.clone());
        } else if let Some(u) = step.get("engine_update") {
            let mut st: Value =
                serde_json::from_str(&std::fs::read_to_string(&state_path).unwrap()).unwrap();
            for (k, v) in u.as_object().unwrap() {
                if k == "containers_append" {
                    st["containers"]
                        .as_array_mut()
                        .unwrap()
                        .extend(v.as_array().unwrap().clone());
                } else {
                    st[k] = v.clone();
                }
            }
            std::fs::write(&state_path, st.to_string()).unwrap();
        } else if let Some(u) = step.get("usage") {
            usage = [u[0].as_u64().unwrap(), u[1].as_u64().unwrap()];
        } else if let Some(b) = step.get("builds") {
            builds = b.clone();
        } else if step.get("state").is_some() {
            assert_eq!(&state_view(&state_path), expect.unwrap(), "{at}");
        } else if step.get("assert_scoped").is_some() {
            assert_eq!(scoped(&state_path), Vec::<String>::new(), "{at}");
        } else if let Some(c) = step.get("cat") {
            let text = std::fs::read_to_string(repo.join(c.as_str().unwrap()))
                .unwrap()
                .replace("\r\n", "\n");
            let text = match step.get("tail").and_then(Value::as_u64) {
                Some(n) => {
                    let lines: Vec<&str> = text.lines().collect();
                    lines[lines.len().saturating_sub(n as usize)..]
                        .iter()
                        .map(|l| format!("{l}\n"))
                        .collect()
                }
                None => text,
            };
            assert_eq!(
                &json!({"text": norm(&text, &root)}),
                expect.unwrap(),
                "{at}"
            );
        } else if let Some(e) = step.get("exists") {
            assert_eq!(
                &json!({"exists": repo.join(e.as_str().unwrap()).exists()}),
                expect.unwrap(),
                "{at}"
            );
        } else if let Some(g) = step.get("gitout") {
            let a: Vec<String> = g
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect();
            let text = git(&a.iter().map(String::as_str).collect::<Vec<_>>(), &repo);
            assert_eq!(
                &json!({"text": norm(&text, &root)}),
                expect.unwrap(),
                "{at}"
            );
        } else {
            panic!("unknown step: {at}");
        }
    }
}

#[test]
fn every_fixture_matches_the_python_tool() {
    let all = fixtures();
    assert!(all.len() >= 20, "fixtures: {}", all.len());
    for f in all {
        run_case(&f);
    }
}

/// The worktree board (spec 022) reads exactly these keys; `worktrees-fixtures.ts` records them.
#[test]
fn list_json_keeps_the_keys_the_worktree_board_reads() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../plugins/agents/hooks/worktrees-fixtures.ts");
    let ts = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let recorded = |name: &str| -> Vec<Value> {
        let head = format!("export const {name} = `");
        let rest = &ts[ts.find(&head).unwrap() + head.len()..];
        serde_json::from_str(&rest[..rest.find('`').unwrap()]).unwrap()
    };
    let keys = |v: &Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    let expected: Vec<String> = ["ahead", "behind", "branch", "dirty", "path"]
        .iter()
        .map(ToString::to_string)
        .collect();
    for name in ["WT_LIST", "WT_LIST_NO_BASE"] {
        for row in recorded(name) {
            let mut k = keys(&row);
            k.sort();
            assert_eq!(k, expected, "{name}");
        }
    }
    // the binary's own rows, from the `list-keys` fixture
    let case: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/wt/list-keys.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let first = case["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| {
            s.get("wt")
                .is_some_and(|w| w[0] == "list" && w[1] == "--json")
        })
        .unwrap();
    let rows: Vec<Value> =
        serde_json::from_str(first["expect"]["stdout"].as_str().unwrap()).unwrap();
    let mut k = keys(&rows[0]);
    k.sort();
    assert_eq!(k, expected);
}
