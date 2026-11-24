//! Actual child-process termination at test-only durability checkpoints.
#![cfg(unix)]
use super::*;
use serde_json::{Value, json};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn setup() -> (tempfile::TempDir, Value, Value) {
    let root = tempfile::tempdir().unwrap();
    let feature = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&feature).unwrap();
    std::fs::write(feature.join("tasks.md"), "- [ ] T001 Build\n").unwrap();
    let request = json!({"schema_version":1,"request_id":"71000000-0000-4000-8000-000000000001",
        "repository_id":"72000000-0000-4000-8000-000000000001","operation":"register",
        "payload":{"task":{"repository_id":"72000000-0000-4000-8000-000000000001","feature_id":"046-task-usage-ledger","task_id":"T001"},
        "phase":"builder","role":"builder","purpose":"implementation","configuration":{"agent_app":"codex","agent_version":"fixture-1","config_digest":format!("sha256:{}","a".repeat(64))}}});
    let registered = execute(root.path(), "register", request.to_string().as_bytes()).unwrap();
    (root, request, registered["result"].clone())
}
fn transition(registration: &Value, run: &Value) -> Value {
    json!({"schema_version":1,"request_id":"71000000-0000-4000-8000-000000000002","repository_id":registration["repository_id"],"operation":"transition",
        "payload":{"run_id":run["run_id"],"expected_generation":0,"state":"running"}})
}
fn snapshot_request(registration: &Value) -> Value {
    json!({"schema_version":1,"request_id":"71000000-0000-4000-8000-000000000003","repository_id":registration["repository_id"],"operation":"snapshot",
        "payload":{"expected_registration_sequence":1,"expected_usage_sequence":0,"predecessor_digest":null}})
}
struct KillChild(Child);
impl Drop for KillChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn terminate_at(root: &Path, operation: &str, request: &Value, boundary: &str) {
    let request_path = root.join("child-request.json");
    let marker = root.join("child-ready");
    let acknowledgement = root.join("child-ack");
    std::fs::write(&request_path, request.to_string()).unwrap();
    let mut child = KillChild(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "usage::process_crash_tests::child_process_fixture",
                "--nocapture",
            ])
            .env("AI_GENT_TEST_CHILD_ROOT", root)
            .env("AI_GENT_TEST_CHILD_OPERATION", operation)
            .env("AI_GENT_TEST_CHILD_BOUNDARY", boundary)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "child exited before {boundary}"
        );
        if std::fs::read_to_string(&marker).ok().as_deref() == Some(boundary) {
            break;
        }
        assert!(Instant::now() < deadline, "child did not reach {boundary}");
        std::thread::sleep(Duration::from_millis(5));
    }
    if boundary == "after_ack" {
        assert!(acknowledgement.is_file());
    } else {
        assert!(
            !acknowledgement.exists(),
            "interrupted operation must not acknowledge"
        );
    }
    child.0.kill().unwrap();
    let status = child.0.wait().unwrap();
    assert_eq!(
        status.signal(),
        Some(9),
        "parent must actually SIGKILL the child"
    );
}
#[test]
#[ignore = "subprocess fixture; invoked only by parent crash tests with a private synthetic root"]
fn child_process_fixture() {
    let root = PathBuf::from(std::env::var_os("AI_GENT_TEST_CHILD_ROOT").expect("fixture root"));
    let operation = std::env::var("AI_GENT_TEST_CHILD_OPERATION").unwrap();
    let boundary = std::env::var("AI_GENT_TEST_CHILD_BOUNDARY").unwrap();
    let request = std::fs::read(root.join("child-request.json")).unwrap();
    if boundary != "after_ack" {
        durable::checkpoint_once(&boundary, root.join("child-ready"));
    }
    let result = match operation.as_str() {
        "transition" => execute(&root, "transition", &request),
        "snapshot" => snapshot(&root, &request),
        "ingest" => ingest(&root, &request),
        "recover" => recover(&root, Some(&request), false),
        _ => panic!("unknown synthetic operation"),
    };
    result.unwrap();
    std::fs::write(root.join("child-ack"), b"acknowledged").unwrap();
    assert_eq!(
        boundary, "after_ack",
        "configured checkpoint was not reached"
    );
    std::fs::write(root.join("child-ready"), b"after_ack").unwrap();
    std::thread::sleep(Duration::from_secs(30));
    panic!("parent did not terminate acknowledged fixture");
}
#[test]
fn killed_append_preserves_previous_acknowledgement_and_idempotent_retry() {
    for boundary in [
        "append_write",
        "append_flush",
        "append_directory_sync",
        "after_ack",
    ] {
        let (root, registration, run) = setup();
        let request = transition(&registration, &run);
        terminate_at(root.path(), "transition", &request, boundary);
        let state = replay(root.path()).unwrap();
        assert_eq!(
            state["result"]["sequence"],
            if boundary == "append_write" { 1 } else { 2 }
        );
        execute(root.path(), "transition", request.to_string().as_bytes()).unwrap();
        execute(root.path(), "transition", request.to_string().as_bytes()).unwrap();
        assert_eq!(replay(root.path()).unwrap()["result"]["sequence"], 2);
    }
}
#[test]
fn killed_partial_append_requires_explicit_preserving_recovery() {
    let (root, registration, run) = setup();
    let request = transition(&registration, &run);
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let original = std::fs::read(&path).unwrap();
    terminate_at(root.path(), "transition", &request, "append_partial");
    let torn = std::fs::read(&path).unwrap();
    assert!(torn.starts_with(&original) && torn.len() > original.len());
    assert_eq!(replay(root.path()).unwrap_err().exit_code(), 5);
    assert_eq!(std::fs::read(&path).unwrap(), torn);
    let plan = recover(root.path(), None, true).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), torn);
    recover(
        root.path(),
        Some(plan["result"]["request"].to_string().as_bytes()),
        false,
    )
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    execute(root.path(), "transition", request.to_string().as_bytes()).unwrap();
    assert_eq!(replay(root.path()).unwrap()["result"]["sequence"], 2);
}
#[test]
fn killed_snapshot_publication_has_no_torn_final_object() {
    for boundary in [
        "publication_write",
        "publication_flush",
        "publication_link",
        "publication_directory_sync",
        "after_ack",
    ] {
        let (root, registration, _) = setup();
        let request = snapshot_request(&registration);
        terminate_at(root.path(), "snapshot", &request, boundary);
        let state = replay(root.path()).unwrap();
        assert_eq!(
            state["result"]["receipt_head"].is_string(),
            matches!(boundary, "publication_directory_sync" | "after_ack")
        );
        let published = snapshot(root.path(), request.to_string().as_bytes()).unwrap();
        assert_eq!(
            published,
            snapshot(root.path(), request.to_string().as_bytes()).unwrap()
        );
        let final_count = std::fs::read_dir(root.path().join(".ai-gent/usage/receipts"))
            .unwrap()
            .filter(|entry| {
                entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".json")
            })
            .count();
        assert_eq!(final_count, 1);
    }
}
#[test]
fn killed_recovery_retains_original_and_resumes_without_truncating_later_work() {
    for boundary in [
        "recovery_truncate",
        "recovery_flush",
        "recovery_directory_sync",
        "after_ack",
    ] {
        let (root, registration, run) = setup();
        let path = root
            .path()
            .join(".ai-gent/usage/journal/registrations.jsonl");
        let original = std::fs::read(&path).unwrap();
        let mut torn = original.clone();
        torn.extend_from_slice(b"incomplete");
        std::fs::write(&path, &torn).unwrap();
        let plan = recover(root.path(), None, true).unwrap();
        let request = &plan["result"]["request"];
        terminate_at(root.path(), "recover", request, boundary);
        let digest = plan["result"]["registration"]["original_digest"]
            .as_str()
            .unwrap()
            .strip_prefix("sha256:")
            .unwrap();
        assert_eq!(
            std::fs::read(
                root.path()
                    .join(".ai-gent/usage/quarantine")
                    .join(format!("{digest}.jsonl"))
            )
            .unwrap(),
            torn
        );
        recover(root.path(), Some(request.to_string().as_bytes()), false).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let transition = transition(&registration, &run);
        execute(root.path(), "transition", transition.to_string().as_bytes()).unwrap();
        let later = std::fs::read(&path).unwrap();
        recover(root.path(), Some(request.to_string().as_bytes()), false).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), later);
        assert_eq!(replay(root.path()).unwrap()["result"]["sequence"], 2);
    }
}

fn usage_request(registration: &Value, run: &Value, n: u32) -> Value {
    let mut identity = run["segments"][0]["configuration"].clone();
    identity["identity_quality"] = "unknown".into();
    json!({"schema_version":1,"request_id":format!("73000000-0000-4000-8000-{n:012}"),"repository_id":registration["repository_id"],"operation":"ingest", "payload":{
        "schema_version":1,"event_id":format!("74000000-0000-4000-8000-{n:012}"),"task":run["task"],
        "attempt_id":run["attempt_id"],"run_id":run["run_id"],"parent_run_id":run["parent_run_id"],"segment_id":run["segments"][0]["segment_id"],"phase":run["phase"],"sequence":n,
        "identity":identity,"source":{"adapter":"fixture","adapter_version":"1","namespace":"synthetic","request_id":format!("request-{n}"),"event_id":format!("event-{n}"),"scope":"turn_total","semantics":"inclusive-input-v1","reference":"evidence:fixture/usage","digest":format!("sha256:{}","b".repeat(64))},
        "raw_usage":{"input_tokens":1000,"output_tokens":200},"counters":{"input_total":1000,"cached_input":null,"cache_creation_input":null,"output_total":200,"reasoning_output":null},
        "quality":"complete","attribution":"exclusive","attestation":"local","contribution":"authoritative","supersedes":[],"extensions":{},"execution_scope":{"kind":"exclusive","beneficiaries":[run["task"].clone()]},
        "observed_at":"2026-11-18T00:00:00Z","occurred_at":null,"host_session_id":null
    }})
}
#[test]
fn killed_usage_append_preserves_acknowledged_tokens_and_deduplicates_retry() {
    for boundary in [
        "append_write",
        "append_flush",
        "append_directory_sync",
        "after_ack",
    ] {
        let (root, registration, run) = setup();
        let first = usage_request(&registration, &run, 1).to_string();
        ingest(root.path(), first.as_bytes()).unwrap();
        let second = usage_request(&registration, &run, 2);
        terminate_at(root.path(), "ingest", &second, boundary);
        let expected = if boundary == "append_write" {
            1200
        } else {
            2400
        };
        assert_eq!(
            replay(root.path()).unwrap()["result"]["usage"]["total_tokens"],
            expected
        );
        ingest(root.path(), second.to_string().as_bytes()).unwrap();
        ingest(root.path(), second.to_string().as_bytes()).unwrap();
        let state = replay(root.path()).unwrap();
        assert_eq!(state["result"]["usage_sequence"], 2);
        assert_eq!(state["result"]["usage"]["total_tokens"], 2400);
    }
}
#[test]
fn killed_partial_usage_append_keeps_acknowledged_tokens_after_explicit_recovery() {
    let (root, registration, run) = setup();
    ingest(
        root.path(),
        usage_request(&registration, &run, 1).to_string().as_bytes(),
    )
    .unwrap();
    let second = usage_request(&registration, &run, 2);
    let path = root
        .path()
        .join(".ai-gent/usage/journal/observations.jsonl");
    let original = std::fs::read(&path).unwrap();
    terminate_at(root.path(), "ingest", &second, "append_partial");
    let torn = std::fs::read(&path).unwrap();
    assert!(torn.starts_with(&original) && torn.len() > original.len());
    assert_eq!(replay(root.path()).unwrap_err().exit_code(), 5);
    assert_eq!(std::fs::read(&path).unwrap(), torn);
    let plan = recover(root.path(), None, true).unwrap();
    recover(
        root.path(),
        Some(plan["result"]["request"].to_string().as_bytes()),
        false,
    )
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(
        replay(root.path()).unwrap()["result"]["usage"]["total_tokens"],
        1200
    );
    ingest(root.path(), second.to_string().as_bytes()).unwrap();
    ingest(root.path(), second.to_string().as_bytes()).unwrap();
    assert_eq!(
        replay(root.path()).unwrap()["result"]["usage"]["total_tokens"],
        2400
    );
}
