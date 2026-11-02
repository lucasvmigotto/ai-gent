//! `compact::proto`: the prototype on the matching fixtures at every level.
use compactcore::{compact, Lang};
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let p: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "..",
        "plugins/compact/skills/code/tests/fixtures",
        name,
    ]
    .iter()
    .collect();
    std::fs::read_to_string(p).unwrap()
}

#[test]
fn proto_every_fixture_at_every_level() {
    for (file, lang) in [
        ("cache.rs", Lang::Rust),
        ("Inventory.java", Lang::Java),
        ("service.ts", Lang::TypeScript),
        ("Widget.tsx", Lang::Tsx),
    ] {
        let src = fixture(file);
        for level in 1..=4 {
            let (out, info) =
                compact(&src, lang, level).unwrap_or_else(|e| panic!("{file} L{level}: {e}"));
            println!(
                "{file} L{level}: reached {} fallback {:?} {} -> {} bytes",
                info.level(),
                info.fallback,
                src.len(),
                out.len()
            );
            assert!(
                info.fallback.is_empty(),
                "{file} L{level}: {:?}",
                info.fallback
            );
            assert_eq!(info.level(), level, "{file}");
        }
    }
}

#[test]
fn proto_strings_and_comments_survive() {
    let src = fixture("Inventory.java");
    let (out, _) = compact(&src, Lang::Java, 4).unwrap();
    assert!(out.contains("\"a  b\\tc\""));
    assert!(out.contains("Javadoc with   odd   spacing must survive verbatim."));
    assert!(out.contains("  indented line inside a text block"));
}

#[test]
fn proto_verifier_rejects_a_changed_program() {
    use compactcore::verify;
    let src = "const a = 1\n+b;\n";
    // joining the lines changes nothing for this program; joining after `return` does
    let ok = "const a = 1 +b;\n";
    assert!(verify(src, ok, Lang::TypeScript).is_ok());
    let src = "function f() {\n  return\n  1;\n}\n";
    let bad = "function f() {return 1;}\n";
    assert!(verify(src, bad, Lang::TypeScript).is_err());
    // a changed string is a changed token
    assert!(verify("let s = \"a  b\";\n", "let s=\"a b\";\n", Lang::TypeScript).is_err());
}
