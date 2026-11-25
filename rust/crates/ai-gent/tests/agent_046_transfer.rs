//! Usage evidence contract: synthetic sources and real CLI, without a paid model or provider call.
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
fn call(root: &Path, action: &str, input: Option<&Value>, dry_run: bool) -> (i32, Value) {
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
    let (code, value) = call(root.path(), "register", Some(&registration), false);
    assert_eq!(code, 0, "{value}");
    (root, value["result"].clone())
}
fn event(registered: &Value, n: u32, scope: &str, request_id: Option<&str>) -> Value {
    let mut observation: Value = serde_json::from_str(include_str!(
        "../../../..//specs/046-task-usage-ledger/contracts/examples/usage-valid.json"
    ))
    .unwrap();
    observation["event_id"] = format!("30000000-0000-4000-8000-{n:012}").into();
    observation["task"] = registered["task"].clone();
    observation["attempt_id"] = registered["attempt_id"].clone();
    observation["run_id"] = registered["run_id"].clone();
    observation["parent_run_id"] = registered["parent_run_id"].clone();
    observation["segment_id"] = registered["segments"][0]["segment_id"].clone();
    observation["phase"] = registered["phase"].clone();
    observation["sequence"] = n.into();
    let configuration = &registered["segments"][0]["configuration"];
    observation["identity"] = configuration.clone();
    observation["identity"]["identity_quality"] = "reported".into();
    observation["identity"]["effective_provider"] = "openai".into();
    observation["identity"]["effective_model"] = "fixture-model".into();
    observation["source"]["event_id"] = format!("source-{n}").into();
    observation["source"]["scope"] = scope.into();
    observation["source"]["request_id"] = json!(request_id);
    observation["execution_scope"]["beneficiaries"] = json!([registered["task"].clone()]);
    observation["host_session_id"] = Value::Null;
    json!({"schema_version":1,"request_id":format!("40000000-0000-4000-8000-{n:012}"),"repository_id":registered["task"]["repository_id"],"operation":"ingest","payload":observation})
}

fn run(root: &Path, action: &str, input: Option<&Value>) -> (i32, Value) {
    call(root, action, input, false)
}
fn exported() -> (tempfile::TempDir, Value, Value) {
    let (root, registered) = setup();
    let input = event(&registered, 1, "turn_total", Some("private-request"));
    assert_eq!(run(root.path(), "ingest", Some(&input)).0, 0);
    let exported = run(root.path(), "export", None);
    assert_eq!(exported.0, 0, "{}", exported.1);
    (root, registered, exported.1)
}
fn import(exported: &Value) -> Value {
    json!({"schema_version":1,"request_id":"80000000-0000-4000-8000-000000000001","repository_id":exported["result"]["bundle"]["repository_id"],"operation":"import","payload":{"bundle_digest":exported["result"]["bundle_digest"],"bundle":exported["result"]["bundle"]}})
}
#[test]
fn export_import_is_private_idempotent_and_never_changes_measured_totals() {
    let (source, _, bundle) = exported();
    let bytes = std::fs::read(
        source
            .path()
            .join(".ai-gent/usage/journal/observations.jsonl"),
    )
    .unwrap();
    let target = tempfile::tempdir().unwrap();
    let request = import(&bundle);
    let plan = call(target.path(), "import", Some(&request), true);
    assert_eq!(plan.0, 0, "{}", plan.1);
    assert!(!target.path().join(".ai-gent").exists());
    let first = run(target.path(), "import", Some(&request));
    assert_eq!(first.0, 0, "{}", first.1);
    assert_eq!(first.1["result"]["assurance"], "local_unverified");
    assert_eq!(first.1["result"]["quality"], "imported");
    assert_eq!(first.1["result"]["contributes_to_local_totals"], false);
    assert_eq!(run(target.path(), "import", Some(&request)), first);
    let replay = run(target.path(), "replay", None);
    assert_eq!(replay.0, 0, "{}", replay.1);
    assert!(replay.1["result"]["usage"]["total_tokens"].is_null());
    assert_eq!(
        std::fs::read(
            source
                .path()
                .join(".ai-gent/usage/journal/observations.jsonl")
        )
        .unwrap(),
        bytes
    );
}
#[test]
fn export_strips_host_session_source_paths_and_request_labels() {
    let (root, registered) = setup();
    let mut input = event(&registered, 1, "turn_total", Some("private-request"));
    input["payload"]["host_session_id"] = "private-session".into();
    input["payload"]["source"]["namespace"] = "private-namespace".into();
    input["payload"]["source"]["reference"] = "evidence:/home/private/account/source".into();
    assert_eq!(run(root.path(), "ingest", Some(&input)).0, 0);
    let bundle = run(root.path(), "export", None);
    assert_eq!(bundle.0, 0, "{}", bundle.1);
    let text = bundle.1.to_string();
    for forbidden in [
        "private-session",
        "private-namespace",
        "private-request",
        "/home/private",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    assert_eq!(
        bundle.1["result"]["bundle"]["observations"][0]["identity"]["effective_model"],
        "fixture-model"
    );
}
#[test]
fn altered_bundle_or_unknown_version_is_rejected_without_writes() {
    let (_, _, bundle) = exported();
    let root = tempfile::tempdir().unwrap();
    let mut request = import(&bundle);
    request["payload"]["bundle"]["observations"][0]["counters"]["input_total"] = 999.into();
    assert_ne!(call(root.path(), "import", Some(&request), true).0, 0);
    assert!(!root.path().join(".ai-gent").exists());
    request = import(&bundle);
    request["schema_version"] = 2.into();
    assert_eq!(run(root.path(), "import", Some(&request)).0, 2);
    assert!(!root.path().join(".ai-gent").exists());
}
#[test]
fn export_refuses_corrupt_sources_without_sanitizing_them_into_success() {
    let (root, _, _) = exported();
    let path = root
        .path()
        .join(".ai-gent/usage/journal/observations.jsonl");
    std::fs::write(&path, b"torn").unwrap();
    assert_eq!(run(root.path(), "export", None).0, 5);
    assert_eq!(std::fs::read(path).unwrap(), b"torn");
}

#[test]
fn import_preserves_current_ledger_and_rejects_global_request_id_reuse() {
    let (_, _, bundle) = exported();
    let (target, registered) = setup();
    let usage = event(&registered, 2, "turn_total", Some("local-request"));
    assert_eq!(run(target.path(), "ingest", Some(&usage)).0, 0);
    let before = run(target.path(), "replay", None).1;
    let request = import(&bundle);
    assert_eq!(run(target.path(), "import", Some(&request)).0, 0);
    assert_eq!(run(target.path(), "replay", None).1, before);
    let mut registration: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    registration["request_id"] = request["request_id"].clone();
    assert_eq!(run(target.path(), "register", Some(&registration)).0, 4);
    let mut conflicting = request.clone();
    conflicting["request_id"] = usage["request_id"].clone();
    assert_eq!(run(target.path(), "import", Some(&conflicting)).0, 4);
}
#[test]
fn corrupted_imported_archive_or_manifest_refuses_retry_without_overwrite() {
    for manifest in [false, true] {
        let (_, _, bundle) = exported();
        let root = tempfile::tempdir().unwrap();
        let request = import(&bundle);
        assert_eq!(run(root.path(), "import", Some(&request)).0, 0);
        let file = if manifest {
            format!("import-{}.json", request["request_id"].as_str().unwrap())
        } else {
            format!(
                "{}.json",
                request["payload"]["bundle_digest"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("sha256:")
            )
        };
        let path = root.path().join(".ai-gent/usage/imports").join(file);
        std::fs::write(&path, b"torn").unwrap();
        assert_eq!(run(root.path(), "import", Some(&request)).0, 5);
        assert_eq!(call(root.path(), "import", Some(&request), true).0, 5);
        assert_eq!(std::fs::read(path).unwrap(), b"torn");
    }
}
#[test]
fn malformed_bundle_graph_quality_and_claims_are_refused_without_creating_store() {
    let (_, _, bundle) = exported();
    for bad in [
        "phase",
        "quality",
        "nil",
        "cycle",
        "attempt-cycle",
        "segment",
        "identity",
        "summary",
        "nullable",
        "source",
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut request = import(&bundle);
        let archive = &mut request["payload"]["bundle"];
        match bad {
            "phase" => archive["runs"][0]["role"] = "reviewer".into(),
            "quality" => archive["runs"][0]["usage_quality"] = "verified".into(),
            "nil" => archive["runs"][0]["run_id"] = "00000000-0000-0000-0000-000000000000".into(),
            "cycle" => archive["runs"][0]["parent_run_id"] = archive["runs"][0]["run_id"].clone(),
            "attempt-cycle" => {
                archive["runs"][0]["parent_attempt_id"] = archive["runs"][0]["attempt_id"].clone();
            }
            "segment" => archive["runs"][0]["segments"][0]["identity_quality"] = "verified".into(),
            "identity" => {
                archive["observations"][0]["identity"]["identity_quality"] = "verified".into();
            }
            "summary" => archive["summary"] = json!({"total_tokens":0}),
            "nullable" => {
                archive["observations"][0]["counters"]
                    .as_object_mut()
                    .unwrap()
                    .remove("input_total");
            }
            _ => archive["observations"][0]["source"]["namespace"] = "private-host".into(),
        }
        let rejected = call(root.path(), "import", Some(&request), true);
        assert_ne!(rejected.0, 0, "{bad}");
        assert!(!root.path().join(".ai-gent").exists(), "{bad}");
    }
}
#[cfg(unix)]
#[test]
fn import_refuses_symlink_escape_before_creating_a_journal() {
    let (_, _, bundle) = exported();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".ai-gent/usage")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join(".ai-gent/usage/imports")).unwrap();
    let request = import(&bundle);
    assert_ne!(run(root.path(), "import", Some(&request)).0, 0);
    assert!(!root.path().join(".ai-gent/usage/journal").exists());
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}
#[test]
fn concurrent_import_retries_publish_one_bundle_and_manifest() {
    let (_, _, bundle) = exported();
    let root = tempfile::tempdir().unwrap();
    let request = import(&bundle);
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let path = root.path().to_path_buf();
            let input = request.clone();
            std::thread::spawn(move || run(&path, "import", Some(&input)))
        })
        .collect();
    for thread in threads {
        let output = thread.join().unwrap();
        assert_eq!(output.0, 0, "{}", output.1);
    }
    assert_eq!(
        std::fs::read_dir(root.path().join(".ai-gent/usage/imports"))
            .unwrap()
            .count(),
        2
    );
}

#[test]
fn shared_and_unattributed_scopes_roundtrip_without_becoming_exclusive() {
    for kind in ["shared", "unattributed"] {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("specs/046-task-usage-ledger");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("tasks.md"),
            "- [ ] T001 Build\n- [ ] T002 Review\n",
        )
        .unwrap();
        let mut request: Value =
            serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
        let first = request["payload"]["task"].clone();
        let mut second = first.clone();
        second["task_id"] = "T002".into();
        request["payload"]["execution_scope"] =
            json!({"kind":kind,"beneficiaries":if kind=="shared"{vec![first,second]}else{vec![]}});
        let registered = run(root.path(), "register", Some(&request));
        assert_eq!(registered.0, 0, "{}", registered.1);
        let mut usage = event(&registered.1["result"], 1, "turn_total", Some("request-1"));
        usage["payload"]["task"] = Value::Null;
        usage["payload"]["attribution"] = kind.into();
        usage["payload"]["execution_scope"] = request["payload"]["execution_scope"].clone();
        assert_eq!(run(root.path(), "ingest", Some(&usage)).0, 0);
        let exported = run(root.path(), "export", None);
        assert_eq!(exported.0, 0, "{}", exported.1);
        let target = tempfile::tempdir().unwrap();
        let imported = run(target.path(), "import", Some(&import(&exported.1)));
        assert_eq!(imported.0, 0, "{}", imported.1);
        assert!(imported.1["result"]["summary"]["exclusive_counters"]["input_total"].is_null());
        assert_eq!(
            imported.1["result"]["summary"][format!("{kind}_counters")]["input_total"],
            1000
        );
    }
}
