//! Existing receipt objects bind different and repeated prefixes of both journals.
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn run(root: &Path, action: &str, input: Option<&Value>) -> (i32, Value) {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("ai-gent"));
    command
        .args(["--json", "usage", "--root"])
        .arg(root)
        .arg(action)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
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
fn usage(registered: &Value, n: u32) -> Value {
    let mut observation: Value = serde_json::from_str(include_str!(
        "../../../..//specs/046-task-usage-ledger/contracts/examples/usage-valid.json"
    ))
    .unwrap();
    for field in ["task", "attempt_id", "run_id", "parent_run_id", "phase"] {
        observation[field] = registered[field].clone();
    }
    observation["event_id"] = format!("35000000-0000-4000-8000-{n:012}").into();
    observation["segment_id"] = registered["segments"][0]["segment_id"].clone();
    observation["sequence"] = n.into();
    observation["identity"] = registered["segments"][0]["configuration"].clone();
    observation["identity"]["identity_quality"] = "reported".into();
    observation["source"]["event_id"] = format!("event-{n}").into();
    observation["source"]["request_id"] = format!("request-{n}").into();
    observation["execution_scope"]["beneficiaries"] = json!([registered["task"].clone()]);
    observation["host_session_id"] = Value::Null;
    json!({"schema_version":1,"request_id":format!("45000000-0000-4000-8000-{n:012}"),"repository_id":registered["task"]["repository_id"],"operation":"ingest","payload":observation})
}
#[test]
fn interleaved_and_repeated_receipt_prefixes_keep_old_objects_and_reject_lost_evidence() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tasks.md"), "- [ ] T001 Build\n").unwrap();
    let registration: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    let (code, response) = run(root.path(), "register", Some(&registration));
    assert_eq!(code, 0, "{response}");
    let registered = &response["result"];
    let mut predecessor = Value::Null;
    let mut snapshots = Vec::new();
    for (index, (registration_sequence, usage_sequence)) in [(1, 0), (1, 1), (2, 1), (2, 2), (2, 2)]
        .into_iter()
        .enumerate()
    {
        if index == 1 || index == 3 {
            let observation = usage(registered, if index == 1 { 1 } else { 2 });
            let result = run(root.path(), "ingest", Some(&observation));
            assert_eq!(result.0, 0, "{}", result.1);
        }
        if index == 2 {
            let transition = json!({"schema_version":1,"request_id":"55000000-0000-4000-8000-000000000001","repository_id":registration["repository_id"],"operation":"transition","payload":{"run_id":registered["run_id"],"expected_generation":0,"state":"running"}});
            let result = run(root.path(), "transition", Some(&transition));
            assert_eq!(result.0, 0, "{}", result.1);
        }
        let request = json!({"schema_version":1,"request_id":format!("65000000-0000-4000-8000-{index:012}"),"repository_id":registration["repository_id"],"operation":"snapshot","payload":{"expected_registration_sequence":registration_sequence,"expected_usage_sequence":usage_sequence,"predecessor_digest":predecessor}});
        let result = run(root.path(), "snapshot", Some(&request));
        assert_eq!(result.0, 0, "{}", result.1);
        predecessor = result.1["result"]["receipt_digest"].clone();
        let path = root.path().join(".ai-gent/usage/receipts").join(format!(
            "{}.json",
            predecessor
                .as_str()
                .unwrap()
                .strip_prefix("sha256:")
                .unwrap()
        ));
        let bytes = std::fs::read(&path).unwrap();
        snapshots.push((request, result.1, path, bytes));
    }
    let replay = run(root.path(), "replay", None);
    assert_eq!(replay.0, 0, "{}", replay.1);
    assert_eq!(replay.1["result"]["receipt_head"], predecessor);
    assert_eq!(replay.1["result"]["usage"]["total_tokens"], 2400);
    assert_eq!(
        run(root.path(), "snapshot", Some(&snapshots[0].0)).1,
        snapshots[0].1
    );
    for (_, _, path, expected) in &snapshots {
        assert_eq!(std::fs::read(path).unwrap(), *expected);
    }
    let usage_path = root
        .path()
        .join(".ai-gent/usage/journal/observations.jsonl");
    let mut bytes = std::fs::read(&usage_path).unwrap();
    bytes.pop();
    std::fs::write(&usage_path, &bytes).unwrap();
    assert_eq!(run(root.path(), "replay", None).0, 5);
    assert_eq!(run(root.path(), "snapshot", Some(&snapshots[0].0)).0, 5);
    assert_eq!(std::fs::read(usage_path).unwrap(), bytes);
}
