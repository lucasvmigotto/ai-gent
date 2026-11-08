//! The connector contract itself (spec 043 T002): versioned fixtures, the JSON schema, and the link
//! from each intended difference to `specs/043-dbrun-port/differences.md`. No driver needed.
use ai_gent_dbrun::connector::contract::{CONTRACT_VERSION, check_schema, load_cases, schema};
use ai_gent_dbrun::connector::{ConnFacts, ExecResult, Mode, Request, Stage, StatementResult};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

#[test]
fn connector_contract_every_fixture_and_the_schema_carry_the_current_version() {
    assert_eq!(schema()["contract_version"], json!(CONTRACT_VERSION));
    let cases = load_cases();
    assert!(cases.len() >= 20, "{} cases", cases.len());
    for case in &cases {
        assert_eq!(
            case.doc["contract_version"],
            json!(CONTRACT_VERSION),
            "{} has no or an old contract_version",
            case.file
        );
    }
}

#[test]
fn connector_contract_schema_lists_exactly_the_fields_of_the_serde_types() {
    let s = schema();
    let conn = ConnFacts {
        host: Some("h".into()),
        port: Some("1".into()),
        database: Some("d".into()),
        service: Some("s".into()),
        sid: Some("i".into()),
        user: Some("u".into()),
        password: Some("p".into()),
        path: Some("f".into()),
    };
    let mut req = Request::new("sqlite", conn, Mode::Write, vec!["SELECT 1".into()]);
    req.explain = true;
    req.expect = vec![Some(1)];
    let req = serde_json::to_value(&req).unwrap();
    assert_eq!(
        keys(&req),
        keys(&s["request"]["properties"]),
        "request fields"
    );
    assert_eq!(
        keys(&req["conn"]),
        keys(&s["request"]["properties"]["conn"]["properties"]),
        "conn fields"
    );
    check_schema("request", &s["request"], &req).unwrap();

    let mut result = ExecResult::failure(Stage::Verify, "x");
    result.server = Some(vec![json!("v")]);
    result.links = Some(0);
    result.committed = Some(false);
    result.seconds = Some(0.5);
    result.results = Some(vec![StatementResult {
        rowcount: 1,
        columns: Some(vec!["a".into()]),
        rows: Some(vec![vec![json!(1)]]),
        truncated: Some(false),
    }]);
    let result = serde_json::to_value(&result).unwrap();
    assert_eq!(
        keys(&result),
        keys(&s["result"]["properties"]),
        "result fields"
    );
    assert_eq!(
        keys(&result["results"][0]),
        keys(&s["result"]["properties"]["results"]["items"]["properties"]),
        "statement result fields"
    );
    check_schema("result", &s["result"], &result).unwrap();
    for stage in ["connect", "execute", "verify", "request"] {
        let line = ExecResult::failure(serde_json::from_value(json!(stage)).unwrap(), "x");
        check_schema("result", &s["result"], &serde_json::to_value(line).unwrap()).unwrap();
    }
}

#[test]
fn connector_contract_schema_check_can_fail() {
    // The validator is only worth having if it refuses: each bad value below must be rejected.
    let s = schema();
    let good = json!({"engine": "sqlite", "mode": "read", "conn": {}});
    check_schema("r", &s["request"], &good).unwrap();
    for bad in [
        json!({"engine": "sqlite", "mode": "read"}),
        json!({"engine": "oracle9", "mode": "read", "conn": {}}),
        json!({"engine": "sqlite", "mode": "drop", "conn": {}}),
        json!({"engine": "sqlite", "mode": "read", "conn": {}, "extra": 1}),
        json!({"engine": "sqlite", "mode": "read", "conn": {"password": ["x"]}}),
        json!({"engine": "sqlite", "mode": "read", "conn": {}, "max_rows": -1}),
        json!({"engine": "sqlite", "mode": "read", "conn": {}, "expect": ["1"]}),
    ] {
        assert!(check_schema("r", &s["request"], &bad).is_err(), "{bad}");
    }
    for bad in [
        json!({}),
        json!({"ok": true, "stage": "boom"}),
        json!({"ok": true, "results": [{"columns": ["a"]}]}),
        json!({"ok": true, "results": [{"rowcount": -2}]}),
        json!({"ok": "yes"}),
        json!({"ok": true, "seconds": -1}),
    ] {
        assert!(check_schema("x", &s["result"], &bad).is_err(), "{bad}");
    }
    assert!(check_schema("x", &json!({"pattern": "a"}), &json!("a")).is_err());
}

#[test]
fn connector_contract_fixtures_cover_every_stage_and_both_max_rows_modes() {
    let mut stages = BTreeSet::new();
    let (mut read_trunc, mut write_cap) = (false, false);
    for case in load_cases().iter().filter(|c| c.kind() == "exec") {
        for step in case.doc["steps"].as_array().unwrap() {
            for key in ["expected", "expected_port"] {
                if let Some(s) = step[key]["stage"].as_str() {
                    stages.insert(s.to_string());
                }
            }
            let mode = step["request"]["mode"].as_str().unwrap_or("");
            let r = &step["expected"]["results"][0];
            read_trunc |= mode == "read" && r["truncated"] == json!(true);
            write_cap |= mode == "write" && r["truncated"] == json!(false) && r["rows"].is_array();
        }
    }
    // `request` is the port's own stage (a malformed line), tested in the crate's unit tests.
    for stage in ["connect", "execute", "verify"] {
        assert!(stages.contains(stage), "no fixture reaches stage {stage}");
    }
    assert!(read_trunc, "no fixture truncates a read");
    assert!(write_cap, "no fixture caps a write without truncating");
}

#[test]
fn connector_contract_each_intended_difference_is_in_differences_md() {
    let md = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../specs/043-dbrun-port/differences.md"),
    )
    .expect("specs/043-dbrun-port/differences.md");
    let mut used = BTreeSet::new();
    for case in load_cases().iter().filter(|c| c.kind() == "exec") {
        for (i, step) in case.doc["steps"].as_array().unwrap().iter().enumerate() {
            let differs = step.get("expected_port").is_some() || step["only"] == "port";
            let id = step["difference"].as_str();
            assert!(
                !differs || id.is_some(),
                "{} step {}: a port-only expectation needs a `difference` id",
                case.file,
                i + 1
            );
            if let Some(id) = id {
                assert!(
                    md.contains(&format!("| {id} |")),
                    "{}: {id} is not a row of differences.md",
                    case.file
                );
                used.insert(id.to_string());
            }
        }
    }
    assert!(
        used.contains("D-001"),
        "the DDL row count fixture names D-001"
    );
}
