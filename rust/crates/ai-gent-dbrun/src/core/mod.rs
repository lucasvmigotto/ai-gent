//! Driver-free port of dbrun's safety core (scan, classify, cost, mask, redact, network).
//!
//! Moved from the spec 038 spike; `tests/golden.rs` runs it against `rust/fixtures/db-core/golden.json`,
//! made by the Python `dbrun.py`. Reasons are part of the contract, so they match the Python text.
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

pub mod mask;
pub mod plan;
pub mod tls;

macro_rules! re {
    ($p:expr) => {{
        static R: OnceLock<Regex> = OnceLock::new();
        R.get_or_init(|| Regex::new($p).unwrap())
    }};
}

/// A refusal with the message the runner shows.
#[derive(Debug, PartialEq, Eq)]
pub struct Refused(pub String);

/// `(backslash escapes, bracket quoting)` for an engine: MySQL and MariaDB escape with a backslash,
/// SQL Server quotes identifiers with brackets.
#[must_use]
pub fn parse_modes(engine: &str) -> (bool, bool) {
    (
        engine == "mysql" || engine == "mariadb",
        engine == "sqlserver",
    )
}

fn starts(c: &[char], i: usize, s: &str) -> bool {
    let b: Vec<char> = s.chars().collect();
    c.len() >= i + b.len() && c[i..i + b.len()] == b[..]
}
fn find(c: &[char], from: usize, s: &str) -> Option<usize> {
    (from..c.len()).find(|&k| starts(c, k, s))
}

/// Returns (original, blanked) per statement.
#[must_use]
pub fn scan(sql: &str, backslash: bool, brackets: bool) -> Vec<(String, String)> {
    let c: Vec<char> = sql.chars().collect();
    let n = c.len();
    let mut stmts = vec![];
    let (mut orig, mut blank) = (String::new(), String::new());
    let closer = |ch: char| -> Option<char> {
        match ch {
            '\'' => Some('\''),
            '"' => Some('"'),
            '`' => Some('`'),
            '[' if brackets => Some(']'),
            _ => None,
        }
    };
    fn flush(stmts: &mut Vec<(String, String)>, orig: &mut String, blank: &mut String) {
        let (o, b) = (orig.trim().to_string(), blank.trim().to_string());
        if !b.is_empty() {
            stmts.push((o, b));
        }
        orig.clear();
        blank.clear();
    }
    let mut i = 0;
    while i < n {
        let ch = c[i];
        if starts(&c, i, "--") {
            let j = find(&c, i, "\n").unwrap_or(n);
            orig.extend(&c[i..j]);
            blank.push(' ');
            i = j;
            continue;
        }
        if starts(&c, i, "/*") {
            let j = find(&c, i + 2, "*/").map_or(n, |j| j + 2);
            orig.extend(&c[i..j]);
            blank.push(' ');
            i = j;
            continue;
        }
        if ch == '$' {
            let mut k = i + 1;
            if k < n && (c[k].is_ascii_alphabetic() || c[k] == '_') {
                while k < n && (c[k].is_ascii_alphanumeric() || c[k] == '_') {
                    k += 1;
                }
            }
            if k < n && c[k] == '$' {
                let tag: String = c[i..=k].iter().collect();
                let tl = tag.chars().count();
                let j = find(&c, i + tl, &tag).map_or(n, |j| j + tl);
                orig.extend(&c[i..j]);
                blank.push_str(" '' ");
                i = j;
                continue;
            }
        }
        if let Some(close) = closer(ch) {
            let mut j = i + 1;
            while j < n {
                if backslash && c[j] == '\\' && (close == '\'' || close == '"') {
                    j += 2;
                    continue;
                }
                if c[j] == close {
                    if close != ']' && j + 1 < n && c[j + 1] == close {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            let end = (j + 1).min(n);
            orig.extend(&c[i..end]);
            blank.push_str(if ch == '\'' { " '' " } else { " x " });
            i = j + 1;
            continue;
        }
        if ch == ';' {
            flush(&mut stmts, &mut orig, &mut blank);
            i += 1;
            continue;
        }
        orig.push(ch);
        blank.push(ch);
        i += 1;
    }
    flush(&mut stmts, &mut orig, &mut blank);
    stmts
}

/// The statement text with comments and literals blanked, statements joined by a space.
#[must_use]
pub fn blank_literals(sql: &str, engine: &str) -> String {
    let (b, k) = parse_modes(engine);
    scan(sql, b, k)
        .into_iter()
        .map(|(_, b)| b)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Splits `sql` into statements, refusing text that splits differently with and without backslash
/// escaping (the engine and the classifier could then disagree on where a statement ends).
///
/// # Errors
/// [`Refused`] when the two splits differ.
pub fn split_statements(sql: &str, engine: &str) -> Result<Vec<String>, Refused> {
    let (b, k) = parse_modes(engine);
    let mine: Vec<String> = scan(sql, b, k).into_iter().map(|x| x.0).collect();
    let other: Vec<String> = scan(sql, !b, k).into_iter().map(|x| x.0).collect();
    if mine != other {
        return Err(Refused("the statements split differently depending on backslash escaping; rewrite the string literals without backslashes".into()));
    }
    Ok(mine)
}

const READ_START: &[&str] = &[
    "SELECT", "WITH", "SHOW", "DESCRIBE", "DESC", "EXPLAIN", "VALUES", "TABLE", "PRAGMA",
];
const DDL_START: &[&str] = &[
    "CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME", "GRANT", "REVOKE", "COMMENT",
];
const WRITE_WORDS: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "MERGE", "UPSERT", "TRUNCATE", "DROP", "ALTER", "CREATE",
    "GRANT", "REVOKE",
];
const READ_PRAGMAS: &[&str] = &[
    "TABLE_INFO",
    "TABLE_XINFO",
    "TABLE_LIST",
    "INDEX_LIST",
    "INDEX_INFO",
    "INDEX_XINFO",
    "FOREIGN_KEY_LIST",
    "DATABASE_LIST",
    "COMPILE_OPTIONS",
    "PAGE_COUNT",
    "PAGE_SIZE",
    "FREELIST_COUNT",
    "FOREIGN_KEY_CHECK",
    "INTEGRITY_CHECK",
    "QUICK_CHECK",
    "COLLATION_LIST",
    "FUNCTION_LIST",
    "PRAGMA_LIST",
    "USER_VERSION",
    "SCHEMA_VERSION",
    "ENCODING",
    "JOURNAL_MODE",
    "FOREIGN_KEYS",
];

fn locking() -> &'static Vec<Regex> {
    static L: OnceLock<Vec<Regex>> = OnceLock::new();
    L.get_or_init(|| {
        [
            r"(?i)\bFOR\s+(NO\s+KEY\s+)?UPDATE\b",
            r"(?i)\bFOR\s+(KEY\s+)?SHARE\b",
            r"(?i)\bLOCK\s+IN\s+SHARE\s+MODE\b",
            r"(?i)\bINTO\b",
            r"(?i)\b(UPDLOCK|XLOCK|HOLDLOCK|TABLOCKX?|PAGLOCK|SERIALIZABLE|REPEATABLEREAD)\b",
        ]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect()
    })
}

fn words(s: &str) -> Vec<String> {
    re!(r"[A-Za-z_][A-Za-z0-9_$]*")
        .find_iter(s)
        .map(|m| m.as_str().to_string())
        .collect()
}

/// Classifies one statement as `"read"` or `"write"`, with the reason. Anything not provably read-only
/// is a write.
#[must_use]
pub fn classify_statement(stmt: &str, engine: &str) -> (&'static str, String) {
    let text = blank_literals(stmt, engine);
    let w = words(&text);
    if w.is_empty() {
        return ("write", "empty or unparseable statement".into());
    }
    let upper: Vec<String> = w.iter().map(|x| x.to_uppercase()).collect();
    let first = upper[0].as_str();
    if !READ_START.contains(&first) {
        return ("write", format!("starts with {first}"));
    }
    if first == "PRAGMA" {
        let name = upper.get(1).map_or("", String::as_str);
        if engine != "sqlite" || !READ_PRAGMAS.contains(&name) || text.contains('=') {
            return (
                "write",
                format!("PRAGMA {} can change the database", name.to_lowercase()),
            );
        }
    }
    if first == "EXPLAIN" {
        if engine == "oracle" {
            return ("write", "Oracle EXPLAIN PLAN writes to PLAN_TABLE".into());
        }
        if upper.iter().any(|x| x == "ANALYZE" || x == "ANALYSE") {
            return ("write", "EXPLAIN ANALYZE executes the statement".into());
        }
    }
    let set: HashSet<&str> = WRITE_WORDS.iter().copied().collect();
    let mut hit: Vec<&String> = upper[1..]
        .iter()
        .filter(|x| set.contains(x.as_str()))
        .collect();
    if !hit.is_empty() {
        hit.sort();
        return ("write", format!("contains {}", hit[0]));
    }
    for p in locking() {
        if let Some(m) = p.find(&text) {
            return (
                "write",
                format!("takes locks or writes ({})", m.as_str().to_uppercase()),
            );
        }
    }
    if re!(r"(?i)\bNEXT\s+VALUE\s+FOR\b|\.\s*NEXTVAL\b").is_match(&text) {
        return (
            "write",
            "advances a sequence, which a rollback doesn't undo".into(),
        );
    }
    if let Some(c) = re!(r"(?i)\b(NEXTVAL|SETVAL|TXID_CURRENT|PG_ADVISORY\w*|PG_TRY_ADVISORY\w*|PG_SLEEP\w*|PG_TERMINATE_BACKEND|PG_CANCEL_BACKEND|PG_RELOAD_CONF|PG_ROTATE_LOGFILE|PG_READ_FILE|PG_READ_BINARY_FILE|PG_LS_DIR|PG_STAT_FILE|SET_CONFIG|LO_IMPORT|LO_EXPORT|LO_UNLINK|DBLINK\w*|SLEEP|BENCHMARK|GET_LOCK|RELEASE_LOCK|RELEASE_ALL_LOCKS|LOAD_FILE|XP_\w+|SP_\w+|OPENROWSET|OPENQUERY|OPENDATASOURCE|WAITFOR|UTL_\w+|SYS_EXEC|SYS_EVAL|LOAD_EXTENSION|WRITEFILE|READFILE)\s*\(").captures(&text) {
        return ("write", format!("calls {}, which has side effects", c[1].to_uppercase()));
    }
    if let Some(m) = oracle_package(&text) {
        return ("write", format!("calls the {} package", m.to_uppercase()));
    }
    ("read", "read-only statement".into())
}

/// Python: `\b(DBMS_(?!METADATA\b|XPLAN\b)\w+|UTL_\w+|OWA_\w+|HTP|HTF)\b` (no lookahead in the regex crate).
fn oracle_package(text: &str) -> Option<String> {
    for m in re!(r"(?i)\b(DBMS_\w+|UTL_\w+|OWA_\w+|HTP|HTF)\b").find_iter(text) {
        let s = m.as_str();
        if s.len() > 5 && s[..5].eq_ignore_ascii_case("DBMS_") {
            let rest = s[5..].to_uppercase();
            if rest == "METADATA" || rest == "XPLAN" {
                continue;
            }
        }
        return Some(s.to_string());
    }
    None
}

/// Whether the statement is DDL (it starts with a DDL keyword).
#[must_use]
pub fn is_ddl(stmt: &str, engine: &str) -> bool {
    let t = blank_literals(stmt, engine);
    re!(r"[A-Za-z_]+")
        .find(&t)
        .is_some_and(|m| DDL_START.contains(&m.as_str().to_uppercase().as_str()))
}

/// Whether every table the statement reads is a system catalog (exempt from the cost rules).
#[must_use]
pub fn catalog_only(text: &str) -> bool {
    let cat = re!(
        r"(?i)^(information_schema|pg_catalog|pg_\w+|sys|all_\w+|user_\w+|dba_\w+|v\$\w+|gv\$\w+|sqlite_\w+|pragma_\w+|performance_schema|mysql)$"
    );
    let t = re!(r"(?i)\b(?:FROM|JOIN)\s+([A-Za-z_][\w$]*)(?:\s*\.\s*([A-Za-z_][\w$]*))?");
    t.captures_iter(text).all(|c| cat.is_match(&c[1]))
}

/// The cost rule a statement breaks on a non-local database (`SELECT *`, an unfiltered `COUNT`), if any.
#[must_use]
pub fn cost_rules(stmt: &str, cls: &str, allow_full_count: bool, engine: &str) -> Option<String> {
    if cls == "local" {
        return None;
    }
    let text = blank_literals(stmt, engine);
    if catalog_only(&text) {
        return None;
    }
    if re!(r"(?i)\bSELECT\s+(DISTINCT\s+|TOP\s*\(?\s*\d+\s*\)?\s+)?([A-Za-z_][A-Za-z0-9_]*\.)?\*")
        .is_match(&text)
    {
        return Some("name the columns instead of SELECT * (safety.md § Cost)".into());
    }
    if re!(r"(?i)\bCOUNT\s*\(").is_match(&text)
        && !re!(r"(?i)\bWHERE\b").is_match(&text)
        && !allow_full_count
    {
        return Some("an unfiltered COUNT scans the whole table — use the catalog's row estimate, filter on an indexed column, or pass --allow-full-count if the user asked for an exact full count".into());
    }
    None
}

/// The container-engine arguments that join the database container's network namespace.
#[must_use]
pub fn network_args(container_id: &str, pod: &str, pid: Option<u64>) -> Vec<String> {
    match (pod.is_empty(), pid) {
        (false, Some(p)) => vec!["--network".into(), format!("ns:/proc/{p}/ns/net")],
        _ => vec!["--network".into(), format!("container:{container_id}")],
    }
}

/// Whole-word redaction, Python `(?<![\w.-])v(?![\w-])`, by hand: the regex crate has no lookaround.
/// `secrets` is `(value, label)` in the order to apply them.
#[must_use]
pub fn redact(text: &str, secrets: &[(&str, &str)]) -> String {
    let wchar = |c: char| c.is_alphanumeric() || c == '_';
    let mut text = text.to_string();
    for (value, label) in secrets {
        let v = value.trim();
        if v.chars().count() < 2 || v == "127.0.0.1" || v == "localhost" {
            continue;
        }
        let (mut out, mut rest, mut from) = (String::new(), 0, 0);
        while let Some(p) = text[from..].find(v) {
            let s = from + p;
            let e = s + v.len();
            let ok_b = text[..s]
                .chars()
                .next_back()
                .is_none_or(|c| !(wchar(c) || c == '.' || c == '-'));
            let ok_a = text[e..]
                .chars()
                .next()
                .is_none_or(|c| !(wchar(c) || c == '-'));
            if ok_b && ok_a {
                out.push_str(&text[rest..s]);
                out.push_str(label);
                rest = e;
                from = e;
            } else {
                from = s + text[s..].chars().next().map_or(1, char::len_utf8);
            }
        }
        out.push_str(&text[rest..]);
        text = out;
    }
    text
}

/// The engine's `EXPLAIN` form of a statement.
#[must_use]
pub fn explain_sql(engine: &str, stmt: &str) -> String {
    let prefix = match engine {
        "sqlite" => "EXPLAIN QUERY PLAN ",
        "oracle" => "EXPLAIN PLAN FOR ",
        _ => "EXPLAIN ",
    };
    format!("{prefix}{stmt}")
}

/// Refuses an action aimed at an engine's system database.
///
/// # Errors
/// [`Refused`] naming the system database.
pub fn require_user_database(
    engine: &str,
    database: Option<&str>,
    action: &str,
) -> Result<(), Refused> {
    let db = database
        .unwrap_or(if engine == "sqlserver" { "master" } else { "" })
        .to_lowercase();
    let system: &[&str] = match engine {
        "sqlserver" => &["master", "model", "msdb", "tempdb"],
        "mysql" | "mariadb" => &["mysql", "sys", "information_schema", "performance_schema"],
        "postgresql" => &["template0", "template1"],
        _ => &[],
    };
    if system.contains(&db.as_str()) {
        return Err(Refused(format!(
            "{action} targets the system database '{db}'; create a database for the project and add a profile for it (<PROFILE>_CLASS=local, <PROFILE>_CONTAINER=<container>, <PROFILE>_DATABASE=<name>; see connections.md)"
        )));
    }
    Ok(())
}
