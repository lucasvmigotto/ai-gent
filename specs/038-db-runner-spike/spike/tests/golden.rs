use dbcore::*;
use serde_json::Value;

fn golden() -> Value {
    serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/golden.json")).unwrap()).unwrap()
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
            bad.push(format!("{e} {s:?}: py={} / {} rust={k} / {why}", r[2], r[3]));
        }
    }
    eprintln!("classify: {n} cases, {kind_bad} verdict diffs, {} verdict-or-reason diffs", bad.len());
    for b in bad.iter().take(10) { eprintln!("  {b}"); }
    assert!(bad.is_empty());
}

#[test]
fn split_ddl_cost_match_python() {
    let g = golden();
    let mut bad = vec![];
    for r in g["split"].as_array().unwrap() {
        let (e, s) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        let got = split_statements(s, e);
        let exp: Vec<String> = r[3].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
        let same = match (&got, r[2].as_str().unwrap()) {
            (Ok(v), "ok") => *v == exp,
            (Err(_), "refused") => true,
            _ => false,
        };
        if !same { bad.push(format!("split {e} {s:?}")); }
    }
    for r in g["ddl"].as_array().unwrap() {
        let (e, s) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        if is_ddl(s, e) != r[2].as_bool().unwrap() { bad.push(format!("ddl {e} {s:?}")); }
    }
    for r in g["cost"].as_array().unwrap() {
        let (e, s, c, a) = (r[0].as_str().unwrap(), r[1].as_str().unwrap(), r[2].as_str().unwrap(), r[3].as_bool().unwrap());
        let got = cost_rules(s, c, a, e);
        // reasons differ in wording by design (shortened); compare presence only
        if got.is_some() != !r[4].is_null() { bad.push(format!("cost {e} {c} {a} {s:?}")); }
    }
    eprintln!("split/ddl/cost diffs: {}", bad.len());
    for b in bad.iter().take(10) { eprintln!("  {b}"); }
    assert!(bad.is_empty());
}

#[test]
fn mask_redact_network_match_python() {
    let g = golden();
    for m in g["mask"].as_array().unwrap() {
        let cols: Vec<String> = m["cols"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().into()).collect();
        let rows: Vec<Vec<Value>> = m["rows"].as_array().unwrap().iter().map(|r| r.as_array().unwrap().clone()).collect();
        let (masked, kinds) = mask::mask_result(&cols, &rows, m["reveal"].as_bool().unwrap());
        let exp: Vec<Vec<Value>> = m["masked"].as_array().unwrap().iter().map(|r| r.as_array().unwrap().clone()).collect();
        assert_eq!(masked, exp, "reveal={}", m["reveal"]);
        let mut k1: Vec<_> = kinds.into_iter().collect();
        k1.sort();
        let mut k2: Vec<(String, String)> = m["kinds"].as_object().unwrap().iter().map(|(a, b)| (a.clone(), b.as_str().unwrap().into())).collect();
        k2.sort();
        assert_eq!(k1, k2);
    }
    for r in g["redact"].as_array().unwrap() {
        let got = redact(r[0].as_str().unwrap(), &[("s3cr3t!", "<password>"), ("db.secret-host.internal", "<host>"), ("app_ro", "<user>")]);
        // python order: password, host, user
        assert_eq!(got, r[1].as_str().unwrap());
    }
    let n = g["network"].as_array().unwrap();
    assert_eq!(network_args("abc", "", None), vec!["--network", "container:abc"]);
    assert_eq!(network_args("abc", "p1", Some(4242)), vec!["--network", "ns:/proc/4242/ns/net"]);
    assert_eq!(n.len(), 2);
}
