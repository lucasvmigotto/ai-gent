//! The profiles files: the `.env` file (KEY=VALUE lines) and the TOML file (`[defaults]`,
//! `[families.<name>]`, `[profiles.<NAME>]`), ported from `dbrun.py` (`parse_env_file`, `env_profiles`,
//! `toml_profiles`). Errors name a file, a line, a table or a key, never a value.
use super::ProfileError;
use std::collections::BTreeMap;
use std::path::Path;

/// Largest profiles file read, in bytes.
pub const MAX_PROFILES_FILE_BYTES: u64 = 1024 * 1024;

/// Suffixes of the `.env` keys (`<NAME>_<SUFFIX>`), in the order `dbrun.py` tries them. The last three
/// are the proposed TLS keys (spec 043 T007).
pub const SUFFIXES: [&str; 15] = [
    "ENGINE",
    "CLASS",
    "HOST",
    "PORT",
    "DATABASE",
    "SERVICE",
    "SID",
    "USER",
    "PASSWORD",
    "PATH",
    "CONTAINER",
    "PROJECT",
    "TLS_MODE",
    "TLS_CA_FILE",
    "TLS_SERVER_NAME",
];

/// What a profiles file says about one profile, before classification and secrets.
#[derive(Clone)]
pub struct RawProfile {
    /// The profile name (`[A-Z][A-Z0-9_]*`).
    pub name: String,
    /// The file it came from.
    pub source: std::path::PathBuf,
    /// Lower-case keys (`engine`, `host`, `tls_mode`, ...) to values.
    pub fields: BTreeMap<String, String>,
    /// The TOML family, when the profile names one.
    pub family: Option<String>,
}

impl std::fmt::Debug for RawProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Every value is shown except the ones a secret can sit in.
        let shown: BTreeMap<&str, &str> = self
            .fields
            .iter()
            .map(|(k, v)| {
                let secret = k.contains("password") || k.contains("secret") || k.contains("token");
                (k.as_str(), if secret { "<redacted>" } else { v.as_str() })
            })
            .collect();
        f.debug_struct("RawProfile")
            .field("name", &self.name)
            .field("source", &self.source)
            .field("fields", &shown)
            .field("family", &self.family)
            .finish()
    }
}

/// The lower-case profile keys.
#[must_use]
pub fn profile_keys() -> Vec<String> {
    SUFFIXES.iter().map(|s| s.to_lowercase()).collect()
}

/// Whether `c` ends a line for Python's `str.splitlines()`.
#[must_use]
pub fn is_line_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\x0b'
            | '\x0c'
            | '\x1c'
            | '\x1d'
            | '\x1e'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// Python's `str.splitlines(keepends=True)` as `(content, terminator)` pairs: `\r\n` is one terminator.
/// The single splitter of this module: parsing and every rewrite of a profiles file use it, so they
/// always agree on what a line is.
#[must_use]
pub fn split_lines(text: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut iter = text.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if is_line_break(c) {
            let mut end = i + c.len_utf8();
            if c == '\r' && matches!(iter.peek(), Some((_, '\n'))) {
                iter.next();
                end += 1;
            }
            out.push((&text[start..i], &text[i..end]));
            start = end;
        }
    }
    if start < text.len() {
        out.push((&text[start..], ""));
    }
    out
}

fn is_env_key(k: &str) -> bool {
    let mut c = k.chars();
    c.next()
        .is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_')
}

/// A profile name: `[A-Z][A-Z0-9_]*`.
#[must_use]
pub fn is_profile_name(n: &str) -> bool {
    let mut c = n.chars();
    c.next().is_some_and(|f| f.is_ascii_uppercase())
        && c.all(|x| x.is_ascii_uppercase() || x.is_ascii_digit() || x == '_')
}

/// KEY=VALUE lines; quotes stripped; no expansion, no execution.
///
/// # Errors
/// A line that is not `KEY=VALUE`, named by file and line number.
pub fn parse_env_text(text: &str, file: &str) -> Result<BTreeMap<String, String>, ProfileError> {
    let mut values = BTreeMap::new();
    for (idx, (raw, _)) in split_lines(text).into_iter().enumerate() {
        let lineno = idx + 1;
        let mut line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("export ") {
            line = rest.trim_start();
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(ProfileError::new(format!(
                "{file}:{lineno}: not a KEY=VALUE line"
            )));
        };
        let key = key.trim();
        if !is_env_key(key) {
            return Err(ProfileError::new(format!(
                "{file}:{lineno}: not a KEY=VALUE line"
            )));
        }
        let mut value = value.trim().to_string();
        let chars: Vec<char> = value.chars().collect();
        if chars.len() >= 2
            && chars[0] == chars[chars.len() - 1]
            && (chars[0] == '"' || chars[0] == '\'')
        {
            value = chars[1..chars.len() - 1].iter().collect();
        } else if let Some((head, _)) = value.split_once(" #") {
            value = head.trim_end().to_string();
        }
        values.insert(key.to_string(), value);
    }
    Ok(values)
}

/// Reads and parses a `.env` profiles file.
///
/// # Errors
/// An unreadable, oversized or malformed file.
pub fn read_env_file(path: &Path) -> Result<BTreeMap<String, String>, ProfileError> {
    let text = crate::fsutil::read_text_bounded(path, MAX_PROFILES_FILE_BYTES)
        .map_err(ProfileError::new)?;
    parse_env_text(&text, &file_name(path))
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

/// The profiles of a `.env` file: `<NAME>_<SUFFIX>=value`. The first suffix a key ends with decides
/// (as in `dbrun.py`: even when the name left over is not a valid profile name, the key is then ignored).
#[must_use]
pub fn env_profiles(
    values: &BTreeMap<String, String>,
    path: &Path,
) -> BTreeMap<String, RawProfile> {
    let mut profiles: BTreeMap<String, RawProfile> = BTreeMap::new();
    for (key, value) in values {
        for suffix in SUFFIXES {
            if key.ends_with(&format!("_{suffix}")) && key.len() > suffix.len() + 1 {
                let name = &key[..key.len() - suffix.len() - 1];
                if is_profile_name(name) {
                    profiles
                        .entry(name.to_string())
                        .or_insert_with(|| RawProfile {
                            name: name.to_string(),
                            source: path.to_path_buf(),
                            fields: BTreeMap::new(),
                            family: None,
                        })
                        .fields
                        .insert(suffix.to_lowercase(), value.clone());
                }
                break;
            }
        }
    }
    profiles
}

type Fields = BTreeMap<String, String>;

/// `[defaults]`, `[families.<name>]`, `[profiles.<NAME>]`; a profile's keys win over its family's,
/// which win over the defaults.
///
/// # Errors
/// Invalid TOML (line and column only, the parser's text can quote the line), an unknown table or key, a
/// value of the wrong type, a bad name, or a family that is not defined. Each names a table or key.
pub fn toml_profiles(
    text: &str,
    path: &Path,
) -> Result<BTreeMap<String, RawProfile>, ProfileError> {
    let file = file_name(path);
    let data: toml::Table = text.parse().map_err(|e: toml::de::Error| {
        let at = e.span().map_or(0, |s| s.start).min(text.len());
        let prefix = &text.as_bytes()[..at];
        #[allow(clippy::naive_bytecount)]
        let line = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
        let col = at
            - prefix
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |p| p + 1)
            + 1;
        ProfileError::new(format!("{file}: invalid TOML at line {line}, column {col}"))
    })?;
    let keys = profile_keys();
    let fields = |table: &toml::Value,
                  wh: &str,
                  family_ok: bool|
     -> Result<(Fields, Option<String>), ProfileError> {
        let toml::Value::Table(t) = table else {
            return Err(ProfileError::new(format!("{file}: [{wh}] must be a table")));
        };
        let mut out = BTreeMap::new();
        let mut family = None;
        for (k, v) in t {
            if k == "family" && family_ok {
                let toml::Value::String(s) = v else {
                    return Err(ProfileError::new(format!(
                        "{file}: [{wh}] family must be a string"
                    )));
                };
                family = Some(s.clone());
                continue;
            }
            if !keys.contains(k) {
                return Err(ProfileError::new(format!(
                    "{file}: unknown key '{k}' in [{wh}]"
                )));
            }
            let s = match v {
                toml::Value::String(s) => s.clone(),
                toml::Value::Integer(i) if k == "port" => i.to_string(),
                _ => {
                    return Err(ProfileError::new(format!(
                        "{file}: [{wh}] {k} must be a string{}",
                        if k == "port" { " or an integer" } else { "" }
                    )));
                }
            };
            out.insert(k.clone(), s);
        }
        Ok((out, family))
    };
    for top in data.keys() {
        if !matches!(top.as_str(), "defaults" | "families" | "profiles") {
            return Err(ProfileError::new(format!("{file}: unknown table [{top}]")));
        }
    }
    let empty = toml::Value::Table(toml::Table::new());
    let (defaults, _) = fields(data.get("defaults").unwrap_or(&empty), "defaults", false)?;
    let tables =
        |v: Option<&toml::Value>, wh: &str| -> Result<Vec<(String, toml::Value)>, ProfileError> {
            match v {
                None => Ok(vec![]),
                Some(toml::Value::Table(t)) if t.values().all(toml::Value::is_table) => {
                    Ok(t.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                }
                Some(_) => Err(ProfileError::new(format!(
                    "{file}: [{wh}] must hold tables ([{wh}.<name>])"
                ))),
            }
        };
    let mut families = BTreeMap::new();
    for (name, table) in tables(data.get("families"), "families")? {
        let ok = name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !ok {
            return Err(ProfileError::new(format!(
                "{file}: family '{name}' must be lowercase letters, digits, - or _"
            )));
        }
        let (f, _) = fields(&table, &format!("families.{name}"), false)?;
        families.insert(name, f);
    }
    let mut profiles = BTreeMap::new();
    for (name, table) in tables(data.get("profiles"), "profiles")? {
        if !is_profile_name(&name) {
            return Err(ProfileError::new(format!(
                "{file}: profile '{name}' must be [A-Z][A-Z0-9_]*"
            )));
        }
        let (own, family) = fields(&table, &format!("profiles.{name}"), true)?;
        let family = family.filter(|f| !f.is_empty());
        if let Some(fam) = family.as_deref()
            && !families.contains_key(fam)
        {
            return Err(ProfileError::new(format!(
                "{file}: [profiles.{name}] names family '{fam}', which isn't defined"
            )));
        }
        let mut merged = defaults.clone();
        if let Some(fam) = family.as_deref().and_then(|f| families.get(f)) {
            merged.extend(fam.clone());
        }
        merged.extend(own);
        profiles.insert(
            name.clone(),
            RawProfile {
                name,
                source: path.to_path_buf(),
                fields: merged,
                family,
            },
        );
    }
    Ok(profiles)
}
