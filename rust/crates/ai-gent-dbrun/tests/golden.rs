//! The spike's golden cases, against the crate: `rust/fixtures/db-core/golden.json` is made by running
//! the Python `dbrun.py` (see `rust/spikes/dbrun/gen_golden.py`), reasons included.
#![allow(clippy::many_single_char_names)] // (engine, sql, class, flag) tuples, as in the Python generator
use ai_gent_dbrun::core::*;
use serde_json::Value;

fn golden() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/db-core/golden.json"
        ))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn classify_matches_python() {
    let g = golden();
    let (mut bad, mut n, mut kind_bad) = (vec![], 0, 0);
    for r in g["classify"].as_array().unwrap() {
        let (e, s) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        let (k, why) = classify_statement(s, e);
        n += 1;
        if k != r[2] {
            kind_bad += 1;
        }
        if k != r[2] || why != r[3].as_str().unwrap() {
            bad.push(format!(
                "{e} {s:?}: py={} / {} rust={k} / {why}",
                r[2], r[3]
            ));
        }
    }
    eprintln!(
        "classify: {n} cases, {kind_bad} verdict diffs, {} verdict-or-reason diffs",
        bad.len()
    );
    for b in bad.iter().take(10) {
        eprintln!("  {b}");
    }
    assert!(bad.is_empty());
}

#[test]
fn split_ddl_cost_match_python() {
    let g = golden();
    let mut bad = vec![];
    for r in g["split"].as_array().unwrap() {
        let (e, s) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        let got = split_statements(s, e);
        let exp: Vec<String> = r[3]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        let same = match (&got, r[2].as_str().unwrap()) {
            (Ok(v), "ok") => *v == exp,
            (Err(_), "refused") => true,
            _ => false,
        };
        if !same {
            bad.push(format!("split {e} {s:?}"));
        }
    }
    for r in g["ddl"].as_array().unwrap() {
        let (e, s) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        if is_ddl(s, e) != r[2].as_bool().unwrap() {
            bad.push(format!("ddl {e} {s:?}"));
        }
    }
    for r in g["cost"].as_array().unwrap() {
        let (e, s, c, a) = (
            r[0].as_str().unwrap(),
            r[1].as_str().unwrap(),
            r[2].as_str().unwrap(),
            r[3].as_bool().unwrap(),
        );
        let got = cost_rules(s, c, a, e);
        // Reasons are part of the safety contract.
        if serde_json::to_value(got).unwrap() != r[4] {
            bad.push(format!("cost {e} {c} {a} {s:?}"));
        }
    }
    eprintln!("split/ddl/cost diffs: {}", bad.len());
    for b in bad.iter().take(10) {
        eprintln!("  {b}");
    }
    assert!(bad.is_empty());
}

#[test]
fn mask_redact_network_match_python() {
    let g = golden();
    for m in g["mask"].as_array().unwrap() {
        let cols: Vec<String> = m["cols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect();
        let rows: Vec<Vec<Value>> = m["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_array().unwrap().clone())
            .collect();
        let (masked, kinds) = mask::mask_result(&cols, &rows, m["reveal"].as_bool().unwrap());
        let exp: Vec<Vec<Value>> = m["masked"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_array().unwrap().clone())
            .collect();
        assert_eq!(masked, exp, "reveal={}", m["reveal"]);
        let mut k1: Vec<_> = kinds.into_iter().collect();
        k1.sort();
        let mut k2: Vec<(String, String)> = m["kinds"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(a, b)| (a.clone(), b.as_str().unwrap().into()))
            .collect();
        k2.sort();
        assert_eq!(k1, k2);
    }
    for r in g["redact"].as_array().unwrap() {
        let got = redact(
            r[0].as_str().unwrap(),
            &[
                ("s3cr3t!", "<password>"),
                ("db.secret-host.internal", "<host>"),
                ("app_ro", "<user>"),
            ],
        );
        // python order: password, host, user
        assert_eq!(got, r[1].as_str().unwrap());
    }
    let n = g["network"].as_array().unwrap();
    assert_eq!(
        network_args("abc", "", None),
        vec!["--network", "container:abc"]
    );
    assert_eq!(
        network_args("abc", "p1", Some(4242)),
        vec!["--network", "ns:/proc/4242/ns/net"]
    );
    assert_eq!(n.len(), 2);
}

#[test]
fn core_original_sixteen_tests_match_python() {
    use serde_json::json;
    let g = golden();
    let cases = g["unit"].as_array().unwrap();
    assert_eq!(cases.len(), 16);
    for case in cases {
        assert!(!case["calls"].as_array().unwrap().is_empty());
        for call in case["calls"].as_array().unwrap() {
            let a = &call["args"];
            let text = |key: &str| a[key].as_str().unwrap();
            let got: Result<Value, Refused> = match call["op"].as_str().unwrap() {
                "classify_statement" => Ok(json!(classify_statement(text("stmt"), text("engine")))),
                "split_statements" => {
                    split_statements(text("sql"), text("engine")).map(|v| json!(v))
                }
                "is_ddl" => Ok(json!(is_ddl(text("stmt"), text("engine")))),
                "cost_rules" => Ok(json!(cost_rules(
                    text("stmt"),
                    text("cls"),
                    a["allow_full_count"].as_bool().unwrap(),
                    text("engine")
                ))),
                "explain_sql" => Ok(json!(explain_sql(text("engine"), text("stmt")))),
                "require_user_database" => require_user_database(
                    a["p"]["engine"].as_str().unwrap(),
                    a["p"]["database"].as_str(),
                    text("action"),
                )
                .map(|()| Value::Null),
                "network_args" => {
                    let c = &a["cls"];
                    Ok(json!(network_args(
                        c["container_id"].as_str().unwrap(),
                        c["pod"].as_str().unwrap_or(""),
                        c["pid"].as_u64()
                    )))
                }
                "mask_result" => {
                    let columns = a["columns"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_owned())
                        .collect::<Vec<_>>();
                    let rows = a["rows"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| r.as_array().unwrap().clone())
                        .collect::<Vec<_>>();
                    let (rows, kinds) =
                        mask::mask_result(&columns, &rows, a["reveal"].as_bool().unwrap());
                    let kinds: serde_json::Map<String, Value> =
                        kinds.into_iter().map(|(k, v)| (k, json!(v))).collect();
                    Ok(json!([rows, kinds]))
                }
                "redact" => {
                    let secrets = call["secrets"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| (v[0].as_str().unwrap(), v[1].as_str().unwrap()))
                        .collect::<Vec<_>>();
                    Ok(json!(redact(text("text"), &secrets)))
                }
                op => panic!("unhandled fixture operation {op}"),
            };
            match got {
                Ok(v) => {
                    assert!(call.get("refused").is_none());
                    assert_eq!(v, call["expected"], "{}: {call}", case["derived_from"]);
                }
                Err(e) => assert_eq!(
                    e.0,
                    call["refused"].as_str().unwrap(),
                    "{}",
                    case["derived_from"]
                ),
            }
        }
    }
}

fn golden_writes() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/db-core/golden-writes.json"
        ))
        .unwrap(),
    )
    .unwrap()
}

/// T006 write side: `rust/fixtures/db-core/gen_writes.py` runs the ORIGINAL on statements that change
/// data, schema, sessions and the server (comment-split keywords, mixed case, hidden literals, every
/// engine). Each of the four verdicts must agree, reason text included: the reason is the contract.
#[test]
fn core_writes_classify_split_ddl_cost_match_python() {
    let g = golden_writes();
    let mut bad = vec![];
    let mut writes = 0;
    for r in g["classify"].as_array().unwrap() {
        let (e, s) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        let (k, why) = classify_statement(s, e);
        writes += usize::from(k == "write");
        if k != r[2] || why != r[3].as_str().unwrap() {
            bad.push(format!(
                "classify {e} {s:?}: py={} / {} rust={k} / {why}",
                r[2], r[3]
            ));
        }
    }
    assert!(
        writes > 1000,
        "the corpus must be mostly writes, got {writes}"
    );
    for r in g["split"].as_array().unwrap() {
        let (e, s) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        match (split_statements(s, e), r[2].as_str().unwrap()) {
            (Ok(v), "ok") => {
                let exp: Vec<&str> = r[3]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_str().unwrap())
                    .collect();
                if v != exp {
                    bad.push(format!("split {e} {s:?}: {v:?} != {exp:?}"));
                }
            }
            (Err(m), "refused") => {
                if m.0 != r[3].as_str().unwrap() {
                    bad.push(format!("split reason {e} {s:?}"));
                }
            }
            _ => bad.push(format!("split verdict {e} {s:?}")),
        }
    }
    for r in g["ddl"].as_array().unwrap() {
        let (e, s) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        if is_ddl(s, e) != r[2].as_bool().unwrap() {
            bad.push(format!("ddl {e} {s:?}"));
        }
    }
    for r in g["cost"].as_array().unwrap() {
        let (e, s, c, a) = (
            r[0].as_str().unwrap(),
            r[1].as_str().unwrap(),
            r[2].as_str().unwrap(),
            r[3].as_bool().unwrap(),
        );
        if serde_json::to_value(cost_rules(s, c, a, e)).unwrap() != r[4] {
            bad.push(format!("cost {e} {c} {a} {s:?}"));
        }
    }
    for r in g["explain"].as_array().unwrap() {
        assert_eq!(
            explain_sql(r[0].as_str().unwrap(), r[1].as_str().unwrap()),
            r[2]
        );
    }
    for b in bad.iter().take(25) {
        eprintln!("  {b}");
    }
    assert!(
        bad.is_empty(),
        "{} differences from the original",
        bad.len()
    );
}

#[test]
fn core_writes_system_databases_match_python() {
    let g = golden_writes();
    let (mut refused, mut ok) = (0, 0);
    for r in g["system_db"].as_array().unwrap() {
        let e = r[0].as_str().unwrap();
        let got = require_user_database(e, r[1].as_str(), "plan");
        match (got, r[2].as_str().unwrap()) {
            (Ok(()), "ok") => ok += 1,
            (Err(m), "refused") => {
                refused += 1;
                assert_eq!(m.0, r[3].as_str().unwrap(), "{r}");
            }
            (got, want) => panic!("{r}: rust {got:?}, python {want}"),
        }
    }
    assert!(
        refused >= 15 && ok >= 10,
        "{refused} refused, {ok} accepted"
    );
}

/// The inputs the core must refuse (the backslash split trick, a write on a system database, `SELECT *` and an
/// unfiltered `COUNT` on production): each is refused AND no text of the input, a fake password in it
/// included, comes back in the refusal.
#[test]
fn core_refusals_name_the_rule_and_never_echo_the_input() {
    let secret = "pw-FAKE-s3cr3t";
    let trick = format!("SELECT '{secret}\\'; DELETE FROM t; SELECT '");
    for engine in ["postgresql", "mysql"] {
        let err = split_statements(&trick, engine).unwrap_err();
        assert!(err.0.contains("split differently"), "{engine}: {err:?}");
        assert!(
            !err.0.contains(secret) && !err.0.contains("DELETE"),
            "{err:?}"
        );
    }
    // The statement is a write however it is dressed; the verdict names a keyword, never the literal.
    let (kind, why) =
        classify_statement(&format!("DELETE FROM t WHERE p = '{secret}'"), "postgresql");
    assert_eq!(kind, "write");
    assert!(!why.contains(secret), "{why}");
    let sys =
        require_user_database("sqlserver", Some("MASTER"), &format!("plan {secret}")).unwrap_err();
    assert!(sys.0.contains("system database 'master'"), "{sys:?}");
    for sql in [
        format!("SELECT * FROM orders WHERE note = '{secret}'"),
        format!("SELECT count(*) FROM orders -- {secret}"),
    ] {
        let rule = cost_rules(&sql, "production", false, "postgresql").expect("must be refused");
        assert!(!rule.contains(secret), "{rule}");
    }
    // Empty statements are writes: nothing runs by accident.
    assert_eq!(classify_statement("", "postgresql").0, "write");
    assert_eq!(classify_statement(";", "postgresql").0, "write");
}
