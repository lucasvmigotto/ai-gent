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
///
/// # Errors
/// An explicit `pass:<entry>` whose entry is not a valid pass entry name (checked here, at load, so a
/// reference can never carry a line break or other text into a listing, an export or a command line).
pub fn secret_source(cfg: &Config, p: &mut Profile) -> Result<(), ProfileError> {
    if let Some(pw) = p.password.as_ref() {
        if let Some(rest) = pw.expose().strip_prefix("pass:") {
            let entry = rest.trim().to_string();
            if passstore::check(&entry).is_err() {
                return Err(ProfileError::new(format!(
                    "profile {}: the pass: reference isn't a pass entry name (letters, digits, . _ @ + - and /; no leading - or /, no ..)",
                    p.name
                )));
            }
            p.password = None;
            p.secret = SecretSource::Pass(entry);
            return Ok(());
        }
        p.secret = SecretSource::File;
        return Ok(());
    }
    let mut candidates = vec![];
    if let Some(f) = &p.family {
        candidates.push(pass_entry(cfg, f));
    }
    candidates.push(pass_entry(cfg, &p.name));
    for entry in candidates {
        if pass_has(cfg, &entry) {
            p.secret = SecretSource::Pass(entry);
            return Ok(());
        }
    }
    p.secret = SecretSource::None;
    Ok(())
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

/// Reads the file for a rewrite: refuses a symbolic link, returns the text and the metadata snapshot
/// [`rewrite`] compares against.
fn read_with_meta(path: &Path) -> Result<(String, std::fs::Metadata), ProfileError> {
    let shown = files::file_name(path);
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| ProfileError::new(format!("cannot read {shown}: {}", e.kind())))?;
    if meta.file_type().is_symlink() {
        return Err(ProfileError::new(format!(
            "{shown} is a symbolic link; dbrun doesn't rewrite through one — edit the real file yourself"
        )));
    }
    let text = crate::fsutil::read_text_bounded(path, files::MAX_PROFILES_FILE_BYTES)
        .map_err(ProfileError::new)?;
    Ok((text, meta))
}

/// What to do with the `NAME_PASSWORD` line of a profile: `Some(entry)` replaces its value with
/// `pass:<entry>`, `None` removes the line.
type Targets = std::collections::BTreeMap<String, Option<String>>;

fn password_line(content: &str) -> Option<regex::Captures<'_>> {
    Regex::new(r"^(\s*(?:export\s+)?)([A-Z][A-Z0-9_]*)_PASSWORD(\s*=)")
        .unwrap()
        .captures(content)
}

/// Applies `targets` line by line, with the same line splitter as the parser. Each replaced line keeps
/// its own terminator; every other line is copied byte for byte.
fn rewrite_lines(text: &str, targets: &Targets) -> String {
    let mut out = String::new();
    for (content, term) in files::split_lines(text) {
        match password_line(content).and_then(|m| targets.get(&m[2]).map(|t| (m, t))) {
            Some((_, None)) => {}
            Some((m, Some(entry))) => {
                let _ = write!(
                    out,
                    "{}{}_PASSWORD{}pass:{entry}{term}",
                    &m[1], &m[2], &m[3]
                );
            }
            None => {
                out.push_str(content);
                out.push_str(term);
            }
        }
    }
    out
}

/// Fails closed: the rewritten text must hold no old plaintext for a target, the new reference (or no line)
/// for each target, and every other line unchanged, in order, and still parse. Anything else refuses
/// the whole migration before a pass entry or a file is touched.
fn verify_rewrite(old: &str, new: &str, targets: &Targets, file: &str) -> Result<(), ProfileError> {
    let bad = |why: &str| {
        ProfileError::new(format!(
            "{file}: the rewrite would not be safe ({why}); nothing was changed — edit the file yourself"
        ))
    };
    let old_map = files::parse_env_text(old, file)?;
    let new_map = files::parse_env_text(new, file).map_err(|_| bad("the result does not parse"))?;
    for (name, want) in targets {
        let key = format!("{name}_PASSWORD");
        let got = new_map.get(&key);
        match want {
            None if got.is_some() => return Err(bad("the old password line is still there")),
            Some(e) if got.map(String::as_str) != Some(format!("pass:{e}").as_str()) => {
                return Err(bad("the old password is still there"));
            }
            _ => {}
        }
    }
    let others = |m: &std::collections::BTreeMap<String, String>| {
        m.iter()
            .filter(|(k, _)| !targets.keys().any(|n| **k == format!("{n}_PASSWORD")))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>()
    };
    if others(&old_map) != others(&new_map) {
        return Err(bad("another value would change"));
    }
    let keep = |t: &str| -> Vec<String> {
        files::split_lines(t)
            .into_iter()
            .filter(|(c, _)| !password_line(c).is_some_and(|m| targets.contains_key(&m[2])))
            .map(|(c, e)| format!("{c}{e}"))
            .collect()
    };
    if keep(old) != keep(new) {
        return Err(bad("another line would change"));
    }
    Ok(())
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
    // Phase 1, no writes: which entry each group goes to, and which already exist.
    let mut refs: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut to_write: Vec<(String, &str)> = vec![];
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
        to_write.push((entry.clone(), pw));
        for n in names {
            refs.insert((*n).to_string(), entry.clone());
        }
    }
    // Phase 2, no writes: snapshot every file (a symbolic link is refused here), build and verify the
    // rewritten text.
    let mut by_path: std::collections::BTreeMap<PathBuf, Targets> =
        std::collections::BTreeMap::new();
    for p in &plain {
        if let (Some(entry), Some(f)) = (refs.get(&p.name), file_of(p)) {
            by_path
                .entry(f)
                .or_default()
                .insert(p.name.clone(), Some(entry.clone()));
        }
    }
    let mut plans = vec![];
    for (path, targets) in by_path {
        let (text, meta) = read_with_meta(&path)?;
        let new = rewrite_lines(&text, &targets);
        verify_rewrite(&text, &new, &targets, &files::file_name(&path))?;
        plans.push((path, new, meta));
    }
    // Phase 3: the pass entries, then the files (each re-checked against its snapshot).
    for (entry, pw) in &to_write {
        store_write(&cfg, entry, pw)?;
        report.written.push(entry.clone());
    }
    for (path, new, meta) in plans {
        rewrite(&path, &new, &meta)?;
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
    let targets: Targets = std::iter::once((p.name.clone(), None)).collect();
    let kept = rewrite_lines(&text, &targets);
    // Verified before anything is written: a rewrite that would leave the password or touch another line
    // refuses the migration.
    verify_rewrite(&text, &kept, &targets, &files::file_name(&path))?;
    store_write(&cfg, &entry, &value)?;
    rewrite(&path, &kept, &meta)?;
    Ok(format!(
        "{}: password moved to pass ({entry}); its line is gone from the profiles file",
        p.name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(name: &str, entry: Option<&str>) -> Targets {
        std::iter::once((name.to_string(), entry.map(str::to_string))).collect()
    }

    #[test]
    fn verify_rewrite_refuses_a_result_that_keeps_the_password_or_changes_another_line() {
        let old = "A_ENGINE=x\nA_PASSWORD=s3cr3t\nB_HOST=keep\n";
        let t = targets("A", None);
        let good = rewrite_lines(old, &t);
        assert!(verify_rewrite(old, &good, &t, "f.env").is_ok());
        // the password line is still there
        let e = verify_rewrite(old, old, &t, "f.env").unwrap_err();
        assert!(e.to_string().contains("nothing was changed"), "{e}");
        assert!(!e.to_string().contains("s3cr3t"));
        // another line dropped, another value changed, a line added
        for bad in [
            "A_ENGINE=x\n",
            "A_ENGINE=y\nB_HOST=keep\n",
            "A_ENGINE=x\nB_HOST=keep\nC_HOST=new\n",
            "A_ENGINE=x\n\nB_HOST=keep\n",
        ] {
            assert!(verify_rewrite(old, bad, &t, "f.env").is_err(), "{bad:?}");
        }
        // replacement: the reference must be there
        let t = targets("A", Some("ai-gent/db/a"));
        let new = rewrite_lines(old, &t);
        assert!(verify_rewrite(old, &new, &t, "f.env").is_ok());
        assert!(verify_rewrite(old, old, &t, "f.env").is_err());
    }
}
