//! Passwords: pass first, a plain-text file last (`shared/secrets.md`), and `secrets migrate`.
//! Ported from `dbrun.py` (`secret_source`, `read_pass`, `cmd_secrets`, `migrate_all`).
//!
//! Only a runner reads a value. Nothing here prints one: reports hold profile names and counts, and every
//! error goes through the pass store's redactor.
use super::{Ctx, Profile, ProfileError, SecretSource, Source, files, user_profiles};
use ai_gent_core::passstore::{self, Config, Secret};
use regex::Regex;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// `<prefix>/db/<name lower-cased, _ as ->`.
#[must_use]
pub fn pass_entry(cfg: &Config, name: &str) -> String {
    format!(
        "{}/db/{}",
        cfg.prefix(),
        name.to_lowercase().replace('_', "-")
    )
}

fn pass_has(cfg: &Config, entry: &str) -> bool {
    passstore::has(cfg, entry).unwrap_or(false)
}

fn check_entry(entry: &str) -> Result<&str, ProfileError> {
    passstore::check(entry).map_err(|e| ProfileError::new(e.to_string()))
}

/// Records where the password comes from; never reads it. Without a value, the family's entry
/// (`db/<family>`) comes before the profile's (`db/<profile>`).
pub fn secret_source(cfg: &Config, p: &mut Profile) {
    if let Some(pw) = p.password.as_ref() {
        if let Some(rest) = pw.expose().strip_prefix("pass:") {
            let entry = rest.trim().to_string();
            p.password = None;
            p.secret = SecretSource::Pass(entry);
            return;
        }
        p.secret = SecretSource::File;
        return;
    }
    let mut candidates = vec![];
    if let Some(f) = &p.family {
        candidates.push(pass_entry(cfg, f));
    }
    candidates.push(pass_entry(cfg, &p.name));
    for entry in candidates {
        if pass_has(cfg, &entry) {
            p.secret = SecretSource::Pass(entry);
            return;
        }
    }
    p.secret = SecretSource::None;
}

/// The entry's first line (pass's convention), registered for redaction and never printed.
///
/// # Errors
/// An invalid entry name, no pass, a locked or empty entry. The message holds no value.
pub fn read_password(cfg: &Config, entry: &str) -> Result<Secret, ProfileError> {
    passstore::read(cfg, entry)
        .map(|(s, _)| s)
        .map_err(|e| ProfileError::new(e.to_string()))
}

fn store_write(cfg: &Config, entry: &str, value: &str) -> Result<(), ProfileError> {
    passstore::write(cfg, entry, value).map_err(|e| ProfileError::new(e.to_string()))
}

fn file_of(p: &Profile) -> Option<PathBuf> {
    match &p.source {
        Source::File(f) => Some(f.clone()),
        Source::RunningContainer => None,
    }
}

fn is_toml(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "toml")
}

/// Rewrites `path` to `new_text` through a new mode-600 file next to it, refused when the file is a
/// symbolic link or changed (size or modification time) since `before` was taken. A failure leaves the file
/// as it was.
fn rewrite(path: &Path, new_text: &str, before: &std::fs::Metadata) -> Result<(), ProfileError> {
    let shown = files::file_name(path);
    let now = std::fs::symlink_metadata(path)
        .map_err(|e| ProfileError::new(format!("cannot read {shown}: {}", e.kind())))?;
    if now.file_type().is_symlink() {
        return Err(ProfileError::new(format!(
            "{shown} is a symbolic link; dbrun doesn't rewrite through one — edit the real file yourself"
        )));
    }
    if now.len() != before.len() || now.modified().ok() != before.modified().ok() {
        return Err(ProfileError::new(format!(
            "{shown} changed while migrating; nothing was rewritten"
        )));
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = dir.join(format!(".{shown}.{}.tmp", std::process::id()));
    super::write_new_private(&tmp, new_text.as_bytes())
        .map_err(|e| ProfileError::new(format!("cannot write a new {shown}: {}", e.kind())))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        ProfileError::new(format!("cannot replace {shown}: {}", e.kind()))
    })
}

fn read_with_meta(path: &Path) -> Result<(String, std::fs::Metadata), ProfileError> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| {
        ProfileError::new(format!(
            "cannot read {}: {}",
            files::file_name(path),
            e.kind()
        ))
    })?;
    let text = crate::fsutil::read_text_bounded(path, files::MAX_PROFILES_FILE_BYTES)
        .map_err(ProfileError::new)?;
    Ok((text, meta))
}

/// `NAME_PASSWORD=<value>` becomes `NAME_PASSWORD=pass:<entry>` for each name in `refs`.
fn replace_password_lines(text: &str, refs: &std::collections::BTreeMap<String, String>) -> String {
    let re = Regex::new(r"^(\s*(?:export\s+)?)([A-Z][A-Z0-9_]*)_PASSWORD(\s*=)").unwrap();
    let mut out = String::new();
    for ln in text.split_inclusive('\n') {
        match re.captures(ln) {
            Some(m) if refs.contains_key(&m[2]) => {
                let _ = writeln!(
                    out,
                    "{}{}_PASSWORD{}pass:{}",
                    &m[1], &m[2], &m[3], refs[&m[2]]
                );
            }
            _ => out.push_str(ln),
        }
    }
    out
}

/// What `secrets migrate --all` did: names and counts only.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MigrateReport {
    /// Profiles now referencing a new pass entry.
    pub referenced: Vec<String>,
    /// New pass entries written.
    pub written: Vec<String>,
    /// Entries that already existed (their profiles were left as they were).
    pub kept: Vec<String>,
    /// TOML profiles with a plain password (dbrun doesn't rewrite TOML).
    pub toml: Vec<String>,
    /// Groups of profiles on different accounts that share one password.
    pub reused: Vec<Vec<String>>,
}

impl MigrateReport {
    /// The text the command prints. No value.
    #[must_use]
    pub fn render(&self) -> String {
        let n = self.written.len();
        let mut s = format!(
            "{} profile(s) now reference {n} new pass entr{}{}",
            self.referenced.len(),
            if n == 1 { "y" } else { "ies" },
            if self.referenced.is_empty() {
                String::new()
            } else {
                format!(": {}", self.referenced.join(", "))
            }
        );
        if !self.kept.is_empty() {
            let _ = write!(
                s,
                "\nleft as they are, the entry already exists: {}",
                self.kept.join(", ")
            );
        }
        if !self.toml.is_empty() {
            let _ = write!(
                s,
                "\nin the TOML file, set password = \"pass:<entry>\" yourself (dbrun doesn't rewrite TOML): {}",
                self.toml.join(", ")
            );
        }
        for g in &self.reused {
            let _ = write!(
                s,
                "\nhygiene: different accounts share one password: {}; rotate one of them",
                g.join(", ")
            );
        }
        s
    }

    /// The result line of the command log.
    #[must_use]
    pub fn log_line(&self) -> String {
        format!(
            "{} profiles, {} entries",
            self.referenced.len(),
            self.written.len()
        )
    }
}

/// `secrets migrate --all`: every plain password of the `.env` file into pass, replaced by a reference.
/// One entry per shared account (profiles with the same user and password go to `db/<user>`); otherwise one
/// per profile.
///
/// # Errors
/// An invalid entry name, a failed pass write, a profiles file that is a symbolic link or changed.
pub fn migrate_all(ctx: &Ctx) -> Result<MigrateReport, ProfileError> {
    let cfg = ctx.pass_config();
    let loaded = user_profiles(ctx)?;
    let mut report = MigrateReport::default();
    let mut plain: Vec<&Profile> = vec![];
    for p in loaded
        .profiles
        .values()
        .filter(|p| p.secret == SecretSource::File)
    {
        if file_of(p).is_some_and(|f| is_toml(&f)) {
            report.toml.push(p.name.clone());
        } else {
            plain.push(p);
        }
    }
    for p in &plain {
        if let Some(pw) = &p.password {
            pw.register();
        }
    }
    // (user, password) -> names, in the order of the sorted names.
    let mut groups: Vec<((String, String), Vec<&str>)> = vec![];
    for p in &plain {
        let key = (
            p.user.clone().unwrap_or_default(),
            p.password
                .as_ref()
                .map_or(String::new(), |s| s.expose().to_string()),
        );
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, names)) => names.push(&p.name),
            None => groups.push((key, vec![&p.name])),
        }
    }
    let users: Vec<&str> = groups.iter().map(|((u, _), _)| u.as_str()).collect();
    let mut refs: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for ((user, pw), names) in &groups {
        let shared = names.len() > 1
            && !user.is_empty()
            && users.iter().filter(|u| **u == user.as_str()).count() == 1;
        let entry = pass_entry(&cfg, if shared { user } else { names[0] });
        check_entry(&entry)?;
        if pass_has(&cfg, &entry) {
            report.kept.push(entry);
            continue;
        }
        store_write(&cfg, &entry, pw)?;
        report.written.push(entry.clone());
        for n in names {
            refs.insert((*n).to_string(), entry.clone());
        }
    }
    let mut by_path: std::collections::BTreeMap<
        PathBuf,
        std::collections::BTreeMap<String, String>,
    > = std::collections::BTreeMap::new();
    for p in &plain {
        if let (Some(entry), Some(f)) = (refs.get(&p.name), file_of(p)) {
            by_path
                .entry(f)
                .or_default()
                .insert(p.name.clone(), entry.clone());
        }
    }
    for (path, part) in by_path {
        let (text, meta) = read_with_meta(&path)?;
        rewrite(&path, &replace_password_lines(&text, &part), &meta)?;
    }
    let mut accounts: Vec<(String, Vec<(String, String)>)> = vec![];
    for ((user, pw), names) in &groups {
        let list: Vec<(String, String)> = names
            .iter()
            .map(|n| ((*n).to_string(), user.clone()))
            .collect();
        match accounts.iter_mut().find(|(k, _)| k == pw) {
            Some((_, v)) => v.extend(list),
            None => accounts.push((pw.clone(), list)),
        }
    }
    report.reused = accounts
        .into_iter()
        .map(|(_, v)| v)
        .filter(|v| {
            v.iter()
                .map(|(_, u)| u)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1
        })
        .map(|v| v.into_iter().map(|(n, _)| n).collect())
        .collect();
    report.referenced = refs.keys().cloned().collect();
    Ok(report)
}

/// `secrets migrate <profile>`: one plain password into pass; its line leaves the `.env` file. Returns the
/// line the command prints and, when something moved, the result for the log.
///
/// # Errors
/// An unknown profile, a TOML profile (not rewritten), an entry that already exists, an invalid entry name,
/// a failed write, a profiles file that is a symbolic link or changed.
pub fn migrate_one(ctx: &Ctx, name: &str) -> Result<String, ProfileError> {
    let cfg = ctx.pass_config();
    let loaded = user_profiles(ctx)?;
    let Some(p) = loaded.profiles.get(name) else {
        return Err(ProfileError::new(format!(
            "no profile '{name}' in the profiles file"
        )));
    };
    if let Some(pw) = &p.password {
        pw.register();
    }
    if p.secret != SecretSource::File {
        let src = match &p.secret {
            SecretSource::Pass(e) => e.clone(),
            other => other.word().to_string(),
        };
        return Ok(format!(
            "{}: nothing to migrate (password source: {src})",
            p.name
        ));
    }
    let path = file_of(p).unwrap_or_default();
    if is_toml(&path) {
        return Err(ProfileError::new(format!(
            "{} is in connections.toml, which dbrun doesn't rewrite: put the value in pass yourself (pass insert {}) and set password = \"pass:<entry>\"",
            p.name,
            pass_entry(&cfg, p.family.as_deref().unwrap_or(&p.name))
        )));
    }
    let entry = pass_entry(&cfg, &p.name);
    check_entry(&entry)?;
    if pass_has(&cfg, &entry) {
        return Err(ProfileError::new(format!(
            "{entry} already exists in pass; remove the {}_PASSWORD line yourself once it's right",
            p.name
        )));
    }
    let (text, meta) = read_with_meta(&path)?;
    let value = p
        .password
        .as_ref()
        .map_or(String::new(), |s| s.expose().to_string());
    // The file may have changed since the profiles were loaded: it must still hold this very value.
    let current = files::parse_env_text(&text, &files::file_name(&path))?;
    if current.get(&format!("{}_PASSWORD", p.name)) != Some(&value) {
        return Err(ProfileError::new(format!(
            "{} changed while migrating; nothing was moved",
            files::file_name(&path)
        )));
    }
    store_write(&cfg, &entry, &value)?;
    let key = Regex::new(&format!(
        r"^\s*(export\s+)?{}_PASSWORD\s*=",
        regex::escape(&p.name)
    ))
    .unwrap();
    let kept: String = text
        .split_inclusive('\n')
        .filter(|ln| !key.is_match(ln))
        .collect();
    rewrite(&path, &kept, &meta)?;
    Ok(format!(
        "{}: password moved to pass ({entry}); its line is gone from the profiles file",
        p.name
    ))
}
