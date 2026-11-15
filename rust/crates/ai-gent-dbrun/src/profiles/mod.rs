//! Profiles, classification and secrets of the runner (spec 043 T010), ported from `dbrun.py`.
//!
//! - [`files`]: the `.env` and TOML profiles files.
//! - [`container`]: the container engine, local discovery, the local proof and the published-port
//!   detection (FR-020).
//! - [`secrets`]: pass-first password lookup through `ai-gent-core`'s pass store, `secrets migrate`.
//!
//! A profile's password comes from pass first (`shared/secrets.md`): an explicit `pass:<entry>`, else the
//! family's entry and then the profile's own (`ai-gent/db/<name>`) when it exists; a plain value in the
//! profiles file is the last resort. Entries are decrypted only when a runner connects ([`get_profile`]
//! with `secret = true`), never to list or classify.
use ai_gent_core::env::Env;
use ai_gent_core::passstore::Secret;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod container;
pub mod files;
pub mod secrets;

pub use container::{Classification, ContainerEngine, Route, Via};
pub use files::RawProfile;

/// Exit code of an operational failure.
pub const EXIT_ERROR: i32 = 1;
/// Exit code of a connection failure.
pub const EXIT_CONNECT: i32 = 4;

/// The engines a profile can name.
pub const ENGINES: [&str; 6] = [
    "postgresql",
    "mysql",
    "mariadb",
    "sqlserver",
    "oracle",
    "sqlite",
];

/// The engine's default port.
#[must_use]
pub fn default_port(engine: &str) -> Option<u16> {
    match engine {
        "postgresql" => Some(5432),
        "mysql" | "mariadb" => Some(3306),
        "sqlserver" => Some(1433),
        "oracle" => Some(1521),
        _ => None,
    }
}

/// A refused or failed profile operation. The message never holds a secret: it goes through the
/// process-wide redactor of the pass store (every value read in this process) before it is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileError {
    /// Exit code the CLI uses.
    pub code: i32,
    message: String,
}

impl ProfileError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self::with_code(message, EXIT_ERROR)
    }

    pub(crate) fn with_code(message: impl Into<String>, code: i32) -> Self {
        Self {
            code,
            message: ai_gent_core::passstore::redact(&message.into()),
        }
    }
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ProfileError {}

/// The environment class of a profile. `Local` in a file is a claim; [`container::classify`] proves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Production (also what an unclassified or unknown class means).
    Production,
    /// Staging.
    Staging,
    /// Development.
    Development,
    /// Local.
    Local,
}

impl Class {
    /// Parses a declared class; anything unknown is production.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "staging" => Self::Staging,
            "development" => Self::Development,
            "local" => Self::Local,
            _ => Self::Production,
        }
    }

    /// The word used in files and output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Staging => "staging",
            Self::Development => "development",
            Self::Local => "local",
        }
    }

    /// The TLS policy's view of the class.
    #[must_use]
    pub fn kind(self) -> crate::core::tls::ClassKind {
        use crate::core::tls::ClassKind;
        match self {
            Self::Production => ClassKind::Production,
            Self::Staging => ClassKind::Staging,
            Self::Development => ClassKind::Development,
            Self::Local => ClassKind::Local,
        }
    }
}

/// Where a profile comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A profiles file.
    File(PathBuf),
    /// A running container of this repository's compose projects.
    RunningContainer,
}

/// Where the password comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretSource {
    /// A pass entry (read only when connecting).
    Pass(String),
    /// A plain value in the profiles file (the last resort).
    File,
    /// The container's own environment (local containers).
    Container,
    /// None known.
    None,
}

impl SecretSource {
    /// `pass`, `file`, `container` or `none`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Self::Pass(_) => "pass",
            Self::File => "file",
            Self::Container => "container",
            Self::None => "none",
        }
    }
}

/// One database profile.
#[derive(Clone)]
pub struct Profile {
    /// The name (`NAME` of a file, `local:<service>` of a container).
    pub name: String,
    /// Where it comes from.
    pub source: Source,
    /// Lower-case engine word.
    pub engine: String,
    /// The declared class (unclassified means production).
    pub class: Class,
    /// TOML family.
    pub family: Option<String>,
    /// Server host.
    pub host: Option<String>,
    /// Server port.
    pub port: Option<String>,
    /// Database.
    pub database: Option<String>,
    /// Oracle service.
    pub service: Option<String>,
    /// Oracle SID.
    pub sid: Option<String>,
    /// User.
    pub user: Option<String>,
    /// SQLite file.
    pub path: Option<String>,
    /// Container name.
    pub container: Option<String>,
    /// Compose project.
    pub project: Option<String>,
    /// The password: a plain value from the file, or the container's, or the pass entry's after
    /// [`get_profile`] read it. Never printed.
    pub password: Option<Secret>,
    /// Where the password comes from.
    pub secret: SecretSource,
    /// `tls_mode` as written.
    pub tls_mode: Option<String>,
    /// `tls_ca_file` as written.
    pub tls_ca_file: Option<String>,
    /// `tls_server_name` as written.
    pub tls_server_name: Option<String>,
    /// Published-port detection of a local container profile (FR-020).
    pub route: Option<Route>,
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Not host, user or password: they are what the redactor hides.
        f.debug_struct("Profile")
            .field("name", &self.name)
            .field("engine", &self.engine)
            .field("class", &self.class)
            .finish_non_exhaustive()
    }
}

impl Profile {
    pub(crate) fn blank(name: &str, source: Source) -> Self {
        Self {
            name: name.to_string(),
            source,
            engine: String::new(),
            class: Class::Production,
            family: None,
            host: None,
            port: None,
            database: None,
            service: None,
            sid: None,
            user: None,
            path: None,
            container: None,
            project: None,
            password: None,
            secret: SecretSource::None,
            tls_mode: None,
            tls_ca_file: None,
            tls_server_name: None,
            route: None,
        }
    }

    pub(crate) fn source_label(&self) -> String {
        match &self.source {
            Source::File(p) => p.display().to_string(),
            Source::RunningContainer => "running container".to_string(),
        }
    }

    fn from_raw(raw: RawProfile) -> Self {
        let mut p = Self::blank(&raw.name, Source::File(raw.source));
        let g = |k: &str| raw.fields.get(k).cloned();
        p.engine = g("engine").unwrap_or_default().to_lowercase();
        p.class = Class::parse(&g("class").unwrap_or_default());
        p.family = raw.family;
        p.host = g("host");
        p.port = g("port");
        p.database = g("database");
        p.service = g("service");
        p.sid = g("sid");
        p.user = g("user");
        p.path = g("path");
        p.container = g("container");
        p.project = g("project");
        p.tls_mode = g("tls_mode");
        p.tls_ca_file = g("tls_ca_file");
        p.tls_server_name = g("tls_server_name");
        p.password = g("password").filter(|v| !v.is_empty()).map(Secret::new);
        p
    }

    /// Host, user and password as `(value, label)` pairs for the shared redactor: the value is
    /// skipped when shorter than two characters or loopback (`dbrun.py` `remember_secrets`).
    #[must_use]
    pub fn redaction_pairs(&self) -> Vec<(String, &'static str)> {
        let mut v = vec![];
        let items: [(Option<&str>, &'static str); 3] = [
            (self.password.as_ref().map(Secret::expose), "<password>"),
            (self.host.as_deref(), "<host>"),
            (self.user.as_deref(), "<user>"),
        ];
        for (value, label) in items {
            let value = value.unwrap_or("").trim();
            if value.chars().count() >= 2 && value != "127.0.0.1" && value != "localhost" {
                v.push((value.to_string(), label));
            }
        }
        v
    }

    /// What the profile still needs before it can connect, by key name.
    #[must_use]
    pub fn missing_keys(&self) -> Vec<&'static str> {
        if self.source == Source::RunningContainer {
            return vec![];
        }
        let none = |o: &Option<String>| o.as_deref().is_none_or(str::is_empty);
        if self.engine == "sqlite" {
            return if none(&self.path) {
                vec!["PATH"]
            } else {
                vec![]
            };
        }
        let mut need = vec![];
        if none(&self.host) && none(&self.container) {
            need.push("HOST");
        }
        if none(&self.user) {
            need.push("USER");
        }
        if self.engine == "oracle" && none(&self.service) && none(&self.database) && none(&self.sid)
        {
            need.push("SERVICE|SID");
        }
        need
    }

    /// The profile's TLS settings, validated (`tls_mode`, `tls_ca_file`, `tls_server_name`).
    ///
    /// # Errors
    /// [`crate::core::tls::TlsError`] naming the field.
    pub fn tls_settings(
        &self,
        home: Option<&Path>,
    ) -> Result<crate::core::tls::TlsSettings, crate::core::tls::TlsError> {
        crate::core::tls::TlsSettings::from_fields(
            self.tls_mode.as_deref(),
            self.tls_ca_file.as_deref(),
            self.tls_server_name.as_deref(),
            home,
        )
    }

    /// The profile's TLS settings after the policy check, for the classification the runner made. The host
    /// the policy sees is the loopback address of a published port when the route is one, else the
    /// profile's host.
    ///
    /// # Errors
    /// [`crate::core::tls::TlsError`]: a bad field, or `unverified` where it is not allowed
    /// (naming `tls_mode`).
    pub fn tls_authorized(
        &self,
        cls: &Classification,
        home: Option<&Path>,
    ) -> Result<crate::core::tls::Authorized, crate::core::tls::TlsError> {
        let settings = self.tls_settings(home)?;
        let host = match &cls.route {
            Some(Route::Published { host, .. }) => host.clone(),
            _ => self.host.clone().unwrap_or_default(),
        };
        settings.authorize(&crate::core::tls::TlsContext {
            declared: self.class.kind(),
            proven: cls.class.kind(),
            host: &host,
        })
    }
}

/// Characters a connect descriptor reads as structure.
const DESCRIPTOR_CHARS: [char; 7] = ['(', ')', '=', '"', '\'', '\\', ' '];

/// Oracle Easy Connect for a service name (`SERVICE`, else `DATABASE`); a connect descriptor for a SID.
/// Refuses a field that holds descriptor characters or whitespace (FR-009, as defence in depth: the shared
/// validator of `core::fields` is the contract).
///
/// # Errors
/// The field that holds a refused character.
pub fn oracle_dsn(p: &Profile) -> Result<String, ProfileError> {
    let bad = |v: &str| {
        v.chars()
            .any(|c| DESCRIPTOR_CHARS.contains(&c) || c.is_whitespace() || c.is_control())
    };
    let host = p.host.clone().unwrap_or_default();
    let port = p
        .port
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "1521".into());
    let service = p
        .service
        .clone()
        .filter(|s| !s.is_empty())
        .or_else(|| p.database.clone().filter(|s| !s.is_empty()));
    for (name, v) in [
        ("host", &host),
        ("port", &port),
        ("service", &service.clone().unwrap_or_default()),
        ("sid", &p.sid.clone().unwrap_or_default()),
    ] {
        if bad(v) {
            return Err(ProfileError::new(format!(
                "profile {}: {name} holds a character that is not allowed in a connect string",
                p.name
            )));
        }
    }
    Ok(match service {
        Some(s) => format!("{host}:{port}/{s}"),
        None => format!(
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST={host})(PORT={port}))(CONNECT_DATA=(SID={})))",
            p.sid.clone().unwrap_or_default()
        ),
    })
}

/// Whether `path` resolves to somewhere inside `root`. A path that cannot be resolved is not inside.
#[must_use]
pub fn inside(path: &Path, root: &Path) -> bool {
    match (path.canonicalize(), root.canonicalize()) {
        (Ok(p), Ok(r)) => p.starts_with(r),
        _ => false,
    }
}

/// `~` and `~/rest` expanded with the home directory of `env`.
#[must_use]
pub fn expand_home(raw: &str, env: &Env) -> PathBuf {
    if (raw == "~" || raw.starts_with("~/") || raw.starts_with("~\\"))
        && let Some(h) = env.home_dir()
    {
        return h.join(raw[1..].trim_start_matches(['/', '\\']));
    }
    PathBuf::from(raw)
}

/// Everything the profile code needs from the machine, explicit so tests build their own.
#[derive(Clone)]
pub struct Ctx {
    /// The environment (profiles locations, pass, engine selection).
    pub env: Env,
    /// The repository root a local database must belong to.
    pub root: PathBuf,
    /// The container engine, if any.
    pub engine: Option<Arc<dyn ContainerEngine>>,
}

impl Ctx {
    /// A context from its parts.
    #[must_use]
    pub fn new(env: Env, root: PathBuf, engine: Option<Arc<dyn ContainerEngine>>) -> Self {
        Self { env, root, engine }
    }

    /// The process environment, the repository of the current directory, the engine found on `PATH`.
    #[must_use]
    pub fn from_process() -> Self {
        let env = Env::real();
        let root = repo_root(&env);
        let engine = container::detect_engine(&env);
        Self { env, root, engine }
    }

    /// `AI_GENT_DB_CONFIG`, else `$XDG_CONFIG_HOME/ai-gent/db`, else `~/.config/ai-gent/db`.
    #[must_use]
    pub fn config_dir(&self) -> PathBuf {
        if let Some(d) = self.env.path("AI_GENT_DB_CONFIG") {
            return d;
        }
        let base = self.env.path("XDG_CONFIG_HOME").unwrap_or_else(|| {
            self.env
                .home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
        });
        base.join("ai-gent").join("db")
    }

    pub(crate) fn pass_config(&self) -> ai_gent_core::passstore::Config {
        ai_gent_core::passstore::Config::new(self.env.clone())
    }

    fn engine_ref(&self) -> Option<&dyn ContainerEngine> {
        self.engine.as_deref()
    }
}

/// `git rev-parse --show-toplevel` (10 s), else the current directory.
#[must_use]
pub fn repo_root(env: &Env) -> PathBuf {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["rev-parse", "--show-toplevel"]);
    if let Ok(o) = ai_gent_core::exec::run(cmd, env, None, std::time::Duration::from_secs(10))
        && o.code == 0
    {
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !s.is_empty() {
            return PathBuf::from(&s)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(s));
        }
    }
    std::env::current_dir()
        .ok()
        .and_then(|d| d.canonicalize().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// What loading the profiles files found.
#[derive(Debug, Default)]
pub struct Loaded {
    /// The profiles by name.
    pub profiles: BTreeMap<String, Profile>,
    /// Warnings for standard error (a file readable by others).
    pub warnings: Vec<String>,
}

/// The profiles files to read: `AI_GENT_DB_CONNECTIONS` (one file, parsed by its extension), else the
/// TOML file then the `.env` file in the config directory.
///
/// # Errors
/// `AI_GENT_DB_CONNECTIONS` naming something that is not a `.toml` or `.env` file.
pub fn profile_files(ctx: &Ctx) -> Result<Vec<PathBuf>, ProfileError> {
    if let Some(e) = ctx
        .env
        .text("AI_GENT_DB_CONNECTIONS")
        .filter(|v| !v.is_empty())
    {
        let path = expand_home(&e, &ctx.env);
        let ext = path.extension().and_then(|x| x.to_str());
        let ok = ext.is_some_and(|x| x == "toml" || x == "env")
            && path.file_name().is_some_and(|n| n != ".env");
        if !ok {
            return Err(ProfileError::new(
                "AI_GENT_DB_CONNECTIONS must name a .toml or .env file",
            ));
        }
        return Ok(if path.exists() { vec![path] } else { vec![] });
    }
    let d = ctx.config_dir();
    Ok([
        d.join("connections.toml"),
        d.join(["connections", ".env"].concat()),
    ]
    .into_iter()
    .filter(|f| f.exists())
    .collect())
}

/// The profiles of the files, with their password sources recorded (nothing decrypted).
///
/// # Errors
/// A malformed file, or a profile defined in two files.
pub fn user_profiles(ctx: &Ctx) -> Result<Loaded, ProfileError> {
    let mut out = Loaded::default();
    for path in profile_files(ctx)? {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(m) = std::fs::metadata(&path) {
                let mode = m.permissions().mode() & 0o777;
                if mode & 0o077 != 0 {
                    out.warnings.push(format!(
                        "warn: {} is mode {mode:o}; it holds credentials — chmod 600 it",
                        files::file_name(&path)
                    ));
                }
            }
        }
        let found = if path.extension().is_some_and(|e| e == "toml") {
            let text = crate::fsutil::read_text_bounded(&path, files::MAX_PROFILES_FILE_BYTES)
                .map_err(ProfileError::new)?;
            files::toml_profiles(&text, &path)?
        } else {
            let values = files::read_env_file(&path)?;
            files::env_profiles(&values, &path)
        };
        for (name, raw) in found {
            if let Some(prev) = out.profiles.get(&name) {
                let prev_name = match &prev.source {
                    Source::File(p) => files::file_name(p),
                    Source::RunningContainer => "running container".into(),
                };
                return Err(ProfileError::new(format!(
                    "profile {name} is defined in both {prev_name} and {}",
                    files::file_name(&path)
                )));
            }
            out.profiles.insert(name, Profile::from_raw(raw));
        }
    }
    let cfg = ctx.pass_config();
    for p in out.profiles.values_mut() {
        secrets::secret_source(&cfg, p)?;
    }
    Ok(out)
}

/// The local containers' profiles plus the files', refusing a name defined twice.
///
/// # Errors
/// As [`user_profiles`], or a file profile that has the name of a container profile.
pub fn all_profiles(ctx: &Ctx) -> Result<Loaded, ProfileError> {
    let mut loaded = Loaded {
        profiles: container::local_profiles(ctx.engine_ref(), &ctx.env, &ctx.root),
        warnings: vec![],
    };
    let user = user_profiles(ctx)?;
    loaded.warnings = user.warnings;
    for (name, p) in user.profiles {
        if loaded.profiles.contains_key(&name) {
            return Err(ProfileError::new(format!(
                "profile {name} is defined twice; rename one"
            )));
        }
        loaded.profiles.insert(name, p);
    }
    Ok(loaded)
}

/// The named profile; with `secret`, its keys are checked and its password read (from pass if it lives
/// there).
///
/// # Errors
/// An unknown profile, missing keys, an unreadable or invalid pass entry, an engine that is not supported.
pub fn get_profile(ctx: &Ctx, name: &str, secret: bool) -> Result<Profile, ProfileError> {
    let loaded = all_profiles(ctx)?;
    let Some(found) = loaded.profiles.get(name) else {
        let known = loaded
            .profiles
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let known = if known.is_empty() {
            "none".to_string()
        } else {
            known
        };
        return Err(ProfileError::new(format!(
            "no profile '{name}' (known: {known}); see `dbrun profiles`"
        )));
    };
    let mut p = found.clone();
    if secret {
        let missing = p.missing_keys();
        if !missing.is_empty() {
            return Err(ProfileError::new(format!(
                "profile {name} is missing {}; see connections.md",
                missing.join(", ")
            )));
        }
        if let SecretSource::Pass(entry) = &p.secret {
            p.password = Some(secrets::read_password(&ctx.pass_config(), entry)?);
        }
    }
    if let Some(pw) = &p.password {
        pw.register();
    }
    // Host and user are NOT put into the process-wide substring redactor: it would mangle ordinary text
    // (a user `postgres` turns `postgresql` into `<token>ql`). They are redacted as whole words with
    // labels through `Profile::redaction_pairs` / the shared `Redactor` (FR-008), as `dbrun.py` did.
    if !ENGINES.contains(&p.engine.as_str()) {
        return Err(ProfileError::new(format!(
            "profile {name}: engine '{}' isn't one of {}",
            p.engine,
            ENGINES.join(", ")
        )));
    }
    Ok(p)
}

/// Classifies a profile with the context's engine and repository.
///
/// # Errors
/// As [`container::classify`].
pub fn classify(ctx: &Ctx, p: &Profile) -> Result<Classification, ProfileError> {
    container::classify(p, ctx.engine_ref(), &ctx.env, &ctx.root)
}

/// One row of `profiles` output: no host, user or password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Profile name.
    pub name: String,
    /// Engine.
    pub engine: String,
    /// Declared class.
    pub class: String,
    /// Source file or `running container`.
    pub source: String,
    /// Password source: the pass entry name, `file`, `container` or `none`.
    pub password: String,
}

/// The listing rows, by name.
#[must_use]
pub fn rows(profiles: &BTreeMap<String, Profile>) -> Vec<Row> {
    profiles
        .values()
        .map(|p| Row {
            name: p.name.clone(),
            engine: if p.engine.is_empty() {
                "?".into()
            } else {
                p.engine.clone()
            },
            class: p.class.as_str().into(),
            source: p.source_label(),
            password: match &p.secret {
                SecretSource::Pass(e) => e.clone(),
                other => other.word().into(),
            },
        })
        .collect()
}

/// `profiles --json`.
#[must_use]
pub fn rows_json(rows: &[Row]) -> String {
    let v: Vec<_> = rows
        .iter()
        .map(|r| json!({"name": r.name, "engine": r.engine, "class": r.class, "source": r.source, "password": r.password}))
        .collect();
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

/// The `.env` text of `profiles --export-env`: passwords as `pass:` references only. Returns the text
/// and the names whose plain password was left out.
///
/// # Errors
/// A value that cannot be written back as one `.env` line (a newline or NUL, or both quote characters
/// together with whitespace).
pub fn export_env_text(
    profiles: &BTreeMap<String, Profile>,
) -> Result<(String, Vec<String>), ProfileError> {
    let mut lines = vec![
        "# Exported by dbrun profiles --export-env: passwords are pass: references only"
            .to_string(),
    ];
    let mut skipped = vec![];
    for (name, p) in profiles {
        lines.push(String::new());
        let fields: [(&str, &Option<String>); 14] = [
            ("engine", &Some(p.engine.clone())),
            ("class", &Some(p.class.as_str().to_string())),
            ("host", &p.host),
            ("port", &p.port),
            ("database", &p.database),
            ("service", &p.service),
            ("sid", &p.sid),
            ("user", &p.user),
            ("path", &p.path),
            ("container", &p.container),
            ("project", &p.project),
            ("tls_mode", &p.tls_mode),
            ("tls_ca_file", &p.tls_ca_file),
            ("tls_server_name", &p.tls_server_name),
        ];
        for (key, value) in fields {
            let Some(v) = value.as_deref().filter(|v| !v.is_empty()) else {
                continue;
            };
            lines.push(format!(
                "{name}_{}={}",
                key.to_uppercase(),
                quote_env_value(name, key, v)?
            ));
        }
        match &p.secret {
            SecretSource::Pass(e) => {
                if passstore_check(e).is_err() || refused_in_env_value(e) {
                    return Err(ProfileError::new(format!(
                        "profile {name}: the pass entry isn't a valid entry name and can't be exported"
                    )));
                }
                lines.push(format!("{name}_PASSWORD=pass:{e}"));
            }
            SecretSource::File => {
                skipped.push(name.clone());
                lines.push(format!(
                    "# {name}_PASSWORD: a plain value isn't exported — move it into pass first"
                ));
            }
            _ => {}
        }
    }
    Ok((lines.join("\n") + "\n", skipped))
}

fn passstore_check(e: &str) -> Result<&str, ai_gent_core::passstore::StoreError> {
    ai_gent_core::passstore::check(e)
}

/// A line break (every one Python's `splitlines` knows, so a reader of the file sees the same lines) or a
/// control character other than tab.
fn refused_in_env_value(v: &str) -> bool {
    v.chars()
        .any(|c| files::is_line_break(c) || (c.is_control() && c != '\t'))
}

fn quote_env_value(name: &str, key: &str, v: &str) -> Result<String, ProfileError> {
    if refused_in_env_value(v) {
        return Err(ProfileError::new(format!(
            "profile {name}: {key} holds a line break or control character and can't be exported"
        )));
    }
    if !v.contains(char::is_whitespace) && !v.contains('#') {
        return Ok(v.to_string());
    }
    if !v.contains('"') {
        Ok(format!("\"{v}\""))
    } else if !v.contains('\'') {
        Ok(format!("'{v}'"))
    } else {
        Err(ProfileError::new(format!(
            "profile {name}: {key} holds both quote characters and can't be exported"
        )))
    }
}

/// `profiles --export-env --out FILE`: writes a new file (mode 600 from its creation, never an existing
/// one) and returns the line the command prints.
///
/// # Errors
/// No `--out`, an existing file, a value that cannot be written, or a write failure.
pub fn export_env(ctx: &Ctx, out: Option<&Path>) -> Result<String, ProfileError> {
    let Some(out) = out else {
        return Err(ProfileError::new(
            "--export-env needs --out FILE (written with mode 600; nothing is printed)",
        ));
    };
    let out = out.to_path_buf();
    let shown = files::file_name(&out);
    if out.exists() {
        return Err(ProfileError::new(format!(
            "{shown} exists; choose another --out"
        )));
    }
    let loaded = user_profiles(ctx)?;
    let (text, skipped) = export_env_text(&loaded.profiles)?;
    write_new_private(&out, text.as_bytes())
        .map_err(|e| ProfileError::new(format!("cannot write {shown}: {}", e.kind())))?;
    Ok(format!(
        "wrote {shown}: {} profile(s){}",
        loaded.profiles.len(),
        if skipped.is_empty() {
            String::new()
        } else {
            format!("; plain passwords left out for {}", skipped.join(", "))
        }
    ))
}

/// Creates `path` (it must not exist) with mode 600 and writes `bytes`.
pub(crate) fn write_new_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(path)?;
    f.write_all(bytes)?;
    f.flush()
}
