#![cfg(feature = "parser")]
use serde_json::Value;
#[test]
fn compare() {
    let g: Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/golden.json")).unwrap()).unwrap();
    let (mut n, mut diff, mut unsafe_) = (0, 0, 0);
    let mut ex = vec![];
    for r in g["classify"].as_array().unwrap() {
        let (e, s, py) = (r[0].as_str().unwrap(), r[1].as_str().unwrap(), r[2].as_str().unwrap());
        let p = dbcore::parsercmp::classify(s, e);
        n += 1;
        if p != py { diff += 1; if p == "read" { unsafe_ += 1; if ex.len() < 12 { ex.push(format!("{e} {s:?}")); } } }
    }
    eprintln!("sqlparser vs dbrun rules: {n} cases, {diff} verdict diffs, {unsafe_} where the parser says READ and dbrun says WRITE");
    for x in ex { eprintln!("  {x}"); }
}
