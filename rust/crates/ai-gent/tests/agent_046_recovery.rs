//! Recovery/receipt boundaries use synthetic repositories and the actual CLI.
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
fn run(root: &Path, action: &str, input: Option<&Value>, dry_run: bool) -> (i32, Value) {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("ai-gent"));
    command
        .args(["--json", "usage", "--root"])
        .arg(root)
        .arg(action)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if dry_run {
        command.arg("--dry-run");
    }
    if input.is_some() {
        command.args(["--input", "-"]).stdin(Stdio::piped());
    }
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    (output.status.code().unwrap(), value)
}
fn setup() -> (tempfile::TempDir, Value) {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tasks.md"), "- [ ] T001 Build\n").unwrap();
    let registration: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    assert_eq!(
        run(root.path(), "register", Some(&registration), false).0,
        0
    );
    (root, registration)
}
fn snapshot(registration: &Value, request: &str, predecessor: &Value, sequence: u64) -> Value {
    json!({"schema_version":1,"request_id":request,"repository_id":registration["repository_id"],"operation":"snapshot","payload":{"expected_registration_sequence":sequence,"expected_usage_sequence":0,"predecessor_digest":predecessor}})
}
#[test]
fn snapshot_is_content_addressed_immutable_and_never_accepts_a_task() {
    let (root, registration) = setup();
    let request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        1,
    );
    let (code, first) = run(root.path(), "snapshot", Some(&request), false);
    assert_eq!(code, 0, "{first}");
    assert_eq!(first["result"]["receipt"]["kind"], "usage_snapshot");
    assert_eq!(first["result"]["receipt"]["assurance"], "local_unverified");
    assert_eq!(first["result"]["receipt"]["coverage"], "unclosed");
    assert!(first["result"]["receipt"].get("accepted").is_none());
    let digest = first["result"]["receipt_digest"].as_str().unwrap();
    let file = root
        .path()
        .join(".ai-gent/usage/receipts")
        .join(format!("{}.json", digest.strip_prefix("sha256:").unwrap()));
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(
        run(root.path(), "snapshot", Some(&request), false),
        (0, first.clone())
    );
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    let mut conflict = request;
    conflict["payload"]["expected_usage_sequence"] = 1.into();
    assert_eq!(run(root.path(), "snapshot", Some(&conflict), false).0, 4);
}
#[test]
fn receipt_predecessor_and_sequences_prevent_forks_and_stale_publication() {
    let (root, registration) = setup();
    let request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        1,
    );
    let first = run(root.path(), "snapshot", Some(&request), false);
    assert_eq!(first.0, 0, "{}", first.1);
    let mut stale = request.clone();
    stale["request_id"] = "90000000-0000-4000-8000-000000000002".into();
    assert_eq!(run(root.path(), "snapshot", Some(&stale), false).0, 4);
    stale["payload"]["predecessor_digest"] = first.1["result"]["receipt_digest"].clone();
    let second = run(root.path(), "snapshot", Some(&stale), false);
    assert_eq!(second.0, 0, "{}", second.1);
    assert_eq!(
        second.1["result"]["receipt"]["predecessor_digest"],
        first.1["result"]["receipt_digest"]
    );
}
#[test]
fn dry_run_plans_torn_tail_recovery_without_modifying_evidence() {
    let (root, _) = setup();
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let original = std::fs::read(&path).unwrap();
    let mut damaged = original.clone();
    damaged.extend_from_slice(b"{\"schema_version\":");
    std::fs::write(&path, &damaged).unwrap();
    assert_eq!(run(root.path(), "replay", None, false).0, 5);
    let plan = run(root.path(), "recover", None, true);
    assert_eq!(plan.0, 0, "{}", plan.1);
    assert_eq!(std::fs::read(&path).unwrap(), damaged);
    assert!(!root.path().join(".ai-gent/usage/quarantine").exists());
    assert_eq!(
        plan.1["result"]["registration"]["discarded_tail_bytes"],
        damaged.len() - original.len()
    );
    let apply = plan.1["result"]["request"].clone();
    let recovered = run(root.path(), "recover", Some(&apply), false);
    assert_eq!(recovered.0, 0, "{}", recovered.1);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let original_digest = plan.1["result"]["registration"]["original_digest"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let copy = root
        .path()
        .join(".ai-gent/usage/quarantine")
        .join(format!("{original_digest}.jsonl"));
    assert_eq!(std::fs::read(copy).unwrap(), damaged);
    assert_eq!(run(root.path(), "replay", None, false).0, 0);
    assert_eq!(run(root.path(), "recover", Some(&apply), false), recovered);
}
#[test]
fn interior_corruption_cannot_be_hidden_by_torn_tail_repair() {
    let (root, _) = setup();
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let mut damaged = std::fs::read(&path).unwrap();
    damaged[0] = b'!';
    damaged.extend_from_slice(b"partial");
    std::fs::write(&path, &damaged).unwrap();
    let result = run(root.path(), "recover", None, true);
    assert_eq!(result.0, 5, "{}", result.1);
    assert_eq!(std::fs::read(&path).unwrap(), damaged);
}

#[test]
fn recovery_retry_after_later_work_does_not_truncate_new_records() {
    let (root, registration) = setup();
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend_from_slice(b"partial");
    std::fs::write(&path, bytes).unwrap();
    let plan = run(root.path(), "recover", None, true);
    let request = plan.1["result"]["request"].clone();
    let first = run(root.path(), "recover", Some(&request), false);
    assert_eq!(first.0, 0, "{}", first.1);
    let mut later = registration;
    later["request_id"] = "91000000-0000-4000-8000-000000000001".into();
    assert_eq!(run(root.path(), "register", Some(&later), false).0, 0);
    let before = std::fs::read(&path).unwrap();
    assert_eq!(run(root.path(), "recover", Some(&request), false), first);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(
        run(root.path(), "replay", None, false).1["result"]["sequence"],
        2
    );
}
#[test]
fn changed_evidence_after_dry_run_requires_a_new_explicit_plan() {
    let (root, _) = setup();
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend_from_slice(b"partial");
    std::fs::write(&path, &bytes).unwrap();
    let plan = run(root.path(), "recover", None, true);
    let request = plan.1["result"]["request"].clone();
    bytes.extend_from_slice(b"changed");
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(run(root.path(), "recover", Some(&request), false).0, 4);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(!root.path().join(".ai-gent/usage/quarantine").exists());
}
#[test]
fn corrupt_receipt_is_preserved_and_blocks_replay_and_publication() {
    let (root, registration) = setup();
    let request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        1,
    );
    let first = run(root.path(), "snapshot", Some(&request), false);
    assert_eq!(first.0, 0, "{}", first.1);
    let digest = first.1["result"]["receipt_digest"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let path = root
        .path()
        .join(".ai-gent/usage/receipts")
        .join(format!("{digest}.json"));
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] = b'!';
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(run(root.path(), "replay", None, false).0, 5);
    assert_eq!(run(root.path(), "snapshot", Some(&request), false).0, 5);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
#[test]
fn concurrent_snapshot_retries_publish_one_immutable_receipt() {
    let (root, registration) = setup();
    let request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        1,
    );
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let root = root.path().to_path_buf();
            let request = request.clone();
            std::thread::spawn(move || run(&root, "snapshot", Some(&request), false))
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert!(
        results
            .iter()
            .all(|result| result.0 == 0 && result == &results[0]),
        "{results:?}"
    );
    assert_eq!(
        std::fs::read_dir(root.path().join(".ai-gent/usage/receipts"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn recovery_cannot_discard_records_already_bound_by_a_published_snapshot() {
    let (root, registration) = setup();
    let mut later = registration.clone();
    later["request_id"] = "91000000-0000-4000-8000-000000000001".into();
    assert_eq!(run(root.path(), "register", Some(&later), false).0, 0);
    let request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        2,
    );
    let first = run(root.path(), "snapshot", Some(&request), false);
    assert_eq!(first.0, 0, "{}", first.1);
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let mut bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes.pop(), Some(b'\n'));
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(run(root.path(), "recover", None, true).0, 5);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(!root.path().join(".ai-gent/usage/quarantine").exists());
}

#[test]
fn request_ids_cannot_move_between_snapshot_recovery_and_registration() {
    let (root, registration) = setup();
    let request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        1,
    );
    assert_eq!(run(root.path(), "snapshot", Some(&request), false).0, 0);
    let mut reused = registration;
    reused["request_id"] = request["request_id"].clone();
    assert_eq!(run(root.path(), "register", Some(&reused), false).0, 4);
    let plan = run(root.path(), "recover", None, true);
    assert_eq!(plan.0, 0, "{}", plan.1);
    let mut recovery = plan.1["result"]["request"].clone();
    let original_id = recovery["request_id"].clone();
    recovery["request_id"] = request["request_id"].clone();
    assert_eq!(run(root.path(), "recover", Some(&recovery), false).0, 4);
    recovery["request_id"] = original_id.clone();
    assert_eq!(run(root.path(), "recover", Some(&recovery), false).0, 0);
    reused["request_id"] = original_id;
    assert_eq!(run(root.path(), "register", Some(&reused), false).0, 4);
}
#[test]
fn malformed_snapshot_requests_fail_before_any_publication() {
    let (root, registration) = setup();
    let request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        1,
    );
    let mut missing = request.clone();
    missing["payload"]
        .as_object_mut()
        .unwrap()
        .remove("predecessor_digest");
    assert_eq!(run(root.path(), "snapshot", Some(&missing), false).0, 2);
    let mut version = request.clone();
    version["schema_version"] = 2.into();
    assert_eq!(run(root.path(), "snapshot", Some(&version), false).0, 2);
    let mut nil = request;
    nil["request_id"] = "00000000-0000-0000-0000-000000000000".into();
    assert_eq!(run(root.path(), "snapshot", Some(&nil), false).0, 2);
    assert!(!root.path().join(".ai-gent/usage/receipts").exists());
}

#[test]
fn snapshot_retry_cannot_acknowledge_receipt_after_bound_records_are_lost() {
    let (root, registration) = setup();
    let path = root
        .path()
        .join(".ai-gent/usage/journal/registrations.jsonl");
    let first_frame = std::fs::read(&path).unwrap();
    let mut later = registration.clone();
    later["request_id"] = "91000000-0000-4000-8000-000000000001".into();
    assert_eq!(run(root.path(), "register", Some(&later), false).0, 0);
    let request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        2,
    );
    let published = run(root.path(), "snapshot", Some(&request), false);
    assert_eq!(published.0, 0, "{}", published.1);
    std::fs::write(&path, &first_frame).unwrap();
    assert_eq!(run(root.path(), "replay", None, false).0, 5);
    let retry = run(root.path(), "snapshot", Some(&request), false);
    assert_eq!(retry.0, 5, "{}", retry.1);
    assert_eq!(retry.1["error"]["code"], "evidence_corrupt");
    assert_eq!(std::fs::read(&path).unwrap(), first_frame);
}

#[test]
fn snapshot_retry_cannot_acknowledge_receipt_after_usage_prefix_is_lost() {
    let (root, registration) = setup();
    let replay = run(root.path(), "replay", None, false);
    let registered = &replay.1["result"]["runs"][0];
    let mut observation: Value = serde_json::from_str(include_str!(
        "../../../..//specs/046-task-usage-ledger/contracts/examples/usage-valid.json"
    ))
    .unwrap();
    for field in [
        "task",
        "attempt_id",
        "run_id",
        "parent_run_id",
        "phase",
        "host_session_id",
    ] {
        observation[field] = registered[field].clone();
    }
    observation["segment_id"] = registered["segments"][0]["segment_id"].clone();
    observation["identity"] = registered["segments"][0]["configuration"].clone();
    observation["identity"]["identity_quality"] = "reported".into();
    observation["execution_scope"]["beneficiaries"] = json!([registered["task"].clone()]);
    let ingest = json!({"schema_version":1,"request_id":"92000000-0000-4000-8000-000000000001",
        "repository_id":registration["repository_id"],"operation":"ingest","payload":observation});
    let ingested = run(root.path(), "ingest", Some(&ingest), false);
    assert_eq!(ingested.0, 0, "{}", ingested.1);
    let mut request = snapshot(
        &registration,
        "90000000-0000-4000-8000-000000000001",
        &Value::Null,
        1,
    );
    request["payload"]["expected_usage_sequence"] = 1.into();
    let published = run(root.path(), "snapshot", Some(&request), false);
    assert_eq!(published.0, 0, "{}", published.1);
    let path = root
        .path()
        .join(".ai-gent/usage/journal/observations.jsonl");
    std::fs::write(&path, []).unwrap();
    assert_eq!(run(root.path(), "replay", None, false).0, 5);
    let retry = run(root.path(), "snapshot", Some(&request), false);
    assert_eq!(retry.0, 5, "{}", retry.1);
    assert_eq!(retry.1["error"]["code"], "evidence_corrupt");
    assert!(std::fs::read(&path).unwrap().is_empty());
}
