//! `compact::proto`: the spike on the matching fixtures at every level (spec 037 T061, T063b).
use compactcore::{compact, verify, Lang};
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

/// File, language and the output sizes of `compact.py` 3.x (Pygments 2.21.0, tree-sitter-language-pack)
/// at levels 1 to 4, measured with `compact.py -q -LN FILE | wc -c`.
const PYTHON: &[(&str, Lang, [usize; 4])] = &[
    ("cache.rs", Lang::Rust, [1550, 1378, 1332, 1243]),
    ("Inventory.java", Lang::Java, [2530, 2096, 2046, 1894]),
    ("service.ts", Lang::TypeScript, [1567, 1491, 1443, 1309]),
    ("Widget.tsx", Lang::Tsx, [568, 440, 425, 396]),
    ("app.mjs", Lang::JavaScript, [1473, 1341, 1281, 1162]),
    ("nosemi.mjs", Lang::JavaScript, [446, 414, 401, 352]),
    ("matrix.c", Lang::C, [1102, 984, 965, 890]),
    ("graph.cpp", Lang::Cpp, [1260, 1139, 1104, 983]),
    ("Program.cs", Lang::CSharp, [1382, 1104, 1079, 1014]),
    ("server.go", Lang::Go, [1533, 1456, 1422, 1309]),
    ("Shapes.kt", Lang::Kotlin, [1705, 1421, 1396, 1282]),
    ("site.css", Lang::Css, [401, 381, 364, 346]),
    ("theme.scss", Lang::Scss, [182, 162, 162, 162]),
];

#[test]
fn proto_every_fixture_at_every_level() {
    for (file, lang, py) in PYTHON {
        let src = fixture(file);
        for level in 1..=4u8 {
            let (out, info) =
                compact(&src, *lang, level).unwrap_or_else(|e| panic!("{file} L{level}: {e}"));
            // the scss fixture has `//` comments: its hazard caps it at level 2 (`EXPECTED`)
            let want = level.min(if *lang == Lang::Scss { 2 } else { 4 });
            assert!(
                info.fallback.is_empty(),
                "{file} L{level}: {:?}",
                info.fallback
            );
            assert_eq!(info.level(), want, "{file}");
            // SC-005: not more than 0.5% larger than the Python output
            let limit = py[usize::from(level) - 1] as f64 * 1.005;
            assert!(
                out.len() as f64 <= limit,
                "{file} L{level}: {} bytes, python {}",
                out.len(),
                py[usize::from(level) - 1]
            );
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
fn tsx_joins_jsx_like_the_python_tool() {
    let src = fixture("Widget.tsx");
    let (out, info) = compact(&src, Lang::Tsx, 4).unwrap();
    assert_eq!(info.level(), 4);
    // text lines lose their indentation, a newline between text and a tag disappears
    assert!(
        out.contains("</h2><p>Some text that\nspans lines</p>"),
        "{out}"
    );
    // a space inside JSX children is text and stays
    assert!(out.contains("{title} ({count})"), "{out}");
    // attributes on one line are not separated by a newline
    assert!(out.contains("key={it}onClick={"), "{out}");
}

#[test]
fn js_writes_asi_newlines_as_semicolons() {
    let src = "const a = 1\nconst b = 2\nlet c = a\n+ b\n";
    let (out, info) = compact(src, Lang::JavaScript, 4).unwrap();
    assert_eq!(info.level(), 4);
    // `a\n+ b` is not joined (a `+` can start a statement), as in compact.py
    assert_eq!(out, "const a=1;const b=2;let c=a\n+b\n");
}

#[test]
fn preprocessor_lines_stay_on_their_own_lines() {
    let src = "#include <stdio.h>\n#define N 3 /* three */\nint main(void) {\n    return N;\n}\n";
    let (out, info) = compact(src, Lang::C, 4).unwrap();
    assert_eq!(info.level(), 4);
    assert_eq!(
        out,
        "#include <stdio.h>\n#define N 3 /* three */\nint main(void){return N;}\n"
    );
    // a backslash continuation belongs to the directive
    let src = "#define M(a) \\\n    do { a; } while (0)\nint x;\n";
    let (out, _) = compact(src, Lang::C, 4).unwrap();
    assert!(
        out.starts_with("#define M(a) \\\n    do { a; } while (0)\nint x;"),
        "{out}"
    );
}

#[test]
fn go_joins_with_the_semicolons_its_lexer_inserts() {
    let src = "package main\n\nfunc f() int {\n\tx := 1\n\treturn x\n}\n";
    let (out, info) = compact(src, Lang::Go, 4).unwrap();
    assert_eq!(info.level(), 4);
    assert_eq!(out, "package main;func f()int{x:=1;return x}\n");
    // a missing semicolon is caught: joining after `x := 1` without one changes the program
    assert!(verify(src, "package main;func f()int{x:=1 return x}\n", Lang::Go).is_err());
    assert!(verify(src, "package main;func f()int{x:=1;return x}\n", Lang::Go).is_ok());
}

#[test]
fn kotlin_keeps_newlines_where_the_grammar_needs_them() {
    // `return` ends the statement at a newline: it must not be joined with what follows
    let src = "fun f(x: Int): Int {\n    if (x > 0) return\n    x\n    return x\n}\n";
    let (out, info) = compact(src, Lang::Kotlin, 4).unwrap();
    assert!(info.fallback.is_empty(), "{:?}", info.fallback);
    // the newline after a bare `return` stays (the same output as compact.py)
    assert_eq!(out, "fun f(x:Int):Int{if(x>0)return\nx;return x}\n");
}

#[test]
fn css_keeps_descendant_combinators() {
    let (out, _) = compact("a  b > c {\n  color : red ;\n}\n", Lang::Css, 4).unwrap();
    assert_eq!(out, "a b > c{color :red;}\n"); // as compact.py: only a gap next to `{};,:` goes
}

#[test]
fn scss_line_comments_cap_the_level() {
    assert_eq!(Lang::Scss.max_level("a { b: c } // x"), 2);
    assert_eq!(Lang::Scss.max_level("a { background: url(http://x) }"), 4);
    assert_eq!(Lang::Css.max_level("// not a hazard for plain css"), 4);
}

#[test]
fn csharp_raw_strings_stay_whole() {
    let src = "class A {\n    string s = \"\"\"\n        a  \"quoted\"  b\n        \"\"\";\n}\n";
    let (out, info) = compact(src, Lang::CSharp, 4).unwrap();
    assert!(info.fallback.is_empty(), "{:?}", info.fallback);
    assert!(
        out.contains("\"\"\"\n        a  \"quoted\"  b\n        \"\"\""),
        "{out}"
    );
}

#[test]
fn proto_verifier_rejects_a_changed_program() {
    let src = "const a = 1\n+b;\n";
    let ok = "const a = 1 +b;\n";
    assert!(verify(src, ok, Lang::TypeScript).is_ok());
    let src = "function f() {\n  return\n  1;\n}\n";
    let bad = "function f() {return 1;}\n";
    assert!(verify(src, bad, Lang::TypeScript).is_err());
    // a changed string is a changed token
    assert!(verify("let s = \"a  b\";\n", "let s=\"a b\";\n", Lang::TypeScript).is_err());
}
