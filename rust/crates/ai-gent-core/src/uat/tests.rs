//! Tests of `uat::lint` and `uat::diff`: a clean roadmap, and one case per defect.
use super::diff::diff;
use super::lint::{Context, Severity, code_like, has_errors, lint};
use super::model::Roadmap;
use serde_json::{Value, json};
use std::fs;

const CLEAN: &str = include_str!("../../../../fixtures/uat/clean.json");

fn roadmap(edit: impl FnOnce(&mut Value)) -> Roadmap {
    let mut v: Value = serde_json::from_str(CLEAN).unwrap();
    edit(&mut v);
    serde_json::from_value(v).unwrap()
}

fn codes(r: &Roadmap) -> Vec<&'static str> {
    lint(r, &Context::default())
        .iter()
        .map(|f| f.code)
        .collect()
}

fn only(r: &Roadmap, code: &str) {
    let found = codes(r);
    assert!(found.contains(&code), "expected {code}, got {found:?}");
}

#[test]
fn clean_roadmap_has_no_findings() {
    let r = roadmap(|_| {});
    assert_eq!(lint(&r, &Context::default()), vec![]);
}

#[test]
fn claim_without_label() {
    only(
        &roadmap(|v| v["scripts"][1]["steps"][0]["claims"][0]["label"] = json!("")),
        "claim-without-label",
    );
}

#[test]
fn inferred_claim_names_how_to_confirm() {
    only(
        &roadmap(|v| v["scripts"][1]["steps"][1]["claims"][0]["ref"] = json!("")),
        "inferred-no-confirmation",
    );
}

#[test]
fn unobservable_expected_result() {
    only(
        &roadmap(|v| v["scripts"][1]["steps"][0]["expected"] = json!("  ")),
        "unobservable-expected",
    );
}

#[test]
fn result_seen_only_by_another_role() {
    only(
        &roadmap(|v| v["scripts"][1]["steps"][0]["seen_by"] = json!("admin")),
        "result-seen-by-other-role",
    );
}

#[test]
fn missing_precondition() {
    only(
        &roadmap(|v| v["scripts"][1]["steps"][1]["pre"] = json!("")),
        "missing-precondition",
    );
}

#[test]
fn shared_test_record() {
    only(
        &roadmap(|v| v["scripts"][2]["test_data"] = json!(["r1-20261119-order"])),
        "shared-test-record",
    );
}

#[test]
fn test_record_needs_the_script_prefix_and_date() {
    only(
        &roadmap(|v| v["scripts"][2]["test_data"] = json!(["order-1"])),
        "test-record-name",
    );
}

#[test]
fn r0_needs_a_write_check() {
    only(
        &roadmap(|v| v["scripts"][0]["steps"][0]["kind"] = json!("normal")),
        "missing-r0",
    );
}

#[test]
fn r0_needs_a_role_confirmation() {
    only(
        &roadmap(|v| v["scripts"][0]["steps"][1]["status"] = json!("retired")),
        "missing-r0",
    );
}

#[test]
fn first_script_must_be_r0() {
    only(
        &roadmap(|v| {
            let first = v["scripts"].as_array_mut().unwrap().remove(0);
            v["scripts"].as_array_mut().unwrap().push(first);
        }),
        "missing-r0",
    );
}

#[test]
fn authorization_rule_needs_a_refused_step() {
    only(
        &roadmap(|v| v["scripts"][2]["steps"][0]["refused"] = json!(false)),
        "rule-no-refused-step",
    );
}

#[test]
fn page_text_free_of_code_and_addresses() {
    for (text, what) in [
        ("Open src/app/orders.ts and press OK.", "a file path"),
        ("Run SELECT name FROM orders.", "SQL"),
        ("Write to ana@example.test.", "an e-mail address"),
        ("Check the order_status field.", "an identifier"),
        ("Press the `Save` button.", "code"),
    ] {
        assert_eq!(code_like(text), Some(what), "{text}");
        only(
            &roadmap(|v| v["scripts"][1]["steps"][0]["action"] = json!(text)),
            "code-like-text",
        );
    }
    assert_eq!(
        code_like("Press OK, then wait and/or look at the list."),
        None
    );
}

#[test]
fn validator_needs_an_assigned_role() {
    only(
        &roadmap(|v| v["validators"][0]["assigned_role"] = json!("")),
        "validator-no-role",
    );
    only(
        &roadmap(|v| v["validators"][0]["assigned_role"] = json!("ghost")),
        "unknown-role",
    );
}

#[test]
fn invite_never_reaches_the_page_text_check() {
    // The account is kept local (FR-091); only the label is page text.
    assert!(codes(&roadmap(|_| {})).is_empty());
    only(
        &roadmap(|v| v["validators"][0]["label"] = json!("ana@example.test")),
        "code-like-text",
    );
}

#[test]
fn production_is_refused() {
    only(
        &roadmap(|v| v["environment"]["class"] = json!("production")),
        "environment-production",
    );
    only(
        &roadmap(|v| v["environment"]["class"] = json!("")),
        "environment-unconfirmed",
    );
}

#[test]
fn ids_are_well_formed_and_unique() {
    only(
        &roadmap(|v| v["scripts"][1]["steps"][0]["id"] = json!("x1")),
        "bad-id",
    );
    only(
        &roadmap(|v| v["scripts"][1]["steps"][1]["id"] = json!("r1-1")),
        "duplicate-id",
    );
    only(
        &roadmap(|v| v["scripts"][1]["steps"][0]["id"] = json!("r2-9")),
        "bad-id",
    );
}

#[test]
fn missing_sections() {
    only(&roadmap(|v| v["how_to"] = json!("")), "missing-section");
    only(&roadmap(|v| v["changelog"] = json!([])), "missing-section");
    only(&roadmap(|v| v["roles"] = json!([])), "missing-section");
}

fn mode_b(v: &mut Value) {
    v["mode"] = json!("B");
    v["features"] = json!(["010"]);
    v["scripts"][1]["steps"][0]["traces"] = json!(["specs/010-orders/spec.md#US1-AS1"]);
}

fn ctx_b() -> Context {
    let mut c = Context::default();
    c.scenarios.insert(
        "010".into(),
        vec![
            "specs/010-orders/spec.md#US1-AS1".into(),
            "specs/010-orders/spec.md#US1-AS2".into(),
        ],
    );
    c
}

#[test]
fn mode_b_scenario_needs_a_step_or_a_reason() {
    let r = roadmap(mode_b);
    let f = lint(&r, &ctx_b());
    let missing: Vec<_> = f
        .iter()
        .filter(|x| x.code == "scenario-without-step")
        .collect();
    assert_eq!(missing.len(), 1);
    assert!(missing[0].at.ends_with("US1-AS2"));
    let listed = roadmap(|v| {
        mode_b(v);
        v["not_testable"] =
            json!([{"trace": "specs/010-orders/spec.md#US1-AS2", "reason": "needs a bank"}]);
    });
    assert!(
        !lint(&listed, &ctx_b())
            .iter()
            .any(|x| x.code == "scenario-without-step")
    );
    let no_reason = roadmap(|v| {
        mode_b(v);
        v["not_testable"] = json!([{"trace": "specs/010-orders/spec.md#US1-AS2", "reason": ""}]);
    });
    assert!(
        lint(&no_reason, &ctx_b())
            .iter()
            .any(|x| x.code == "not-testable-no-reason")
    );
}

#[test]
fn mode_b_feature_that_cannot_be_read_is_reported() {
    let r = roadmap(mode_b);
    assert!(codes(&r).contains(&"feature-unreadable"));
}

#[test]
fn scenarios_come_from_the_spec_reader() {
    let spec = "# Feature\n\n### User Story 1 - Orders (Priority: P1)\n\nText.\n\n**Acceptance Scenarios**:\n\n1. **Given** a, **When** b, **Then** c.\n2. **Given** d, **When** e, **Then** f.\n\n### User Story 2 - More (Priority: P2)\n\n**Acceptance Scenarios**:\n\n1. **Given** g, **When** h, **Then** i.\n\n## Requirements\n";
    assert_eq!(
        super::scenarios(spec, "010-orders"),
        vec![
            "specs/010-orders/spec.md#US1-AS1",
            "specs/010-orders/spec.md#US1-AS2",
            "specs/010-orders/spec.md#US2-AS1",
        ]
    );
}

#[test]
fn context_reads_features_from_a_directory() {
    let dir = tempfile::tempdir().unwrap();
    let feature = dir.path().join("specs/010-orders");
    fs::create_dir_all(&feature).unwrap();
    fs::write(
        feature.join("spec.md"),
        "# F\n\n### User Story 1 - A (Priority: P1)\n\n**Acceptance Scenarios**:\n\n1. **Given** a, **When** b, **Then** c.\n",
    )
    .unwrap();
    let r = roadmap(mode_b);
    let ctx = super::context_for(&r, dir.path(), None);
    assert_eq!(
        ctx.scenarios["010"],
        vec!["specs/010-orders/spec.md#US1-AS1"]
    );
    assert!(
        lint(&r, &ctx)
            .iter()
            .all(|f| f.code != "scenario-without-step")
    );
}

#[test]
fn size_budget_warns_without_blocking() {
    let r = roadmap(|_| {});
    let ctx = Context {
        budget: Some(1000),
        ..Context::default()
    };
    let f = lint(&r, &ctx);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].code, "size-budget");
    assert_eq!(f[0].severity, Severity::Warn);
    assert!(!has_errors(&f));
    let roomy = Context {
        budget: Some(10_000_000),
        ..Context::default()
    };
    assert!(lint(&r, &roomy).is_empty());
}

#[test]
fn exclusive_script_needs_its_note_and_record() {
    only(
        &roadmap(|v| v["scripts"][1]["steps"][0]["exclusive_note"] = json!("")),
        "missing-exclusivity",
    );
    only(
        &roadmap(|v| v["scripts"][1]["test_data"] = json!([])),
        "exclusive-no-record",
    );
}

#[test]
fn a_roadmap_that_is_not_json_is_refused() {
    assert!(
        Roadmap::parse("{")
            .unwrap_err()
            .starts_with("not a roadmap")
    );
    assert!(Roadmap::parse("{}").is_ok());
}

// diff

fn bumped(v: &mut Value) {
    v["version"] = json!(2);
}

#[test]
fn same_roadmap_is_no_change() {
    let a = roadmap(|_| {});
    let d = diff(&a, &a);
    assert!(d.refused.is_empty() && d.changelog.is_empty());
}

#[test]
fn removed_step_is_refused() {
    let old = roadmap(|_| {});
    let new = roadmap(|v| {
        bumped(v);
        v["scripts"][2]["steps"] = json!([]);
    });
    let d = diff(&old, &new);
    assert!(
        d.refused
            .iter()
            .any(|r| r.code == "step-removed" && r.at == "r2-1")
    );
}

#[test]
fn retiring_keeps_the_id() {
    let old = roadmap(|_| {});
    let new = roadmap(|v| {
        bumped(v);
        v["scripts"][2]["steps"][0]["status"] = json!("retired");
    });
    let d = diff(&old, &new);
    assert!(d.refused.is_empty(), "{:?}", d.refused);
    assert_eq!(d.retired, ["r2-1"]);
    assert_eq!(d.changelog, "v2: retired r2-1");
}

#[test]
fn retired_id_cannot_come_back() {
    let old = roadmap(|v| v["scripts"][2]["steps"][0]["status"] = json!("retired"));
    let new = roadmap(|v| {
        bumped(v);
        v["scripts"][2]["steps"][0]["status"] = json!("active");
    });
    assert!(
        diff(&old, &new)
            .refused
            .iter()
            .any(|r| r.code == "id-reused")
    );
}

#[test]
fn renumbered_step_is_refused() {
    let old = roadmap(|_| {});
    let new = roadmap(|v| {
        bumped(v);
        v["scripts"][2]["steps"][0]["id"] = json!("r2-2");
    });
    let codes: Vec<_> = diff(&old, &new).refused.iter().map(|r| r.code).collect();
    assert!(
        codes.contains(&"step-removed") && codes.contains(&"step-renumbered"),
        "{codes:?}"
    );
}

#[test]
fn changed_text_needs_a_bumped_revision() {
    let old = roadmap(|_| {});
    let edit = |v: &mut Value| {
        bumped(v);
        v["scripts"][1]["steps"][0]["expected"] = json!("The order shows as pending.");
    };
    let stale = diff(&old, &roadmap(edit));
    assert!(
        stale
            .refused
            .iter()
            .any(|r| r.code == "revision-not-bumped")
    );
    assert_eq!(stale.changed, ["r1-1"]);
    let ok = diff(
        &old,
        &roadmap(|v| {
            edit(v);
            v["scripts"][1]["steps"][0]["rev"] = json!(2);
        }),
    );
    assert!(ok.refused.is_empty(), "{:?}", ok.refused);
    assert_eq!(ok.changelog, "v2: rewrote r1-1");
}

#[test]
fn a_bumped_revision_without_a_change_is_refused() {
    let old = roadmap(|_| {});
    let new = roadmap(|v| {
        bumped(v);
        v["scripts"][1]["steps"][0]["rev"] = json!(2);
    });
    assert!(
        diff(&old, &new)
            .refused
            .iter()
            .any(|r| r.code == "revision-without-change")
    );
}

#[test]
fn appended_steps_are_listed_for_review() {
    let old = roadmap(|_| {});
    let new = roadmap(|v| {
        bumped(v);
        v["scripts"][2]["steps"].as_array_mut().unwrap().push(json!({
            "id": "r2-2", "order": 2, "rev": 1, "status": "active", "kind": "normal", "who": "clerk",
            "action": "Try again after a refresh.", "expected": "Still not allowed.", "pre": "An order exists.",
            "claims": []
        }));
    });
    let d = diff(&old, &new);
    assert!(d.refused.is_empty(), "{:?}", d.refused);
    assert_eq!(d.appended, ["r2-2"]);
    assert_eq!(d.review, ["r2-2"]);
}

#[test]
fn changed_claims_go_back_to_the_reviewer() {
    let old = roadmap(|_| {});
    let new = roadmap(|v| {
        v["scripts"][1]["steps"][0]["claims"][0]["text"] = json!("A new order starts as pending.");
    });
    assert_eq!(diff(&old, &new).review, ["r1-1"]);
}

#[test]
fn version_moves_only_with_a_change() {
    let old = roadmap(|_| {});
    let moved = diff(&old, &roadmap(bumped));
    assert!(
        moved
            .refused
            .iter()
            .any(|r| r.code == "version-without-change")
    );
    let new = roadmap(|v| {
        v["scripts"][2]["steps"][0]["status"] = json!("retired");
    });
    assert!(
        diff(&old, &new)
            .refused
            .iter()
            .any(|r| r.code == "version-not-bumped")
    );
}

#[test]
fn a_removed_script_is_refused() {
    let old = roadmap(|_| {});
    let new = roadmap(|v| {
        bumped(v);
        v["scripts"].as_array_mut().unwrap().pop();
    });
    assert!(
        diff(&old, &new)
            .refused
            .iter()
            .any(|r| r.code == "script-removed")
    );
}

// render (T012)

use super::render::{esc, page, render};

fn html(edit: impl FnOnce(&mut Value)) -> String {
    render(&roadmap(edit), &Context::default()).expect("clean roadmap renders")
}

#[test]
fn render_refuses_a_roadmap_with_errors() {
    let r = roadmap(|v| v["how_to"] = json!(""));
    let errs = render(&r, &Context::default()).unwrap_err();
    assert!(errs.iter().all(|f| f.severity == Severity::Error));
    assert_eq!(errs[0].code, "missing-section");
}

#[test]
fn render_warnings_do_not_refuse() {
    let r = roadmap(|v| v["settings"] = json!({"doc_budget_bytes": 1000}));
    assert!(render(&r, &Context::default()).is_ok());
}

#[test]
fn escaping() {
    assert_eq!(
        esc("<a href=\"x\">&'"),
        "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
    );
    let h = html(|v| v["title"] = json!("Orders <script>alert(1)</script> & more"));
    assert!(!h.contains("<script>alert"));
    assert!(h.contains("Orders &lt;script&gt;alert(1)&lt;/script&gt; &amp; more"));
}

#[test]
fn text_that_looks_like_a_token_stays_text() {
    let r = roadmap(|v| v["how_to"] = json!("Press OK. @@DATA@@ and @@BODY@@ are words here."));
    let h = page(&r);
    assert!(h.contains("@@DATA@@ and @@BODY@@ are words"));
    assert_eq!(h.matches("id=\"uat-data\"").count(), 1);
}

#[test]
fn data_island_cannot_close_the_script_and_keeps_accounts_out() {
    let h = html(|v| v["scripts"][1]["steps"][0]["action"] = json!("Create an order."));
    let start = h.find("id=\"uat-data\">").unwrap() + 14;
    let island = &h[start..h[start..].find("</script>").unwrap() + start];
    assert!(!island.contains('<') && !island.contains("ana@example.test"));
    let parsed: Value = serde_json::from_str(island).unwrap();
    assert_eq!(parsed["steps"].as_array().unwrap().len(), 5);
    assert!(!h.contains("ana@example.test") && !h.contains("v2@example.test"));
}

#[test]
fn every_active_step_row_has_its_data_attributes_and_controls() {
    let h = html(|_| {});
    for id in ["r0-1", "r0-2", "r1-1", "r1-2", "r2-1"] {
        let row = format!("<tr data-step=\"{id}\" data-rev=\"1\" data-role=\"");
        assert!(h.contains(&row), "{id}");
        for ctl in [
            format!("id=\"res-{id}-ok\""),
            format!("id=\"res-{id}-nok\""),
            format!("id=\"obs-{id}\""),
            format!("id=\"links-{id}\""),
            format!("id=\"imp-btn-{id}\""),
            format!("id=\"imp-{id}\""),
            format!("id=\"chips-{id}\""),
        ] {
            assert!(h.contains(&ctl), "{ctl}");
        }
    }
    assert_eq!(h.matches("data-kind=\"").count(), 5);
    assert!(
        h.contains("data-kind=\"write-check\"") && h.contains("data-kind=\"role-confirmation\"")
    );
    assert!(h.contains("data-exclusive=\"true\"") && h.contains("data-exclusive=\"false\""));
    assert!(h.contains("id=\"prop-role\"") && h.contains("id=\"prop-note\""));
}

#[test]
fn retired_steps_are_not_rendered() {
    let h = html(|v| {
        v["scripts"][2]["steps"].as_array_mut().unwrap().push(json!({
            "id": "r2-2", "order": 2, "rev": 2, "status": "retired", "kind": "normal", "who": "clerk",
            "action": "Old step.", "expected": "Old result.", "claims": []
        }));
    });
    assert!(!h.contains("data-step=\"r2-2\"") && h.contains("data-step=\"r2-1\""));
}

#[test]
fn no_upload_control_with_attachments_off() {
    let h = html(|_| {});
    assert!(h.contains("name=\"uat-attachments\" content=\"off\""));
    assert!(!h.contains("type=\"file\"") && !h.contains("id=\"ev-"));
    assert!(h.contains("Describe the screen in words."));
    let on = html(|v| v["settings"] = json!({"attachments": {"on": true, "budget_mb": 5}}));
    assert!(on.contains("name=\"uat-attachments\" content=\"on\""));
    assert!(on.contains("id=\"ev-r1-1\"") && on.contains("5 MB for the whole session"));
    assert!(on.contains("Do not upload cookies, tokens, passwords or personal data."));
}

#[test]
fn sections_come_in_the_order_of_fr_001() {
    let h = html(|_| {});
    let mut last = 0;
    for id in [
        "how-to",
        "people",
        "roster",
        "rules",
        "test-data",
        "scripts",
        "results",
        "limitations",
        "changelog",
    ] {
        let at = h
            .find(&format!("<section id=\"{id}\""))
            .unwrap_or_else(|| panic!("{id}"));
        assert!(at > last, "{id} out of order");
        last = at;
    }
}

#[test]
fn tokens_themes_and_safe_script() {
    let h = html(|_| {});
    assert!(
        h.contains(":root:not([data-theme=\"light\"])") && h.contains(":root[data-theme=\"dark\"]")
    );
    assert!(h.contains("prefers-color-scheme:dark") && h.contains("prefers-reduced-motion"));
    assert!(h.contains("role=\"status\"") && h.contains("id=\"notice\" role=\"alert\""));
    assert!(h.contains("name=\"uat-version\" content=\"1\""));
    assert!(!h.contains("innerHTML") && !h.contains("localStorage") && !h.contains("src=\"http"));
    assert!(!h.contains("@@"));
}

#[test]
fn page_golden_body() {
    insta::assert_snapshot!(super::render::body(&roadmap(|_| {})));
}
