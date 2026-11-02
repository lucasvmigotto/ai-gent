//! The repo-case runner: a git repository, a pass store, fake tools, a loopback HTTP stub (the twin of
//! the one in `scripts/test-parity-repo.py`) and the canary check (spec 037 FR-A5).
#![allow(dead_code, clippy::too_many_lines, clippy::items_after_statements)]
use super::{compare_files, plain_root, tree};
use assert_cmd::Command;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const TOKEN: &str = "tok-3f9a1c77e0";

type Seen = Arc<Mutex<Vec<Value>>>;

fn find_header_end(b: &[u8]) -> Option<usize> {
    b.windows(4).position(|w| w == b"\r\n\r\n")
}

fn handle(mut s: TcpStream, seen: &Seen, counter: &Arc<Mutex<i64>>, port: u16) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = find_header_end(&buf) {
            break i;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.lines();
    let first = lines.next().unwrap_or("");
    let mut parts = first.split(' ');
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }
    let len: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        let n = s.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let mut hdrs = serde_json::Map::new();
    for (canon, low) in [
        ("Authorization", "authorization"),
        ("Content-Type", "content-type"),
        ("Accept", "accept"),
    ] {
        if let Some(v) = headers.get(low) {
            hdrs.insert(canon.to_string(), json!(v));
        }
    }
    seen.lock().unwrap().push(json!({
        "method": method, "path": path, "headers": Value::Object(hdrs),
        "body": String::from_utf8_lossy(&body)
    }));
    let bare = path.split('?').next().unwrap_or("");
    // Literal payloads, in the key order the Python stub sends (serde_json would sort the keys).
    let (status, raw) = if path.starts_with("/echo-token") {
        (401, format!("{{\"message\":\"bad credentials {TOKEN}\"}}"))
    } else if method == "POST" && bare.ends_with("/issues") {
        let mut c = counter.lock().unwrap();
        *c += 1;
        (
            201,
            format!(
                "{{\"number\":{},\"id\":{},\"html_url\":\"https://github.com/o/r/issues/{}\"}}",
                *c,
                9000 + *c,
                *c
            ),
        )
    } else if path.starts_with("/slow") {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        (200, "{\"slow\":true}".to_string())
    } else if path.starts_with("/fail") {
        (422, "{\"message\":\"Validation Failed\"}".to_string())
    } else if path.starts_with("/page1") {
        (
            200,
            format!("{{\"values\":[1,2],\"next\":\"http://127.0.0.1:{port}/page2\"}}"),
        )
    } else if path.starts_with("/page2") {
        (200, "{\"values\":[3]}".to_string())
    } else {
        (
            if method == "POST" { 201 } else { 200 },
            "{\"ok\":true}".to_string(),
        )
    };
    let resp = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{raw}",
        raw.len()
    );
    let _ = s.write_all(resp.as_bytes());
}

pub struct Stub {
    pub url: String,
    pub seen: Seen,
}

pub fn start_stub() -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen: Seen = Arc::default();
    let counter = Arc::new(Mutex::new(100i64));
    let (s2, c2) = (seen.clone(), counter);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (s3, c3) = (s2.clone(), c2.clone());
            std::thread::spawn(move || handle(stream, &s3, &c3, port));
        }
    });
    Stub {
        url: format!("http://127.0.0.1:{port}"),
        seen,
    }
}

pub fn fake_tool() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap().parent().unwrap().join("examples");
    static BUILD: std::sync::Once = std::sync::Once::new();
    BUILD.call_once(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let ok = std::process::Command::new(cargo)
            .args([
                "build",
                "--example",
                "fake_tool",
                "-p",
                "ai-gent",
                "--locked",
            ])
            .status()
            .is_ok_and(|s| s.success());
        assert!(ok, "could not build the fake_tool example");
    });
    dir.join(if cfg!(windows) {
        "fake_tool.exe"
    } else {
        "fake_tool"
    })
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<&str> = if cfg!(windows) {
        vec![".exe", ".cmd", ".bat", ""]
    } else {
        vec![""]
    };
    for dir in std::env::split_paths(&path) {
        for e in &exts {
            let c = dir.join(format!("{name}{e}"));
            if c.is_file() {
                return Some(c);
            }
        }
    }
    None
}

fn git(root: &Path, args: &[String], env: &BTreeMap<String, String>) {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root.join("repo"))
        .env_clear()
        .envs(env)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

fn norm(text: &str, root: &str, stub: &str) -> String {
    let mut t = text
        .replace(root, "<ROOT>")
        .replace(&root.replace('/', "\\"), "<ROOT>");
    if let Some(h) = stub.strip_prefix("http://") {
        t = t.replace(h, "<STUB>");
    }
    t
}

pub fn run_repo_case(path: &Path) -> Vec<String> {
    let case: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let name = case["name"].as_str().unwrap().to_string();
    let tmp = tempfile::tempdir().unwrap();
    let (root, rs) = plain_root(tmp.path());
    for d in ["home", "store", "bin", "repo"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    // A PATH with only the fake tools, git and curl: a real gh or az on this machine never interferes.
    let mut path_dirs: Vec<PathBuf> = vec![root.join("bin")];
    for t in ["git", "curl"] {
        let found = which(t).unwrap_or_else(|| panic!("{t} is needed"));
        if cfg!(windows) {
            path_dirs.push(found.parent().unwrap().to_path_buf());
        } else {
            #[cfg(unix)]
            std::os::unix::fs::symlink(&found, root.join("bin").join(t)).unwrap();
        }
    }
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    env.insert(
        "HOME".into(),
        root.join("home").to_string_lossy().to_string(),
    );
    env.insert(
        "PASSWORD_STORE_DIR".into(),
        root.join("store").to_string_lossy().to_string(),
    );
    env.insert(
        "PATH".into(),
        std::env::join_paths(&path_dirs)
            .unwrap()
            .to_string_lossy()
            .to_string(),
    );
    env.insert(
        "GIT_CONFIG_GLOBAL".into(),
        root.join("gitconfig").to_string_lossy().to_string(),
    );
    for (k, v) in [
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("NO_COLOR", "1"),
        ("TZ", "UTC"),
        ("GIT_AUTHOR_NAME", "t"),
        ("GIT_AUTHOR_EMAIL", "t@t"),
        ("GIT_COMMITTER_NAME", "t"),
        ("GIT_COMMITTER_EMAIL", "t@t"),
        ("LANG", "C.UTF-8"),
    ] {
        env.insert(k.into(), v.into());
    }
    for k in [
        "SystemRoot",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "TMPDIR",
        "PATHEXT",
        "COMSPEC",
        "USERPROFILE",
    ] {
        if let Ok(v) = std::env::var(k) {
            env.insert(
                k.into(),
                if k == "USERPROFILE" {
                    root.join("home").to_string_lossy().to_string()
                } else {
                    v
                },
            );
        }
    }
    git(
        &root,
        &[
            "init".into(),
            "-q".into(),
            root.join("repo").to_string_lossy().to_string(),
        ],
        &env,
    );
    if let Some(r) = case["remote"].as_str() {
        git(&root, &strs(&json!(["remote", "add", "origin", r])), &env);
    }
    let stub = case["stub"].as_bool().unwrap_or(false).then(start_stub);
    let stub_url = stub.as_ref().map_or(String::new(), |s| s.url.clone());
    let mut tools: Vec<(String, Value)> = vec![("pass".into(), json!({}))];
    for (n, v) in case["tools"].as_object().unwrap() {
        tools.push((n.clone(), v.clone()));
    }
    for (n, spec) in &tools {
        let dest = root.join("bin").join(if cfg!(windows) {
            format!("{n}.exe")
        } else {
            n.clone()
        });
        std::fs::copy(fake_tool(), &dest).unwrap();
        let rules: Vec<Value> = spec["rules"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| {
                        let mut r = r.clone();
                        if let Some(c) = r["capture"].as_str().map(|c| c.replace("<ROOT>", &rs)) {
                            r["capture"] = json!(c);
                        }
                        r
                    })
                    .collect()
            })
            .unwrap_or_default();
        std::fs::write(
            root.join("bin").join(format!("{n}.rules.json")),
            json!({"rules": rules}).to_string(),
        )
        .unwrap();
    }
    for (entry, text) in case["store"].as_object().unwrap() {
        let p = root.join("store").join(format!("{entry}.gpg"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text.as_str().unwrap()).unwrap();
    }
    for (rel, text) in case["files"].as_object().unwrap() {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text.as_str().unwrap().replace("<ROOT>", &rs)).unwrap();
    }
    for g in case["git"].as_array().unwrap() {
        git(&root, &strs(g), &env);
    }
    if stub.is_some() {
        env.insert("REPORUN_TEST_BASE".into(), stub_url.clone());
    }
    for (k, v) in case["env"].as_object().unwrap() {
        match v.as_str() {
            Some(s) => env.insert(k.clone(), s.replace("{stub}", &stub_url)),
            None => env.remove(k),
        };
    }
    let before = tree(&root);
    let mut problems = Vec::new();
    let scanner = case["scan"].as_bool().unwrap_or(false).then(start_scanner);
    let mut cursor = 0usize;
    for (i, run) in case["runs"].as_array().unwrap().iter().enumerate() {
        if let Some(gs) = run.get("git").and_then(Value::as_array) {
            for g in gs {
                git(&root, &strs(g), &env);
            }
        }
        let argv: Vec<String> = strs(&run["argv"])
            .iter()
            .map(|a| a.replace("<ROOT>", &rs))
            .collect();
        let mut cmd = Command::cargo_bin("ai-gent").unwrap();
        cmd.arg("repo")
            .args(&argv)
            .current_dir(root.join("repo"))
            .env_clear()
            .envs(&env);
        if let Some(s) = run["stdin"].as_str() {
            cmd.write_stdin(s.to_string());
        }
        let out = cmd.output().unwrap();
        let stdout = norm(&String::from_utf8_lossy(&out.stdout), &rs, &stub_url);
        let stderr = norm(&String::from_utf8_lossy(&out.stderr), &rs, &stub_url);
        let label = argv.iter().take(3).cloned().collect::<Vec<_>>().join(" ");
        let want_exit = run["exit"].as_i64().unwrap();
        let got_exit = i64::from(out.status.code().unwrap_or(-1));
        if want_exit != got_exit {
            problems.push(format!(
                "{name} run {i} ({label}): exit {got_exit}, expected {want_exit}\n{stderr}"
            ));
        }
        if run["stdout"].as_str().unwrap() != stdout {
            problems.push(format!(
                "{name} run {i} ({label}): stdout differs\n--- expected\n{}\n--- got\n{stdout}",
                run["stdout"].as_str().unwrap()
            ));
        }
        for needle in strs(&run["stderr_has"]) {
            if !stderr.contains(&needle) {
                problems.push(format!(
                    "{name} run {i} ({label}): stderr lacks {needle:?}\n{stderr}"
                ));
            }
        }
        // The canary (FR-A5): the token may be in the child's environment and the stub's request only.
        if stdout.contains(TOKEN) || stderr.contains(TOKEN) {
            problems.push(format!(
                "{name} run {i} ({label}): the token reached stdout or stderr"
            ));
        }
        let seen: Vec<Value> = stub
            .as_ref()
            .map(|s| {
                let all = s.seen.lock().unwrap();
                let new: Vec<Value> = all[cursor..].to_vec();
                cursor = all.len();
                new
            })
            .unwrap_or_default();
        let seen: Vec<Value> = seen
            .into_iter()
            .map(|mut v| {
                let p = norm(v["path"].as_str().unwrap(), &rs, &stub_url);
                v["path"] = json!(p);
                v
            })
            .collect();
        if run["seen"] != Value::Array(seen.clone()) {
            problems.push(format!(
                "{name} run {i} ({label}): requests differ\n--- expected\n{}\n--- got\n{}",
                run["seen"],
                Value::Array(seen)
            ));
        }
    }
    if let Some((stop, handle)) = scanner {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let (hits, looked) = handle.join().unwrap();
        if cfg!(target_os = "linux") && looked == 0 {
            problems.push(format!("{name}: the process scan saw no process"));
        }
        if hits > 0 {
            problems.push(format!(
                "{name}: the token was on a command line ({hits} time(s))"
            ));
        }
    }
    // Files: only what the runs created or changed, outside the scaffolding.
    let mut filtered = before.clone();
    filtered.retain(|k, _| {
        !k.starts_with("bin/")
            && !k.starts_with("home/")
            && !k.starts_with("store/")
            && k != "gitconfig"
    });
    let after_all = tree(&root);
    let mut after = after_all.clone();
    after.retain(|k, _| {
        !k.starts_with("bin/")
            && !k.starts_with("home/")
            && !k.starts_with("store/")
            && k != "gitconfig"
            && !k.starts_with("repo/.git/")
    });
    filtered.retain(|k, _| !k.starts_with("repo/.git/"));
    let _ = after;
    problems.extend(compare_files(
        &name,
        &case,
        &filtered,
        &root,
        &rs,
        &|rel: &str| {
            rel.starts_with("bin/")
                || rel.starts_with("home/")
                || rel.starts_with("store/")
                || rel == "gitconfig"
                || rel.starts_with("repo/.git/")
        },
    ));
    for (rel, data) in tree(&root) {
        if rel.starts_with("repo/.git/") || rel.starts_with("bin/") {
            continue;
        }
        if String::from_utf8_lossy(&data).contains(TOKEN) && !rel.starts_with("store/") {
            problems.push(format!("{name}: the token reached {rel}"));
        }
    }
    problems
}

type Scanner = (
    Arc<std::sync::atomic::AtomicBool>,
    std::thread::JoinHandle<(usize, usize)>,
);

/// Poll `/proc/*/cmdline` for the token while a case runs (Linux; elsewhere it looks at nothing).
fn start_scanner() -> Scanner {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = stop.clone();
    let handle = std::thread::spawn(move || {
        let (mut hits, mut looked) = (0, 0);
        while !flag.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(dir) = std::fs::read_dir("/proc") {
                for e in dir.flatten() {
                    if let Ok(raw) = std::fs::read(e.path().join("cmdline")) {
                        looked += 1;
                        if String::from_utf8_lossy(&raw).contains(TOKEN) {
                            hits += 1;
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        (hits, looked)
    });
    (stop, handle)
}
