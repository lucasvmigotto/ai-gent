//! Registration contract: real CLI and filesystem, no model calls or credentials.
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn request() -> Value {
    serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap()
}
fn project() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("tasks.md"),
        "# Tasks\n- [ ] T001 Build\n- [ ] T002 Review\n",
    )
    .unwrap();
    d
}
fn run(root: &Path, action: &str, input: Option<&Value>) -> (i32, Value) {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin("ai-gent"));
    c.args(["--json", "usage", "--root"])
        .arg(root)
        .args(["--protocol-version", "1", action])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if input.is_some() {
        c.args(["--input", "-"]).stdin(Stdio::piped());
    }
    let mut child = c.spawn().unwrap();
    if let Some(v) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(v.to_string().as_bytes())
            .unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let v = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| panic!("stdout {:?}; stderr {:?}", out.stdout, out.stderr));
    (out.status.code().unwrap(), v)
}
#[test]
fn identity_and_segment_boundaries_positive() {
    let project_dir = project();
    let registration = request();
    let (code, first) = run(project_dir.path(), "register", Some(&registration));
    assert_eq!(code, 0, "{first}");
    let r = &first["result"];
    assert_eq!(r["task"], registration["payload"]["task"]);
    assert_eq!(r["usage_quality"], "missing");
    assert_eq!(r["state"], "registered");
    assert_eq!(
        r["segments"][0]["configuration"]["effective_model"],
        Value::Null
    );
    assert_eq!(
        run(project_dir.path(), "register", Some(&registration)).1,
        first
    );
    let mut second = registration.clone();
    second["request_id"] = "10000000-0000-4000-8000-000000000002".into();
    let other = run(project_dir.path(), "register", Some(&second)).1;
    assert_ne!(other["result"]["attempt_id"], r["attempt_id"]);
    assert_ne!(other["result"]["run_id"], r["run_id"]);
    let mut config = registration["payload"]["configuration"].clone();
    config["effective_provider"] = "anthropic".into();
    config["effective_model"] = "fixture-fallback".into();
    let seg = json!({"schema_version":1,"request_id":"10000000-0000-4000-8000-000000000003",
        "repository_id":registration["repository_id"],"operation":"segment",
        "payload":{"run_id":r["run_id"],"expected_generation":0,"configuration":config}});
    let (code, changed) = run(project_dir.path(), "segment", Some(&seg));
    assert_eq!(code, 0, "{changed}");
    let segments = changed["result"]["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0], r["segments"][0]);
    assert_ne!(segments[0]["segment_id"], segments[1]["segment_id"]);
    assert_eq!(segments[1]["identity_quality"], "reported");
    let replay = run(project_dir.path(), "replay", None);
    assert_eq!(replay.0, 0, "{}", replay.1);
    assert_eq!(replay.1["result"]["sequence"], 3);
}
#[test]
fn identity_and_segment_boundaries_negative() {
    let project_dir = project();
    let registration = request();
    let first = run(project_dir.path(), "register", Some(&registration));
    assert_eq!(first.0, 0, "{}", first.1);
    let mut conflict = registration.clone();
    conflict["payload"]["configuration"]["agent_app"] = "claude".into();
    let bad = run(project_dir.path(), "register", Some(&conflict));
    assert_eq!(bad.0, 4, "{}", bad.1);
    assert_eq!(bad.1["error"]["code"], "identity_conflict");
    let mut unknown = registration.clone();
    unknown["request_id"] = "10000000-0000-4000-8000-000000000002".into();
    unknown["payload"]["task"]["task_id"] = "T999".into();
    assert_eq!(run(project_dir.path(), "register", Some(&unknown)).0, 3);
    let mut other = registration.clone();
    other["repository_id"] = "20000000-0000-4000-8000-000000000002".into();
    assert_eq!(run(project_dir.path(), "register", Some(&other)).0, 4);
    assert_eq!(
        run(project_dir.path(), "replay", None).1["result"]["sequence"],
        1
    );
}
#[test]
fn scoped_identity_does_not_infer_repository_or_host() {
    let project_dir = project();
    let p2 = project();
    let a = run(project_dir.path(), "register", Some(&request())).1;
    let mut registration = request();
    registration["repository_id"] = "20000000-0000-4000-8000-000000000002".into();
    registration["payload"]["task"]["repository_id"] = registration["repository_id"].clone();
    registration["payload"]["configuration"]["agent_app"] = "opencode".into();
    let (code, b) = run(p2.path(), "register", Some(&registration));
    assert_eq!(code, 0, "{b}");
    assert_ne!(a["result"]["task"], b["result"]["task"]);
    assert_ne!(a["result"]["run_id"], b["result"]["run_id"]);
}
#[test]
fn unknown_fields_versions_and_parent_runs_fail_without_registration() {
    let project_dir = project();
    for field in ["schema_version", "parent_run_id", "prompt"] {
        let mut registration = request();
        match field {
            "schema_version" => registration[field] = 2.into(),
            "parent_run_id" => {
                registration["payload"][field] = "10000000-0000-4000-8000-000000000099".into();
            }
            _ => registration["payload"][field] = "private text must not be persisted".into(),
        }
        let bad = run(project_dir.path(), "register", Some(&registration));
        assert_ne!(bad.0, 0, "{}", bad.1);
    }
    assert_eq!(
        run(project_dir.path(), "replay", None).1["result"]["sequence"],
        0
    );
}

fn next_request(registration: &mut Value, n: u32) {
    registration["request_id"] = format!("10000000-0000-4000-8000-{n:012}").into();
}
fn journal(root: &Path) -> std::path::PathBuf {
    root.join(".ai-gent/usage/journal/registrations.jsonl")
}
#[test]
fn terminal_state_retains_identity_and_cannot_become_task_acceptance() {
    let project_dir = project();
    let registration = request();
    let r = run(project_dir.path(), "register", Some(&registration)).1["result"].clone();
    let mut t = json!({"schema_version":1,"request_id":"10000000-0000-4000-8000-000000000002","repository_id":registration["repository_id"],"operation":"transition","payload":{"run_id":r["run_id"],"expected_generation":0,"state":"running"}});
    let started = run(project_dir.path(), "transition", Some(&t));
    assert_eq!(started.0, 0, "{}", started.1);
    next_request(&mut t, 3);
    t["payload"]["expected_generation"] = 1.into();
    t["payload"]["state"] = "finished".into();
    let finished = run(project_dir.path(), "transition", Some(&t));
    assert_eq!(finished.0, 0, "{}", finished.1);
    assert_eq!(finished.1["result"]["usage_quality"], "missing");
    assert_eq!(finished.1["result"]["attempt_id"], r["attempt_id"]);
    next_request(&mut t, 4);
    t["payload"]["expected_generation"] = 2.into();
    t["payload"]["state"] = "accepted".into();
    assert_eq!(run(project_dir.path(), "transition", Some(&t)).0, 2);
    t["payload"]["state"] = "running".into();
    assert_eq!(run(project_dir.path(), "transition", Some(&t)).0, 3);
    let mut s = json!({"schema_version":1,"request_id":"10000000-0000-4000-8000-000000000005","repository_id":registration["repository_id"],"operation":"segment","payload":{"run_id":r["run_id"],"expected_generation":2,"configuration":registration["payload"]["configuration"]}});
    s["payload"]["configuration"]["requested_model"] = "another".into();
    assert_eq!(run(project_dir.path(), "segment", Some(&s)).0, 3);
    assert_eq!(
        run(project_dir.path(), "replay", None).1["result"]["sequence"],
        3
    );
}
#[test]
fn shared_attempt_parent_scope_and_stale_generation_are_checked() {
    let project_dir = project();
    let registration = request();
    let a = run(project_dir.path(), "register", Some(&registration)).1["result"].clone();
    let mut child = registration.clone();
    next_request(&mut child, 2);
    child["payload"]["attempt_id"] = a["attempt_id"].clone();
    child["payload"]["parent_run_id"] = a["run_id"].clone();
    child["payload"]["phase"] = "reviewer".into();
    child["payload"]["role"] = "reviewer".into();
    child["payload"]["purpose"] = "review".into();
    let b = run(project_dir.path(), "register", Some(&child));
    assert_eq!(b.0, 0, "{}", b.1);
    assert_eq!(b.1["result"]["attempt_id"], a["attempt_id"]);
    assert_ne!(b.1["result"]["run_id"], a["run_id"]);
    next_request(&mut child, 3);
    child["payload"]["task"]["task_id"] = "T002".into();
    assert_eq!(run(project_dir.path(), "register", Some(&child)).0, 4);
    let stale = json!({"schema_version":1,"request_id":"10000000-0000-4000-8000-000000000004","repository_id":registration["repository_id"],"operation":"transition","payload":{"run_id":a["run_id"],"expected_generation":99,"state":"running"}});
    assert_eq!(run(project_dir.path(), "transition", Some(&stale)).0, 4);
    let mut retry = registration.clone();
    next_request(&mut retry, 5);
    retry["payload"]["parent_attempt_id"] = a["attempt_id"].clone();
    retry["payload"]["purpose"] = "retry".into();
    let c = run(project_dir.path(), "register", Some(&retry));
    assert_eq!(c.0, 0, "{}", c.1);
    assert_eq!(c.1["result"]["parent_attempt_id"], a["attempt_id"]);
    assert_eq!(c.1["result"]["attempt_ordinal"], 2);
}
#[test]
fn retry_after_task_change_returns_original_ack_but_new_run_cannot_reuse_changed_attempt() {
    let project_dir = project();
    let registration = request();
    let first = run(project_dir.path(), "register", Some(&registration));
    assert_eq!(first.0, 0, "{}", first.1);
    std::fs::write(
        project_dir
            .path()
            .join("specs/046-task-usage-ledger/tasks.md"),
        "- [ ] T001 Changed meaning\n",
    )
    .unwrap();
    assert_eq!(
        run(project_dir.path(), "register", Some(&registration)).1,
        first.1
    );
    let mut new = registration.clone();
    next_request(&mut new, 2);
    new["payload"]["attempt_id"] = first.1["result"]["attempt_id"].clone();
    assert_eq!(run(project_dir.path(), "register", Some(&new)).0, 4);
    assert_eq!(run(project_dir.path(), "replay", None).0, 0);
}
#[test]
fn duplicate_run_id_and_undeclared_task_fields_are_rejected() {
    let project_dir = project();
    let registration = request();
    let a = run(project_dir.path(), "register", Some(&registration));
    assert_eq!(a.0, 0, "{}", a.1);
    let mut duplicate = registration.clone();
    next_request(&mut duplicate, 2);
    duplicate["payload"]["run_id"] = a.1["result"]["run_id"].clone();
    assert_eq!(run(project_dir.path(), "register", Some(&duplicate)).0, 4);
    for bad in ["../escape", "046-../../escape", "046-α", "046--bad"] {
        let mut registration = request();
        registration["payload"]["task"]["feature_id"] = bad.into();
        assert_eq!(
            run(project_dir.path(), "register", Some(&registration)).0,
            2
        );
    }
    let mut verified = request();
    verified["payload"]["configuration"]["identity_quality"] = "verified".into();
    assert_eq!(run(project_dir.path(), "register", Some(&verified)).0, 2);
}
#[test]
fn sixteen_competing_processes_receive_one_durable_registration_for_one_request() {
    let project_dir = project();
    let root = project_dir.path().to_path_buf();
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || run(&root, "register", Some(&request())))
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    for (code, value) in &results {
        assert_eq!(*code, 0, "{value}");
        assert_eq!(*value, results[0].1);
    }
    assert_eq!(
        run(project_dir.path(), "replay", None).1["result"]["sequence"],
        1
    );
}
#[test]
fn corrupt_or_torn_evidence_blocks_writes_and_is_preserved() {
    for torn in [false, true] {
        let project_dir = project();
        assert_eq!(run(project_dir.path(), "register", Some(&request())).0, 0);
        let log = journal(project_dir.path());
        let mut data = std::fs::read(&log).unwrap();
        if torn {
            data.extend(b"{partial");
        } else {
            let mut v: Value = serde_json::from_slice(&data).unwrap();
            v["payload"]["result"]["segments"][0]["configuration"]["agent_app"] = "forged".into();
            data = v.to_string().into_bytes();
            data.push(b'\n');
        }
        std::fs::write(&log, &data).unwrap();
        let bad = run(project_dir.path(), "replay", None);
        assert_eq!(bad.0, 5, "{}", bad.1);
        assert_eq!(bad.1["error"]["code"], "evidence_corrupt");
        let mut registration = request();
        next_request(&mut registration, 2);
        assert_eq!(
            run(project_dir.path(), "register", Some(&registration)).0,
            5
        );
        assert_eq!(std::fs::read(log).unwrap(), data);
    }
}
#[test]
fn readonly_replay_creates_nothing_and_storage_failure_never_acknowledges() {
    let project_dir = project();
    assert_eq!(run(project_dir.path(), "replay", None).0, 0);
    assert!(!project_dir.path().join(".ai-gent").exists());
    std::fs::create_dir_all(journal(project_dir.path())).unwrap();
    assert_ne!(run(project_dir.path(), "register", Some(&request())).0, 0);
    assert!(journal(project_dir.path()).is_dir());
}
#[cfg(unix)]
#[test]
fn private_permissions_and_symlink_boundaries() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let project_dir = project();
    assert_eq!(run(project_dir.path(), "register", Some(&request())).0, 0);
    assert_eq!(
        std::fs::metadata(journal(project_dir.path()))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(project_dir.path().join(".ai-gent/usage"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let outside = tempfile::tempdir().unwrap();
    let p2 = project();
    symlink(outside.path(), p2.path().join(".ai-gent")).unwrap();
    assert_eq!(run(p2.path(), "register", Some(&request())).0, 4);
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn sixteen_independent_registrations_serialize_attempt_ordinals_without_id_collisions() {
    let directory = project();
    let root = directory.path().to_path_buf();
    let threads: Vec<_> = (1..=16)
        .map(|n| {
            let root = root.clone();
            std::thread::spawn(move || {
                let mut input = request();
                next_request(&mut input, n);
                run(&root, "register", Some(&input))
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    let mut ids = std::collections::BTreeSet::new();
    let mut ordinals = std::collections::BTreeSet::new();
    for (code, value) in results {
        assert_eq!(code, 0, "{value}");
        ids.insert(value["result"]["run_id"].as_str().unwrap().to_owned());
        ordinals.insert(value["result"]["attempt_ordinal"].as_u64().unwrap());
    }
    assert_eq!(ids.len(), 16);
    assert_eq!(ordinals, (1..=16).collect());
    assert_eq!(
        run(directory.path(), "replay", None).1["result"]["sequence"],
        16
    );
}

#[test]
fn journals_in_other_projects_are_ignored_without_editing_their_root_configuration() {
    let directory = project();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .arg(directory.path())
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(run(directory.path(), "register", Some(&request())).0, 0);
    assert!(!directory.path().join(".gitignore").exists());
    assert!(
        Command::new("git")
            .current_dir(directory.path())
            .args([
                "check-ignore",
                "-q",
                ".ai-gent/usage/journal/registrations.jsonl"
            ])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .current_dir(directory.path())
            .args(["check-ignore", "-q", ".ai-gent/usage/.gitignore"])
            .status()
            .unwrap()
            .success()
    );
}
