//! The command line of `ai-gent-compact` (spec 037 T066): paths, directories, stdin, `-o`, `-n`,
//! `--outline`, `--list-languages`, languages outside the D29 scope, and the read-only guarantee.
use assert_cmd::Command;
use predicates::prelude::PredicateStrExt;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "..",
        "plugins/compact/skills/code/tests/fixtures",
    ]
    .iter()
    .collect()
}

fn bin() -> Command {
    Command::cargo_bin("ai-gent-compact").unwrap()
}

#[test]
fn compacts_a_file_and_summarises_on_stderr() {
    let out = bin()
        .arg("-L4")
        .arg(fixtures().join("cache.rs"))
        .assert()
        .success();
    let o = out.get_output();
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("rust L4"), "{err}");
    assert!(err.contains("verify=lex+ast"), "{err}");
    assert!(!String::from_utf8_lossy(&o.stdout).contains("compact:"));
}

#[test]
fn quiet_prints_no_summary() {
    let out = bin()
        .args(["-q", "-L4"])
        .arg(fixtures().join("cache.rs"))
        .assert()
        .success();
    assert!(out.get_output().stderr.is_empty());
}

#[test]
fn stdin_needs_a_language_and_works_with_one() {
    bin()
        .args(["-q", "-l", "rust", "-L4", "-"])
        .write_stdin("fn main() {\n    let x = 1;\n}\n")
        .assert()
        .success()
        .stdout("fn main(){let x=1;}\n");
    bin()
        .args(["-"])
        .write_stdin("x\n")
        .assert()
        .success()
        .stdout("x\n")
        .stderr(predicates_contains("unknown language"));
}

fn predicates_contains(s: &'static str) -> impl predicates::Predicate<[u8]> {
    predicates::str::contains(s).from_utf8()
}

#[test]
fn outline_only_languages_come_back_as_they_are() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.py");
    std::fs::write(&p, "def f():\n    return 1\n").unwrap();
    bin()
        .arg(&p)
        .assert()
        .success()
        .stdout("def f():\n    return 1\n")
        .stderr(predicates_contains("outline only, no compaction"));
    // the same for a language that has a level in the table but no grammar here
    let y = dir.path().join("a.yaml");
    std::fs::write(&y, "a:   1\n").unwrap();
    bin()
        .arg("-q")
        .arg(&y)
        .assert()
        .success()
        .stdout("a:   1\n");
}

#[test]
fn numbers_point_at_the_original_lines() {
    let path = fixtures().join("Inventory.java");
    let lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    let out = bin()
        .args(["-q", "-L3", "--outline", "1", "-n"])
        .arg(&path)
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let mut checked = 0;
    for row in text.lines() {
        let Some((num, rest)) = row.split_once('|') else {
            continue;
        };
        let Ok(n) = num.trim().parse::<usize>() else {
            continue;
        };
        let head = rest.split_whitespace().next().unwrap_or("");
        assert!(
            lines[n - 1].trim().starts_with(head) || lines[n - 1].contains(head),
            "{n}: {rest:.60}"
        );
        checked += 1;
    }
    assert!(checked > 5, "{text}");
}

#[test]
fn outline_keeps_top_level_items_on_their_own_lines() {
    let out = bin()
        .args(["-q", "-L4", "--outline", "0", "-l", "rust", "-"])
        .write_stdin("fn a() {\n    1;\n}\nfn b() {\n    2;\n}\n")
        .assert()
        .success();
    assert_eq!(
        String::from_utf8_lossy(&out.get_output().stdout),
        "fn a(){1;}\nfn b(){2;}\n"
    );
}

#[test]
fn strip_comments_keeps_directives() {
    let src = "//go:build linux\n// plain comment\npackage main\n\nfunc f() {} // trailing\n";
    let out = bin()
        .args(["-q", "-L3", "-c", "-l", "go", "-"])
        .write_stdin(src)
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.contains("//go:build linux"), "{text}");
    assert!(
        !text.contains("plain comment") && !text.contains("trailing"),
        "{text}"
    );
}

#[test]
fn paragraphs_keep_a_blank_line() {
    let src = "fn a() {}\n\nfn b() {}\n";
    let out = bin()
        .args(["-q", "-L3", "-p", "-l", "rust", "-"])
        .write_stdin(src)
        .assert()
        .success();
    assert_eq!(
        String::from_utf8_lossy(&out.get_output().stdout),
        "fn a() {}\n\nfn b() {}\n"
    );
}

#[test]
fn a_directory_is_walked_in_order_with_headers() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("b")).unwrap();
    std::fs::create_dir_all(d.path().join("node_modules")).unwrap();
    std::fs::write(d.path().join("z.rs"), "fn z() {}\n").unwrap();
    std::fs::write(d.path().join("a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(d.path().join("b/c.rs"), "fn c() {}\n").unwrap();
    std::fs::write(d.path().join("node_modules/x.rs"), "fn x() {}\n").unwrap();
    std::fs::write(d.path().join("notes.txt"), "text\n").unwrap();
    let out = bin().args(["-q", "-L4"]).arg(d.path()).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    let order: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with("==> "))
        .map(|l| l.rsplit('/').next().unwrap())
        .collect();
    assert_eq!(order, ["a.rs <==", "z.rs <==", "c.rs <=="], "{text}");
    assert!(!text.contains("fn x"));
}

#[test]
fn list_languages_matches_the_python_tool_for_the_compacted_ones() {
    let golden = std::fs::read_to_string(
        [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "fixtures/compact/list-languages.golden",
        ]
        .iter()
        .collect::<PathBuf>(),
    )
    .unwrap();
    let out = bin().arg("--list-languages").assert().success();
    let got = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let rows: std::collections::HashMap<&str, &str> = got
        .lines()
        .map(|l| (l.split_whitespace().next().unwrap(), l))
        .collect();
    assert_eq!(rows.len(), golden.lines().count());
    for l in golden.lines() {
        let key = l.split_whitespace().next().unwrap();
        let ours = rows[key];
        if ours.contains("outline only") {
            assert!(ours.contains("max L0"), "{ours}");
        } else {
            assert_eq!(ours.split(" | ").next().unwrap(), l, "{key}");
        }
    }
    for k in [
        "java",
        "csharp",
        "c",
        "cpp",
        "go",
        "rust",
        "javascript",
        "kotlin",
        "css",
        "typescript",
        "tsx",
    ] {
        assert!(!rows[k].contains("outline only"), "{k}");
    }
}

#[test]
fn it_never_writes_next_to_its_input() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("A.java");
    std::fs::copy(fixtures().join("Inventory.java"), &p).unwrap();
    let before = (
        std::fs::read(&p).unwrap(),
        std::fs::metadata(&p).unwrap().modified().unwrap(),
    );
    bin()
        .args(["-q", "-o"])
        .arg(d.path().join("view.txt"))
        .arg(&p)
        .assert()
        .success();
    assert_eq!(
        (
            std::fs::read(&p).unwrap(),
            std::fs::metadata(&p).unwrap().modified().unwrap()
        ),
        before
    );
    assert!(d.path().join("view.txt").is_file());
    // there is no in-place option
    bin().args(["-i"]).arg(&p).assert().failure();
    let mut names: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["A.java", "view.txt"]);
}

#[test]
fn unreadable_input_is_exit_1_and_binary_files_are_skipped() {
    bin().arg("/no/such/file.rs").assert().code(1);
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("a.rs");
    std::fs::write(&p, b"fn a() {\0}\n").unwrap();
    bin()
        .arg(&p)
        .assert()
        .success()
        .stderr(predicates_contains("skipping binary file"));
}

#[test]
fn level_0_is_the_source_and_unparsable_files_are_checked_lexically() {
    let src = "fn broken( {\n  \n";
    bin()
        .args(["-q", "-L0", "-l", "rust", "-"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout(src);
    // a source the grammar cannot parse is compacted and checked lexically, as compact.py does
    bin()
        .args(["-L4", "-l", "rust", "-"])
        .write_stdin(src)
        .assert()
        .success()
        .stdout("fn broken({\n")
        .stderr(predicates_contains("verify=lex"));
}
