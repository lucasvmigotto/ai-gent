//! Port of `plugins/db/hooks/guard.sh`.
use super::{PREFIX, re};
use crate::guard::event::Event;
use crate::guard::reasons::decide;
use crate::guard::segment::{Shell, View};
use crate::guard::{Decision, Level};
use serde_json::Value;

const CLIENTS: &str = "psql|pg_dump|pg_dumpall|pg_restore|pgcli|mysql|mariadb|mysqldump|mariadb-dump|mysqladmin|mycli|sqlcmd|mssql-cli|bcp|osql|isql|sqlplus|sql|sqlcl|rman|impdp|expdp|imp|exp|mongosh|mongo|mongodump|mongorestore|mongoimport|sqlite3|litecli|usql|cqlsh|clickhouse-client";
const RESTORE: [&str; 5] = ["pg_restore", "impdp", "imp", "mongorestore", "mongoimport"];
const CREDS: &str = r"(^|[^A-Za-z0-9_.-])connections\.(env|toml)($|[^A-Za-z0-9_.-])|\.config/ai-gent($|[^A-Za-z0-9_.-])|XDG_CONFIG_HOME\}?/ai-gent($|[^A-Za-z0-9_.-])|AI_GENT_DB_(CONFIG|CONNECTIONS)";

/// Where the config directory lives on Windows (`%APPDATA%`, `%LOCALAPPDATA%`, `$env:...`), after
/// `\` became `/`. Only the Windows shells look for it.
const WIN_CREDS: &str = r"(APPDATA|LOCALAPPDATA)(%|\}?)/ai-gent($|[^A-Za-z0-9_.-])|[Aa]pp[Dd]ata/(Local|Roaming)/ai-gent($|[^A-Za-z0-9_.-])";

fn windows_path(value: &str) -> bool {
    let b = value.as_bytes();
    value.starts_with("\\\\")
        || (b.len() > 2
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/'))
}

/// A credentials file as a word on a command line (quoted prose blanked first).
#[must_use]
pub fn creds_in_command(v: &View) -> Decision {
    if re(CREDS).is_match(&v.words) || (v.shell != Shell::Sh && re(WIN_CREDS).is_match(&v.words)) {
        decide("db.creds", &[])
    } else {
        Decision::none()
    }
}

/// A credentials file as the target of a non-shell tool. Only path-like keys count, never the
/// text a Write or Edit puts into a file.
#[must_use]
pub fn creds_in_tool(ev: &Event) -> Decision {
    let glob = ev.tool_name == "Glob";
    let r = re(CREDS);
    let mut hit = false;
    visit(&ev.tool_input, &mut |k, val| {
        let key = matches!(k, "file_path" | "notebook_path" | "path" | "glob")
            || (glob && k == "pattern");
        let norm = if windows_path(val) {
            val.replace('\\', "/")
        } else {
            val.to_string()
        };
        if key
            && (r.is_match(val)
                || (windows_path(val) && (r.is_match(&norm) || re(WIN_CREDS).is_match(&norm))))
        {
            hit = true;
        }
    });
    if hit {
        decide("db.creds", &[])
    } else {
        Decision::none()
    }
}

fn visit(v: &Value, f: &mut dyn FnMut(&str, &str)) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                if let Value::String(s) = x {
                    f(k, s);
                } else {
                    visit(x, f);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| visit(x, f)),
        _ => {}
    }
}

/// Evaluate the db guard on a shell command (credentials words first, then direct clients).
#[must_use]
pub fn evaluate(v: &View) -> Decision {
    let d = creds_in_command(v);
    if d.level != Level::None {
        return d;
    }
    clients(v)
}

/// Direct database clients only (no credentials check).
#[must_use]
pub fn clients(v: &View) -> Decision {
    let direct = re(&format!(
        r"{PREFIX}([^[:space:]]*/)?({CLIENTS})([[:space:]]|$)"
    ));
    let wrapped = re(&format!(
        r"(docker|podman|nerdctl|kubectl|oc)[[:space:]].*(exec|run)[[:space:]].*[[:space:]/]({CLIENTS})([[:space:]]|$)"
    ));
    let Some(hit) = v
        .segments
        .iter()
        .find(|s| direct.is_match(s) || wrapped.is_match(s))
    else {
        return Decision::none();
    };
    let tool: String = re(&format!(r"(^|[[:space:]/])({CLIENTS})([[:space:]]|$)"))
        .find(hit)
        .map(|m| {
            m.as_str()
                .chars()
                .filter(|c| *c != ' ' && *c != '/')
                .collect()
        })
        .unwrap_or_default();
    if RESTORE.contains(&tool.as_str()) {
        return decide("db.restore", &[("tool", &tool)]);
    }
    let flat = v
        .raw
        .replace("\\n", " ")
        .replace("\\t", " ")
        .replace(['\n', '\t'], " ")
        .to_ascii_lowercase();
    if re(r"(^|[^a-z_])(insert|update|delete|merge|upsert|truncate|drop|alter|create|grant|revoke|rename|replace[[:space:]]+into)([^a-z_]|$)")
        .is_match(&flat)
    {
        return decide("db.sql_write", &[("tool", &tool)]);
    }
    decide("db.bypass", &[("tool", &tool)])
}
