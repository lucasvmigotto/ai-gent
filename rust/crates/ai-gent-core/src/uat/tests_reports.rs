//! Tests of `uat::digest`, `uat::report` and `uat::preflight` over snapshot pages (T013, T014).
use super::digest::{FollowUp, Snapshot, status, text};
use super::model::Roadmap;
use super::preflight::{Declared, Read, checklist, checklist_text, verify, verify_text};
use super::report::{Records, report};
use serde_json::{Value, json};

const CLEAN: &str = include_str!("../../../../fixtures/uat/clean.json");

fn roadmap(edit: impl FnOnce(&mut Value)) -> Roadmap {
    let mut v: Value = serde_json::from_str(CLEAN).unwrap();
    edit(&mut v);
    serde_json::from_value(v).unwrap()
}

fn snap(v: Value) -> Snapshot {
    serde_json::from_value(v).unwrap()
}

fn mark(r: &str, obs: &str, at: &str) -> Value {
    json!({"r": r, "obs": obs, "at": at, "rev": 1, "links": []})
}

/// One validator, the administrator, who did R0 and approved.
fn one() -> Value {
    json!({
        "at": "2026-11-20T10:00:00Z",
        "roster": {"u1": {"validator": "v1", "role": "admin"}},
        "marks": {"u1": {"steps": {
            "r0-1": mark("OK", "", "2026-11-20T09:50:00Z"),
            "r0-2": mark("OK", "", "2026-11-20T09:51:00Z"),
            "r1-2": mark("OK", "", "2026-11-20T09:55:00Z")
        }}},
        "names": {"u1": "Ana S."}
    })
}

/// Two validators at once: the clerk found a NOK, suggested a change and proposed another role.
fn two() -> Value {
    let mut v = one();
    v["at"] = json!("2026-11-20T11:00:00Z");
    v["roster"]["u2"] = json!({"validator": "v2", "role": "clerk"});
    v["marks"]["u2"] = json!({
        "steps": {
            "r0-1": mark("OK", "", "2026-11-20T10:30:00Z"),
            "r1-1": mark("NOK", "The order stays blank after saving.", "2026-11-20T10:40:00Z"),
            "r2-1": mark("OK", "", "2026-11-20T10:45:00Z")
        },
        "improvements": {"r1-1": {"txt": "Show a saved message.", "at": "2026-11-20T10:41:00Z", "rev": 1}}
    });
    v["proposals"] = json!({"u2": {"role": "admin", "note": "I also approve orders.", "at": "2026-11-20T10:32:00Z", "decision": "pending"}});
    v["triage"] = json!({"r1-1~u2": {"class": "defect", "reply": "Thanks, we reproduced it.", "status": "open"}});
    v
}

fn s(v: Value) -> Snapshot {
    snap(v)
}

#[test]
fn empty_register() {
    let r = roadmap(|_| {});
    let st = status(&r, &s(json!({"at": "2026-11-20T09:00:00Z"})), None, None);
    assert!(
        st.scripts
            .iter()
            .all(|p| p.ok == 0 && p.nok == 0 && p.unmarked == p.steps)
    );
    assert_eq!(st.scripts[1].steps, 2);
    assert_eq!(st.write_checks_missing, ["Ana", "Validator 2"]);
    assert!(text(&st).starts_with("Last real check: 2026-11-20 09:00\n"));
}

#[test]
fn one_validator() {
    let r = roadmap(|_| {});
    let st = status(&r, &s(one()), None, None);
    assert_eq!(st.scripts[0].ok, 2);
    assert_eq!(st.scripts[1].ok, 1);
    assert_eq!(st.scripts[1].markers, 1);
    assert_eq!(st.write_checks_missing, ["Validator 2"]);
    assert!(st.gap_role.is_empty(), "{:?}", st.gap_role);
}

#[test]
fn concurrent_validators_and_nok_wins() {
    let r = roadmap(|_| {});
    let st = status(&r, &s(two()), None, None);
    let r1 = &st.scripts[1];
    assert_eq!((r1.ok, r1.nok, r1.unmarked), (1, 1, 0));
    assert_eq!(st.new_noks.len(), 1);
    assert_eq!(st.new_noks[0].who, "Validator 2");
    assert_eq!(st.new_noks[0].class, "defect");
    assert_eq!(st.new_improvements.len(), 1);
    assert!(st.write_checks_missing.is_empty());
}

#[test]
fn a_pending_proposal_changes_no_count() {
    let r = roadmap(|_| {});
    let with = status(&r, &s(two()), None, None);
    let mut v = two();
    v["proposals"] = json!({});
    let without = status(&r, &s(v), None, None);
    assert_eq!(with.scripts[2].ok, without.scripts[2].ok);
    assert_eq!(with.gap_role, without.gap_role);
    assert_eq!(with.proposals_pending.len(), 1);
    assert_eq!(with.proposals_pending[0].assigned, "Clerk");
    assert_eq!(with.proposals_pending[0].proposed, "Administrator");
}

#[test]
fn marks_by_the_wrong_role_are_flagged() {
    let r = roadmap(|_| {});
    let mut v = one();
    v["marks"]["u1"]["steps"]["r1-1"] = mark("OK", "", "2026-11-20T09:56:00Z");
    let st = status(&r, &s(v), None, None);
    assert!(
        st.gap_role.contains(&"r1-1".to_string()),
        "{:?}",
        st.gap_role
    );
    assert!(!st.gap_role.contains(&"r1-2".to_string()));
}

#[test]
fn an_unlisted_account_counts_for_no_role() {
    let r = roadmap(|_| {});
    let mut v = one();
    v["marks"]["u9"] = json!({"steps": {"r1-1": mark("OK", "", "2026-11-20T09:58:00Z")}});
    let st = status(&r, &s(v), None, None);
    assert_eq!(st.unassigned, ["an unlisted account"]);
    assert!(st.gap_role.contains(&"r1-1".to_string()));
}

#[test]
fn stale_marks_are_shown_not_counted() {
    let r = roadmap(|v| v["scripts"][1]["steps"][1]["rev"] = json!(2));
    let st = status(&r, &s(one()), None, None);
    assert_eq!(st.stale_steps, ["r1-2"]);
    assert_eq!(st.scripts[1].stale, 1);
    assert_eq!(st.scripts[1].ok, 0);
}

#[test]
fn retired_steps_are_not_counted() {
    let r = roadmap(|v| {
        v["scripts"][2]["steps"].as_array_mut().unwrap().push(json!({
            "id": "r2-2", "order": 2, "rev": 2, "status": "retired", "kind": "normal", "who": "clerk",
            "action": "Old.", "expected": "Old.", "claims": []
        }));
    });
    let st = status(&r, &s(two()), None, None);
    assert_eq!(st.scripts[2].steps, 1);
}

#[test]
fn people_who_left_leave_their_marks_and_a_gap() {
    let r = roadmap(|_| {});
    let mut v = one();
    v["left"] = json!(["u1"]);
    let st = status(&r, &s(v), None, None);
    assert_eq!(st.scripts[1].ok, 1);
    assert_eq!(st.gap_left, ["r0-1", "r0-2", "r1-2"]);
}

#[test]
fn nothing_new_is_one_line() {
    let r = roadmap(|_| {});
    let a = s(one());
    let mut b = one();
    b["at"] = json!("2026-11-20T10:30:00Z");
    let st = status(&r, &s(b), Some(&a), None);
    assert!(st.no_news);
    assert_eq!(
        text(&st),
        "No news since 10:00 (last real check 10:30). Next check on request.\n"
    );
}

#[test]
fn only_what_changed_is_new() {
    let r = roadmap(|_| {});
    let st = status(&r, &s(two()), Some(&s(one())), None);
    assert!(!st.no_news);
    assert_eq!(st.new_noks.len(), 1);
    assert_eq!(st.previous_check.as_deref(), Some("2026-11-20T10:00:00Z"));
}

#[test]
fn a_rewritten_mark_is_new() {
    let r = roadmap(|_| {});
    let mut b = two();
    b["marks"]["u2"]["steps"]["r1-1"]["obs"] = json!("Now it shows an error.");
    let st = status(&r, &s(b), Some(&s(two())), None);
    assert_eq!(st.new_noks.len(), 1);
    assert!(text(&st).contains("Now it shows an error."));
}

#[test]
fn the_timer_line_is_honest() {
    let r = roadmap(|_| {});
    let f = |expires: &str| FollowUp {
        on: true,
        every_minutes: 30,
        started_at: "2026-11-13T10:00:00Z".into(),
        expires_at: expires.into(),
        last_check: "2026-11-20T10:00:00Z".into(),
    };
    let live = status(&r, &s(one()), None, Some(&f("2026-11-20T23:00:00Z")));
    assert_eq!(live.timer, "Next check about 30 minutes after 10:00");
    let gone = status(&r, &s(one()), None, Some(&f("2026-11-20T08:00:00Z")));
    assert_eq!(
        gone.timer,
        "Timer expired on 2026-11-20 08:00; checks are on request now"
    );
    assert!(text(&gone).contains("Timer expired on"));
    let off = status(&r, &s(one()), None, None);
    assert!(text(&off).contains("Checks are on request."));
}

#[test]
fn pages_merge_and_the_latest_time_wins() {
    let mut a = one();
    a["at"] = json!("2026-11-20T10:00:00Z");
    let b = json!({"at": "2026-11-20T10:20:00Z", "marks": {"u3": {"steps": {}}}, "roster": {"u3": {"validator": "", "role": ""}}});
    let m = Snapshot::merge(vec![s(a), s(b)]);
    assert_eq!(m.at, "2026-11-20T10:20:00Z");
    assert!(m.marks.contains_key("u1") && m.marks.contains_key("u3"));
}

#[test]
fn a_snapshot_that_is_not_json_is_refused() {
    assert!(
        Snapshot::parse("[")
            .unwrap_err()
            .starts_with("not a snapshot")
    );
}

#[test]
fn status_text_golden() {
    let r = roadmap(|_| {});
    let st = status(&r, &s(two()), Some(&s(one())), None);
    insta::assert_snapshot!(text(&st));
}

#[test]
fn report_golden() {
    let r = roadmap(|_| {});
    let rec = Records {
        triage: Some("r1-1 · Validator 2: defect, reproduced."),
        decisions: Some("- none yet"),
        preflight: Some("Preflight passed."),
    };
    insta::assert_snapshot!(report(&r, &s(two()), &rec));
}

#[test]
fn report_mode_b_has_the_traceability_table() {
    let r = roadmap(|v| {
        v["mode"] = json!("B");
        v["features"] = json!(["010"]);
        v["scripts"][1]["steps"][0]["traces"] = json!(["specs/010-orders/spec.md#US1-AS1"]);
        v["not_testable"] =
            json!([{"trace": "specs/010-orders/spec.md#US1-AS2", "reason": "needs a bank"}]);
    });
    let out = report(&r, &s(two()), &Records::default());
    assert!(out.contains("## Scenario traceability"));
    assert!(out.contains("| specs/010-orders/spec.md#US1-AS1 | r1-1 |"));
    assert!(out.contains("not testable by hand: needs a bank"));
}

// preflight

fn declared_ok() -> Declared {
    serde_json::from_value(json!({
        "db": {"rules": [
            {"path": "marks", "read": "view", "write": "owner"},
            {"path": "marks/{self}", "write": "interact"},
            {"path": "roster", "read": "view", "write": "owner"},
            {"path": "proposals", "read": "view", "write": "owner"},
            {"path": "proposals/{self}", "write": "interact"},
            {"path": "meta", "read": "view", "write": "owner"},
            {"path": "triage", "read": "view", "write": "owner"}
        ]},
        "user": {"scopes": ["profile"]}
    }))
    .unwrap()
}

fn read(level: &str, hide: &[&str]) -> Read {
    let mut cols = serde_json::Map::new();
    for c in ["marks", "roster", "proposals", "meta", "triage"] {
        if !hide.contains(&c) {
            cols.insert(c.into(), json!({"readable": true, "docs": 1}));
        }
    }
    serde_json::from_value(json!({"level": level, "collections": cols})).unwrap()
}

fn reads() -> Vec<Read> {
    vec![read("view", &[]), read("interact", &[]), read("admin", &[])]
}

#[test]
fn checklist_names_the_fixes_and_the_pass() {
    let r = roadmap(|v| {
        v["validators"][0]["planned_sharing"] = json!("viewer");
        v["validators"][1]["planned_sharing"] = json!("");
        v["validators"][1]["outside_org"] = json!(true);
        v["validators"].as_array_mut().unwrap().push(json!({
            "key": "v3", "label": "Rui", "assigned_role": "clerk", "planned_sharing": "contributor"
        }));
    });
    let c = checklist(&r);
    let states: Vec<_> = c.lines.iter().map(|l| l.state).collect();
    assert_eq!(states, ["fix", "ask", "pass"]);
    assert!(c.action_needed);
    assert!(c.lines[0].change.contains("Ana") && c.lines[0].change.contains("Contributor"));
    assert_eq!(c.lines[1].caveats.len(), 2);
    assert!(c.lines[1].caveats[0].contains("link sharing"));
    assert!(c.lines.iter().all(|l| l.confirmation == "pending"));
    insta::assert_snapshot!(checklist_text(&c));
}

#[test]
fn editors_get_the_republish_warning() {
    let r = roadmap(|v| {
        v["validators"][0]["planned_sharing"] = json!("editor");
        v["validators"][1]["planned_sharing"] = json!("contributor");
    });
    let c = checklist(&r);
    assert!(!c.action_needed);
    assert!(c.warnings[0].contains("republish"));
    assert!(
        c.lines[0]
            .caveats
            .iter()
            .any(|x| x.contains("Editors can also republish"))
    );
}

#[test]
fn preflight_passes_on_a_matching_declaration_and_reads() {
    let r = roadmap(|_| {});
    let v = verify(&r, &declared_ok(), &reads());
    assert!(v.ok, "{}", verify_text(&v));
    assert_eq!(v.checks.iter().filter(|c| c.kind == "read").count(), 15);
    let t = verify_text(&v);
    assert!(t.contains("Preflight passed."));
    assert!(t.contains("What this proves:") && t.contains("What this does not prove:"));
    assert!(t.contains("sharing level"));
}

#[test]
fn a_missing_collection_is_a_mismatch() {
    let r = roadmap(|_| {});
    let mut rs = reads();
    rs[1] = read("interact", &["triage"]);
    let v = verify(&r, &declared_ok(), &rs);
    assert!(!v.ok);
    let bad: Vec<_> = v.checks.iter().filter(|c| !c.pass).collect();
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].what, "triage at interact");
    assert_eq!(bad[0].seen, "missing");
    assert!(verify_text(&v).contains("do not send the invitation"));
}

#[test]
fn a_hidden_collection_and_a_missing_level_are_mismatches() {
    let r = roadmap(|_| {});
    let mut rs = reads();
    rs[0].collections.get_mut("marks").unwrap().readable = false;
    rs.pop();
    let v = verify(&r, &declared_ok(), &rs);
    let seen: Vec<_> = v
        .checks
        .iter()
        .filter(|c| !c.pass)
        .map(|c| c.seen.as_str())
        .collect();
    assert!(seen.contains(&"hidden") && seen.contains(&"no read"));
}

#[test]
fn a_wrong_declared_write_rule_is_a_mismatch() {
    let r = roadmap(|_| {});
    let mut d = declared_ok();
    d.db.as_mut().unwrap().rules[0].write = Some("interact".into());
    let v = verify(&r, &d, &reads());
    let bad: Vec<_> = v.checks.iter().filter(|c| !c.pass).collect();
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].what, "rule marks");
    assert_eq!(bad[0].seen, "read view write interact");
}

#[test]
fn an_extra_rule_a_missing_scope_and_assets_are_mismatches() {
    let r = roadmap(|_| {});
    let mut d = declared_ok();
    d.db.as_mut().unwrap().rules.push(super::preflight::Rule {
        path: "secrets".into(),
        read: Some("view".into()),
        write: None,
    });
    d.user = None;
    d.assets = Some(json!({}));
    let v = verify(&r, &d, &reads());
    let bad: Vec<_> = v
        .checks
        .iter()
        .filter(|c| !c.pass)
        .map(|c| c.what.as_str())
        .collect();
    assert_eq!(bad, ["rule secrets", "user scope profile", "assets"]);
    let on = roadmap(|v| v["settings"] = json!({"attachments": {"on": true, "budget_mb": 5}}));
    let none = verify(&on, &declared_ok(), &reads());
    assert!(none.checks.iter().any(|c| c.what == "assets" && !c.pass));
}
