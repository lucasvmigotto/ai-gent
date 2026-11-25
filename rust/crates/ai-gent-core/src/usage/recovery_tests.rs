//! Fault checkpoints are test-only, thread-local and inaccessible to runtime callers.
use super::*;
use serde_json::{Value, json};
fn setup() -> (tempfile::TempDir, Value, Value) {
    let root = tempfile::tempdir().unwrap();
    let feature = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&feature).unwrap();
    std::fs::write(feature.join("tasks.md"), "- [ ] T001 Build\n").unwrap();
    let request = json!({"schema_version":1,"request_id":"10000000-0000-4000-8000-000000000001",
        "repository_id":"20000000-0000-4000-8000-000000000001","operation":"register",
        "payload":{"task":{"repository_id":"20000000-0000-4000-8000-000000000001","feature_id":"046-task-usage-ledger","task_id":"T001"},
        "phase":"builder","role":"builder","purpose":"implementation","configuration":{"agent_app":"codex","agent_version":"fixture-1","config_digest":format!("sha256:{}","a".repeat(64))}}});
    let result = execute(root.path(), "register", request.to_string().as_bytes()).unwrap();
    (root, request, result["result"].clone())
}
fn transition(registration: &Value, run: &Value) -> Value {
    json!({"schema_version":1,"request_id":"10000000-0000-4000-8000-000000000002","repository_id":registration["repository_id"],"operation":"transition",
        "payload":{"run_id":run["run_id"],"expected_generation":0,"state":"running"}})
}
fn snapshot_request(registration: &Value) -> Value {
    json!({"schema_version":1,"request_id":"10000000-0000-4000-8000-000000000003","repository_id":registration["repository_id"],"operation":"snapshot",
        "payload":{"expected_registration_sequence":1,"expected_usage_sequence":0,"predecessor_digest":null}})
}
#[test]
fn failed_append_write_flush_and_directory_sync_never_acknowledge_success() {
    for boundary in ["append_write", "append_flush", "append_directory_sync"] {
        let (root, registration, registered) = setup();
        let request = transition(&registration, &registered);
        durable::fail_once(boundary);
        let error = execute(root.path(), "transition", request.to_string().as_bytes()).unwrap_err();
        assert_eq!(error.exit_code(), 5, "{boundary}");
        let before_retry = replay(root.path()).unwrap()["result"]["sequence"]
            .as_u64()
            .unwrap();
        assert_eq!(before_retry, if boundary == "append_write" { 1 } else { 2 });
        execute(root.path(), "transition", request.to_string().as_bytes()).unwrap();
        assert_eq!(replay(root.path()).unwrap()["result"]["sequence"], 2);
    }
}
#[test]
fn partial_append_is_preserved_until_explicit_recovery_then_retry_is_exactly_once() {
    let (root, registration, registered) = setup();
    let request = transition(&registration, &registered);
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let original = std::fs::read(&path).unwrap();
    durable::fail_once("append_partial");
    assert_eq!(
        execute(root.path(), "transition", request.to_string().as_bytes())
            .unwrap_err()
            .exit_code(),
        5
    );
    let torn = std::fs::read(&path).unwrap();
    assert!(torn.len() > original.len());
    assert_eq!(replay(root.path()).unwrap_err().exit_code(), 5);
    let plan = recover(root.path(), None, true).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), torn);
    let recovery_request = plan["result"]["request"].to_string();
    recover(root.path(), Some(recovery_request.as_bytes()), false).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    execute(root.path(), "transition", request.to_string().as_bytes()).unwrap();
    execute(root.path(), "transition", request.to_string().as_bytes()).unwrap();
    assert_eq!(replay(root.path()).unwrap()["result"]["sequence"], 2);
}
#[test]
fn every_publication_failure_leaves_either_no_receipt_or_a_complete_retryable_receipt() {
    for boundary in [
        "publication_write",
        "publication_flush",
        "publication_link",
        "publication_directory_sync",
    ] {
        let (root, registration, _) = setup();
        let request = snapshot_request(&registration).to_string();
        durable::fail_once(boundary);
        assert_eq!(
            snapshot(root.path(), request.as_bytes())
                .unwrap_err()
                .exit_code(),
            5,
            "{boundary}"
        );
        let before_retry = replay(root.path()).unwrap();
        let head = &before_retry["result"]["receipt_head"];
        assert_eq!(head.is_string(), boundary == "publication_directory_sync");
        let first = snapshot(root.path(), request.as_bytes()).unwrap();
        assert_eq!(first, snapshot(root.path(), request.as_bytes()).unwrap());
        assert_eq!(first["result"]["receipt"]["assurance"], "local_unverified");
    }
}
#[test]
fn recovery_failure_before_or_after_truncation_keeps_durable_original_and_can_resume() {
    for boundary in [
        "publication_write",
        "recovery_truncate",
        "recovery_flush",
        "recovery_directory_sync",
    ] {
        let (root, _, _) = setup();
        let path = root
            .path()
            .join(".ai-gent/usage/journal/registrations.jsonl");
        let original = std::fs::read(&path).unwrap();
        let mut torn = original.clone();
        torn.extend_from_slice(b"partial");
        std::fs::write(&path, &torn).unwrap();
        let plan = recover(root.path(), None, true).unwrap();
        let request = plan["result"]["request"].to_string();
        durable::fail_once(boundary);
        assert_eq!(
            recover(root.path(), Some(request.as_bytes()), false)
                .unwrap_err()
                .exit_code(),
            5,
            "{boundary}"
        );
        let current = std::fs::read(&path).unwrap();
        assert!(current == original || current == torn);
        let result = recover(root.path(), Some(request.as_bytes()), false).unwrap();
        assert_eq!(
            result,
            recover(root.path(), Some(request.as_bytes()), false).unwrap()
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
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
    }
}
#[test]
fn bounded_lock_wait_returns_retryable_error_without_mutation() {
    let (root, registration, registered) = setup();
    let store = journal::Store::open(root.path(), false).unwrap();
    let lock = store.lock(false).unwrap();
    let request = transition(&registration, &registered);
    let error = execute(root.path(), "transition", request.to_string().as_bytes()).unwrap_err();
    assert_eq!(error.code, "lock_timeout");
    assert!(error.retryable);
    drop(lock);
    assert_eq!(replay(root.path()).unwrap()["result"]["sequence"], 1);
}
