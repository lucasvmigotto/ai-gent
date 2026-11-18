//! Usage evidence contract: synthetic sources and real CLI, without a paid model or provider call.
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
fn setup() -> (tempfile::TempDir, Value) {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tasks.md"), "- [ ] T001 Build\n").unwrap();
    let registration: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    let (code, value) = run(root.path(), "register", Some(&registration));
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
#[test]
fn usage_semantics_and_dedup_positive() {
    let (root, registration) = setup();
    let input = event(&registration, 1, "request_delta", Some("request-1"));
    let (code, first) = run(root.path(), "ingest", Some(&input));
    assert_eq!(code, 0, "{first}");
    assert_eq!(first["result"]["summary"]["total_tokens"], 1200);
    assert_eq!(first["result"]["summary"]["counters"]["cached_input"], 400);
    assert_eq!(run(root.path(), "ingest", Some(&input)).1, first);
    let terminal = event(&registration, 2, "turn_total", Some("request-1"));
    let (code, last) = run(root.path(), "ingest", Some(&terminal));
    assert_eq!(code, 0, "{last}");
    assert_eq!(last["result"]["summary"]["total_tokens"], 1200);
    assert_eq!(last["result"]["summary"]["observation_count"], 2);
    assert_eq!(last["result"]["summary"]["contribution_count"], 1);
    assert_eq!(last["result"]["summary"]["assurance"], "local_unverified");
    let replay = run(root.path(), "replay", None);
    assert_eq!(replay.0, 0, "{}", replay.1);
    assert_eq!(replay.1["result"]["usage"]["total_tokens"], 1200);
}
#[test]
fn disjoint_cache_input_is_added_once_and_unknown_is_not_zero() {
    let (root, registered) = setup();
    let mut input = event(&registered, 1, "turn_total", Some("request-1"));
    input["payload"]["source"]["semantics"] = "anthropic-disjoint-input-v1".into();
    input["payload"]["raw_usage"] = json!({"input_tokens":600,"cache_read_input_tokens":300,"cache_creation_input_tokens":100,"output_tokens":200});
    input["payload"]["counters"] = json!({"input_total":1000,"cached_input":300,"cache_creation_input":100,"output_total":200,"reasoning_output":null});
    let (code, result) = run(root.path(), "ingest", Some(&input));
    assert_eq!(code, 0, "{result}");
    assert_eq!(result["result"]["summary"]["total_tokens"], 1200);
    let mut missing = event(&registered, 2, "turn_total", Some("request-2"));
    missing["payload"]["raw_usage"] = json!({"input_tokens":null,"output_tokens":50});
    missing["payload"]["counters"] = json!({"input_total":null,"cached_input":null,"cache_creation_input":null,"output_total":50,"reasoning_output":null});
    let result = run(root.path(), "ingest", Some(&missing));
    assert_eq!(result.0, 0, "{}", result.1);
    assert!(result.1["result"]["summary"]["total_tokens"].is_null());
    assert_eq!(
        result.1["result"]["summary"]["known_total_lower_bound"],
        1250
    );
}
#[test]
fn inconsistent_counters_and_replayed_identities_cannot_mutate_totals() {
    let (root, registered) = setup();
    let first = event(&registered, 1, "turn_total", Some("request-1"));
    assert_eq!(run(root.path(), "ingest", Some(&first)).0, 0);
    let mut conflict = first.clone();
    conflict["payload"]["raw_usage"]["input_tokens"] = 999.into();
    assert_eq!(run(root.path(), "ingest", Some(&conflict)).0, 4);
    let mut invalid = event(&registered, 2, "turn_total", Some("request-2"));
    invalid["payload"]["raw_usage"]["cached_input_tokens"] = 1001.into();
    assert_eq!(run(root.path(), "ingest", Some(&invalid)).0, 2);
    invalid["payload"]["raw_usage"]["cached_input_tokens"] = (-1).into();
    assert_eq!(run(root.path(), "ingest", Some(&invalid)).0, 2);
    let mut orphan = event(&registered, 3, "turn_total", Some("request-3"));
    orphan["payload"]["run_id"] = "50000000-0000-4000-8000-000000000001".into();
    assert_eq!(run(root.path(), "ingest", Some(&orphan)).0, 3);
    assert_eq!(
        run(root.path(), "replay", None).1["result"]["usage"]["total_tokens"],
        1200
    );
}
#[test]
fn cumulative_summary_replaces_earlier_requests_and_rejects_counter_reset() {
    let (root, registered) = setup();
    let delta = event(&registered, 1, "request_delta", Some("request-1"));
    assert_eq!(run(root.path(), "ingest", Some(&delta)).0, 0);
    let summary = event(&registered, 2, "session_cumulative", None);
    let got = run(root.path(), "ingest", Some(&summary));
    assert_eq!(got.0, 0, "{}", got.1);
    assert_eq!(got.1["result"]["summary"]["total_tokens"], 1200);
    let mut reset = event(&registered, 3, "session_cumulative", None);
    reset["payload"]["raw_usage"]["input_tokens"] = 900.into();
    reset["payload"]["counters"]["input_total"] = 900.into();
    let denied = run(root.path(), "ingest", Some(&reset));
    assert_eq!(denied.0, 3, "{}", denied.1);
    assert_eq!(
        run(root.path(), "replay", None).1["result"]["usage"]["total_tokens"],
        1200
    );
}

#[test]
fn diagnostic_observations_do_not_charge_and_local_claims_cannot_self_certify() {
    let (root, registered) = setup();
    let mut diagnostic = event(&registered, 1, "request_delta", Some("request-1"));
    diagnostic["payload"]["contribution"] = "observation".into();
    let result = run(root.path(), "ingest", Some(&diagnostic));
    assert_eq!(result.0, 0, "{}", result.1);
    assert!(result.1["result"]["summary"]["total_tokens"].is_null());
    let terminal = event(&registered, 2, "turn_total", Some("request-1"));
    assert_eq!(
        run(root.path(), "ingest", Some(&terminal)).1["result"]["summary"]["total_tokens"],
        1200
    );
    let mut forged = event(&registered, 3, "turn_total", Some("request-2"));
    forged["payload"]["identity"]["identity_quality"] = "verified".into();
    assert_eq!(run(root.path(), "ingest", Some(&forged)).0, 3);
    forged["payload"]["identity"]["identity_quality"] = "reported".into();
    forged["payload"]["attestation"] = "provider_reconciled".into();
    assert_eq!(run(root.path(), "ingest", Some(&forged)).0, 3);
}
#[test]
fn same_provider_request_cannot_be_charged_to_two_runs_or_two_adapters() {
    let (root, registered) = setup();
    let first = event(&registered, 1, "turn_total", Some("request-1"));
    assert_eq!(run(root.path(), "ingest", Some(&first)).0, 0);
    let mut alias = event(&registered, 2, "turn_total", Some("request-1"));
    alias["payload"]["source"]["adapter"] = "second-source".into();
    assert_eq!(run(root.path(), "ingest", Some(&alias)).0, 4);
    let mut request: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    request["request_id"] = "60000000-0000-4000-8000-000000000001".into();
    let other = run(root.path(), "register", Some(&request));
    assert_eq!(other.0, 0, "{}", other.1);
    let replayed = event(&other.1["result"], 2, "turn_total", Some("request-1"));
    assert_eq!(run(root.path(), "ingest", Some(&replayed)).0, 4);
    assert_eq!(
        run(root.path(), "replay", None).1["result"]["usage"]["total_tokens"],
        1200
    );
}
#[test]
fn lost_optional_cumulative_fields_keep_known_bounds_and_do_not_become_zero() {
    let (root, registered) = setup();
    let first = event(&registered, 1, "session_cumulative", None);
    assert_eq!(run(root.path(), "ingest", Some(&first)).0, 0);
    let mut partial = event(&registered, 2, "session_cumulative", None);
    partial["payload"]["raw_usage"]["input_tokens"] = Value::Null;
    partial["payload"]["counters"]["input_total"] = Value::Null;
    let result = run(root.path(), "ingest", Some(&partial));
    assert_eq!(result.0, 0, "{}", result.1);
    assert!(result.1["result"]["summary"]["total_tokens"].is_null());
    assert_eq!(
        result.1["result"]["summary"]["known_total_lower_bound"],
        1200
    );
}
#[test]
fn new_transport_request_for_same_source_event_is_deduplicated_and_persisted() {
    let (root, registered) = setup();
    let first = event(&registered, 1, "turn_total", Some("request-1"));
    let initial = run(root.path(), "ingest", Some(&first));
    assert_eq!(initial.0, 0, "{}", initial.1);
    let mut alias = first.clone();
    alias["request_id"] = "60000000-0000-4000-8000-000000000001".into();
    let ack = run(root.path(), "ingest", Some(&alias));
    assert_eq!(ack.0, 0, "{}", ack.1);
    assert_eq!(ack.1["result"]["duplicate"], true);
    assert_eq!(ack.1["result"]["summary"]["total_tokens"], 1200);
    assert_eq!(run(root.path(), "ingest", Some(&alias)).1, ack.1);
    alias["payload"] = event(&registered, 2, "turn_total", Some("request-2"))["payload"].clone();
    assert_eq!(run(root.path(), "ingest", Some(&alias)).0, 4);
}

#[test]
fn shared_scope_is_registered_before_dispatch_and_never_becomes_exclusive_cost() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("tasks.md"),
        "- [ ] T001 Build\n- [ ] T002 Review\n",
    )
    .unwrap();
    let mut input: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    let task = input["payload"]["task"].clone();
    let mut other = task.clone();
    other["task_id"] = "T002".into();
    input["payload"]["execution_scope"] = json!({"kind":"shared","beneficiaries":[task,other]});
    let registered = run(root.path(), "register", Some(&input));
    assert_eq!(registered.0, 0, "{}", registered.1);
    let mut usage = event(&registered.1["result"], 1, "turn_total", Some("request-1"));
    usage["payload"]["task"] = Value::Null;
    usage["payload"]["attribution"] = "shared".into();
    usage["payload"]["execution_scope"] = input["payload"]["execution_scope"].clone();
    let accepted = run(root.path(), "ingest", Some(&usage));
    assert_eq!(accepted.0, 0, "{}", accepted.1);
    assert_eq!(accepted.1["result"]["summary"]["total_tokens"], 1200);
    assert!(accepted.1["result"]["summary"]["exclusive_counters"]["input_total"].is_null());
    assert_eq!(
        accepted.1["result"]["summary"]["shared_counters"]["input_total"],
        1000
    );
    usage["request_id"] = "60000000-0000-4000-8000-000000000009".into();
    usage["payload"]["attribution"] = "exclusive".into();
    assert_eq!(run(root.path(), "ingest", Some(&usage)).0, 4);
}
#[test]
fn previous_registration_frames_remain_byte_compatible() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".ai-gent/usage/journal");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("registrations.lock"), "").unwrap();
    let legacy = include_bytes!("fixtures/agent_046/registration-v1.jsonl");
    std::fs::write(dir.join("registrations.jsonl"), legacy).unwrap();
    let replay = run(root.path(), "replay", None);
    assert_eq!(replay.0, 0, "{}", replay.1);
    assert_eq!(replay.1["result"]["sequence"], 1);
    assert_eq!(
        std::fs::read(dir.join("registrations.jsonl")).unwrap(),
        legacy
    );
}

#[test]
fn malformed_dates_missing_fields_and_unsafe_integer_ranges_are_refused() {
    let (root, registered) = setup();
    for change in ["date", "field", "overflow", "unknown"] {
        let mut input = event(&registered, 1, "turn_total", Some("request-1"));
        match change {
            "date" => input["payload"]["observed_at"] = "2026-02-30T00:00:00Z".into(),
            "field" => {
                input["payload"]["counters"]
                    .as_object_mut()
                    .unwrap()
                    .remove("input_total");
            }
            "overflow" => {
                input["payload"]["raw_usage"]["input_tokens"] = 9_007_199_254_740_992_u64.into();
                input["payload"]["counters"]["input_total"] = 9_007_199_254_740_992_u64.into();
            }
            _ => input["payload"]["prompt"] = "not permitted".into(),
        }
        let refused = run(root.path(), "ingest", Some(&input));
        assert_eq!(refused.0, 2, "{}", refused.1);
    }
    assert_eq!(
        run(root.path(), "replay", None).1["result"]["usage"]["observation_count"],
        0
    );
}
#[test]
fn unknown_semantics_preserve_raw_data_but_never_invent_a_total() {
    let (root, registered) = setup();
    let mut input = event(&registered, 1, "turn_total", Some("request-1"));
    input["payload"]["source"]["semantics"] = "future-provider-format".into();
    input["payload"]["counters"] = json!({"input_total":null,"cached_input":null,"cache_creation_input":null,"output_total":null,"reasoning_output":null});
    input["payload"]["quality"] = "imported".into();
    let result = run(root.path(), "ingest", Some(&input));
    assert_eq!(result.0, 0, "{}", result.1);
    assert!(result.1["result"]["summary"]["total_tokens"].is_null());
    assert_eq!(result.1["result"]["normalized"]["known_semantics"], false);
    assert_eq!(result.1["result"]["normalized"]["quality"], "imported");
    let log = std::fs::read_to_string(
        root.path()
            .join(".ai-gent/usage/journal/observations.jsonl"),
    )
    .unwrap();
    let frame: Value = serde_json::from_str(log.trim_end()).unwrap();
    assert_eq!(
        frame["payload"]["payload"]["raw_usage"],
        input["payload"]["raw_usage"]
    );
}
#[test]
fn observation_log_corruption_is_preserved_and_blocks_both_mutation_paths() {
    let (root, registered) = setup();
    assert_eq!(
        run(
            root.path(),
            "ingest",
            Some(&event(&registered, 1, "turn_total", Some("request-1")))
        )
        .0,
        0
    );
    let path = root
        .path()
        .join(".ai-gent/usage/journal/observations.jsonl");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend(b"{torn");
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(run(root.path(), "replay", None).0, 5);
    assert_eq!(
        run(
            root.path(),
            "ingest",
            Some(&event(&registered, 2, "turn_total", Some("request-2")))
        )
        .0,
        5
    );
    let mut registration: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    registration["request_id"] = "60000000-0000-4000-8000-000000000001".into();
    assert_eq!(run(root.path(), "register", Some(&registration)).0, 5);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
#[test]
fn concurrent_ingestion_replays_once_without_multiplying_totals() {
    let (root, registered) = setup();
    let input = event(&registered, 1, "turn_total", Some("request-1"));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let path = root.path().to_path_buf();
            let input = input.clone();
            std::thread::spawn(move || run(&path, "ingest", Some(&input)))
        })
        .collect();
    for thread in threads {
        let result = thread.join().unwrap();
        assert_eq!(result.0, 0, "{}", result.1);
        assert_eq!(result.1["result"]["summary"]["total_tokens"], 1200);
    }
    let replay = run(root.path(), "replay", None);
    assert_eq!(replay.1["result"]["usage_sequence"], 1);
}

#[test]
fn source_session_can_be_discovered_after_registration_but_cannot_change_mid_run() {
    let (root, registered) = setup();
    let mut first = event(&registered, 1, "turn_total", Some("request-1"));
    first["payload"]["host_session_id"] = "discovered-session".into();
    let accepted = run(root.path(), "ingest", Some(&first));
    assert_eq!(accepted.0, 0, "{}", accepted.1);
    let mut changed = event(&registered, 2, "turn_total", Some("request-2"));
    changed["payload"]["host_session_id"] = "other-session".into();
    assert_eq!(run(root.path(), "ingest", Some(&changed)).0, 3);
}
#[test]
fn unattributed_runs_preserve_cost_without_crediting_the_anchor_task() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("specs/046-task-usage-ledger");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tasks.md"), "- [ ] T001 Coordinate\n").unwrap();
    let mut registration: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    registration["payload"]["execution_scope"] = json!({"kind":"unattributed","beneficiaries":[]});
    let registered = run(root.path(), "register", Some(&registration));
    assert_eq!(registered.0, 0, "{}", registered.1);
    let mut input = event(&registered.1["result"], 1, "turn_total", Some("request-1"));
    input["payload"]["task"] = Value::Null;
    input["payload"]["attribution"] = "unattributed".into();
    input["payload"]["execution_scope"] = registration["payload"]["execution_scope"].clone();
    let result = run(root.path(), "ingest", Some(&input));
    assert_eq!(result.0, 0, "{}", result.1);
    assert_eq!(
        result.1["result"]["summary"]["unattributed_counters"]["input_total"],
        1000
    );
    assert!(result.1["result"]["summary"]["exclusive_counters"]["input_total"].is_null());
}
#[test]
fn unknown_cumulative_gap_does_not_hide_a_later_counter_reset() {
    let (root, registered) = setup();
    assert_eq!(
        run(
            root.path(),
            "ingest",
            Some(&event(&registered, 1, "session_cumulative", None))
        )
        .0,
        0
    );
    let mut gap = event(&registered, 2, "session_cumulative", None);
    gap["payload"]["raw_usage"] = json!({});
    gap["payload"]["counters"] = json!({"input_total":null,"output_total":null,"cached_input":null,"cache_creation_input":null,"reasoning_output":null});
    assert_eq!(run(root.path(), "ingest", Some(&gap)).0, 0);
    let mut reset = event(&registered, 3, "session_cumulative", None);
    reset["payload"]["raw_usage"]["input_tokens"] = 900.into();
    reset["payload"]["counters"]["input_total"] = 900.into();
    assert_eq!(run(root.path(), "ingest", Some(&reset)).0, 3);
}
#[test]
fn request_ids_are_unique_across_registration_and_usage_logs() {
    let (root, registered) = setup();
    let usage = event(&registered, 1, "turn_total", Some("request-1"));
    assert_eq!(run(root.path(), "ingest", Some(&usage)).0, 0);
    let mut registration: Value =
        serde_json::from_str(include_str!("fixtures/agent_046/register.json")).unwrap();
    registration["request_id"] = usage["request_id"].clone();
    assert_eq!(run(root.path(), "register", Some(&registration)).0, 4);
    let mut reused = event(&registered, 2, "turn_total", Some("request-2"));
    reused["request_id"] = "10000000-0000-4000-8000-000000000001".into();
    assert_eq!(run(root.path(), "ingest", Some(&reused)).0, 4);
}
