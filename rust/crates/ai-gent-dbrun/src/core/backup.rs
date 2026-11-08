//! Backup before `apply` and restore after it (spec 043 T008, FR-018, FR-019): the [`Backup`] trait that
//! sits next to the [`crate::connector::Connector`] trait, and its implementation over the engines' own
//! tools run inside the database container (`pg_dump`, `mysqldump`, and their restores), through an
//! [`Exec`] that tests replace with a fake. No Python and no Rust logical dump.
//!
//! Mirrors `backup`, `container_shell` and `cmd_restore` of `plugins/db/scripts/dbrun.py`:
//!
//! - the password reaches the tool as the first line of its stdin, never in argv or the environment of
//!   the container engine; the user and the database name are positional arguments, as in Python;
//! - a dump goes to `<backups>/<plan id>.<ext>`, created exclusively with mode 600; a restore streams that
//!   file into the tool and recreates the whole database (objects a plan created disappear);
//! - SQLite uses the backup API on the file, without a container.
//!
//! Fail closed, beyond Python:
//!
//! - a system database (`master`, `mysql`, `template1`, ...) is refused for a backup and for a restore
//!   ([`require_user_database`]), whatever the caller checked before;
//! - the user, the database and the container name are matched against an allowlist before they become
//!   arguments (a database name `postgresql://...` would make `pg_dump -d` connect elsewhere);
//! - the location a restore reads is recomputed from the plan id and must equal the recorded one: a plan
//!   file edited to point at another file restores nothing;
//! - every exec has a deadline (30 minutes, as Python) and a cap on the bytes it may write; stderr is a
//!   bounded tail, redacted; a failed dump leaves no file behind;
//! - Oracle has no backup (`apply` stays refused there, plan decision) and SQL Server's `BACKUP DATABASE`
//!   needs the autocommit path of its connector, which does not exist yet: both are refused, not guessed.
use super::redact::Redactor;
use super::{Refused, require_user_database};
use ai_gent_core::env::Env;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Deadline of one backup or restore exec, as `container_shell` in `dbrun.py` (1800 s).
pub const EXEC_TIMEOUT: Duration = Duration::from_mins(30);
/// Most bytes a dump may write (a bound, not a promise: 64 GiB).
pub const MAX_BACKUP_BYTES: u64 = 64 * 1024 * 1024 * 1024;
/// Bytes of stderr kept, and characters of it shown (`[-400:]` in Python).
const STDERR_KEEP: usize = 8 * 1024;
const STDERR_SHOWN: usize = 400;

/// Whether `dbrun.py` can back up a database of this engine (`BACKUP_SUPPORT`).
#[must_use]
pub fn backup_supported(engine: &str) -> bool {
    matches!(
        engine,
        "postgresql" | "mysql" | "mariadb" | "sqlite" | "sqlserver"
    )
}

/// Why a backup or a restore did not happen.
#[derive(Debug, PartialEq, Eq)]
pub enum BackupError {
    /// The policy says no (system database, no method, bad location). Exit 3.
    Refused(String),
    /// The tool or the file system failed. Exit 1.
    Failed(String),
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(m) | Self::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for BackupError {}

impl From<Refused> for BackupError {
    fn from(r: Refused) -> Self {
        Self::Refused(r.0)
    }
}

/// What a backup needs to know about the database. `Debug` never prints the password.
#[derive(Clone, Default)]
pub struct BackupTarget {
    /// Engine word.
    pub engine: String,
    /// Database name.
    pub database: Option<String>,
    /// Login name.
    pub user: Option<String>,
    /// Login password (first line of the tool's stdin).
    pub password: Option<String>,
    /// Host, only so error text can be redacted.
    pub host: Option<String>,
    /// SQLite file.
    pub path: Option<String>,
    /// The database container (every engine but SQLite).
    pub container: Option<String>,
}

impl fmt::Debug for BackupTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackupTarget")
            .field("engine", &self.engine)
            .field("database", &self.database)
            .field("container", &self.container)
            .finish_non_exhaustive()
    }
}

impl BackupTarget {
    fn redactor(&self) -> Redactor {
        Redactor::from_parts(
            self.password.as_deref(),
            self.host.as_deref(),
            self.user.as_deref(),
        )
    }
}

/// Takes and restores backups of a proven-local database. `dir` is the backups directory the caller owns.
pub trait Backup {
    /// Whether this engine has a backup method (`BACKUP_SUPPORT`).
    fn supported(&self, engine: &str) -> bool {
        backup_supported(engine)
    }

    /// Backs the database up for plan `plan_id` and returns the location recorded in the plan.
    ///
    /// # Errors
    /// [`BackupError::Refused`] for a system database, an engine without a method or an unsafe input;
    /// [`BackupError::Failed`] when the tool fails.
    fn take(&self, target: &BackupTarget, plan_id: &str, dir: &Path)
    -> Result<String, BackupError>;

    /// Restores the backup recorded at `location` for plan `plan_id`.
    ///
    /// # Errors
    /// As [`Backup::take`], and [`BackupError::Refused`] when `location` is not the place `take` uses.
    fn restore(
        &self,
        target: &BackupTarget,
        plan_id: &str,
        location: &str,
        dir: &Path,
    ) -> Result<(), BackupError>;
}

/// One command run inside the database container.
#[derive(Debug)]
pub struct ExecSpec<'a> {
    /// Container name (already validated).
    pub container: &'a str,
    /// The `sh -c` script.
    pub script: &'a str,
    /// `$1` of the script.
    pub user: &'a str,
    /// `$2` of the script.
    pub database: &'a str,
    /// First bytes of stdin (the password line).
    pub stdin: &'a [u8],
    /// A file streamed to stdin after `stdin`.
    pub stdin_file: Option<&'a Path>,
    /// Where stdout goes; created exclusively with mode 600 by the exec, removed on any failure.
    pub stdout_file: Option<&'a Path>,
    /// Deadline.
    pub timeout: Duration,
    /// Most bytes stdout may write.
    pub max_stdout: u64,
}

/// What an [`Exec`] reports; `stderr_tail` is raw (the caller redacts it).
#[derive(Debug, Default)]
pub struct ExecOutput {
    /// Exit code (124 for a timeout, 125 when stdout passed its cap).
    pub code: i32,
    /// The last bytes of stderr.
    pub stderr_tail: String,
}

/// Runs a command in the database container. The real one is [`ContainerExec`]; tests use a fake.
pub trait Exec {
    /// Runs the spec.
    ///
    /// # Errors
    /// A fixed message when the engine program cannot run or its pipes fail.
    fn run(&self, spec: &ExecSpec<'_>) -> Result<ExecOutput, String>;
}

/// The real [`Exec`]: `<engine> exec -i <container> sh -c <script> sh <user> <database>`.
pub struct ContainerExec {
    program: PathBuf,
    env: Env,
}

impl ContainerExec {
    /// Runs through the container engine at `program` with exactly the environment `env`.
    #[must_use]
    pub fn new(program: impl Into<PathBuf>, env: Env) -> Self {
        Self {
            program: program.into(),
            env,
        }
    }
}

fn create_private(path: &Path) -> io::Result<std::fs::File> {
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(path)
}

impl Exec for ContainerExec {
    fn run(&self, spec: &ExecSpec<'_>) -> Result<ExecOutput, String> {
        let mut cmd = Command::new(&self.program);
        cmd.args(["exec", "-i", spec.container, "sh", "-c", spec.script, "sh"])
            .arg(spec.user)
            .arg(spec.database)
            .env_clear();
        for (k, v) in self.env.iter() {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut sink = match spec.stdout_file {
            Some(p) => Some(
                create_private(p)
                    .map_err(|e| format!("cannot create the backup file: {}", e.kind()))?,
            ),
            None => None,
        };
        let outcome = (|| {
            let mut child = cmd
                .spawn()
                .map_err(|_| "the container engine could not be started".to_string())?;
            let (Some(stdin), Some(mut stdout), Some(mut stderr)) =
                (child.stdin.take(), child.stdout.take(), child.stderr.take())
            else {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the engine pipes could not be opened".to_string());
            };
            let head = spec.stdin.to_vec();
            let file = spec.stdin_file.map(Path::to_path_buf);
            std::thread::spawn(move || {
                let mut stdin = stdin;
                let _ = stdin.write_all(&head);
                if let Some(f) = file
                    && let Ok(mut f) = std::fs::File::open(f)
                {
                    let _ = io::copy(&mut f, &mut stdin);
                }
            });
            let cap = spec.max_stdout;
            let (out_tx, out_rx) = std::sync::mpsc::channel();
            let (err_tx, err_rx) = std::sync::mpsc::channel();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stop_w = std::sync::Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut written = 0u64;
                let mut over = false;
                let mut buf = vec![0u8; 64 * 1024];
                while let Ok(n) = stdout.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    written += n as u64;
                    if written > cap {
                        over = true;
                        break;
                    }
                    if let Some(f) = sink.as_mut()
                        && f.write_all(&buf[..n]).is_err()
                    {
                        over = true;
                        break;
                    }
                }
                stop_w.store(over, std::sync::atomic::Ordering::SeqCst);
                let _ = out_tx.send(over);
            });
            std::thread::spawn(move || {
                let mut tail: Vec<u8> = Vec::new();
                let mut buf = [0u8; 4096];
                while let Ok(n) = stderr.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    tail.extend_from_slice(&buf[..n]);
                    if tail.len() > STDERR_KEEP {
                        tail.drain(..tail.len() - STDERR_KEEP);
                    }
                }
                let _ = err_tx.send(tail);
            });
            let deadline = Instant::now() + spec.timeout;
            let mut timed_out = false;
            let status = loop {
                match child.try_wait() {
                    Ok(Some(s)) => break s.code(),
                    Ok(None)
                        if Instant::now() < deadline
                            && !stop.load(std::sync::atomic::Ordering::SeqCst) =>
                    {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    _ => {
                        // Deadline, a cap hit or a lost pipe: kill our child by handle.
                        timed_out = Instant::now() >= deadline;
                        let _ = child.kill();
                        break child.wait().ok().and_then(|s| s.code());
                    }
                }
            };
            // A grandchild that inherited the pipes can keep them open after the engine client is
            // gone: the readers get a short grace, then are left to end on their own.
            let grace = Duration::from_secs(2);
            let over = out_rx.recv_timeout(grace).unwrap_or(true);
            let tail = err_rx.recv_timeout(grace).unwrap_or_default();
            let code = if timed_out {
                124
            } else if over {
                125
            } else {
                status.unwrap_or(-1)
            };
            Ok(ExecOutput {
                code,
                stderr_tail: String::from_utf8_lossy(&tail).into_owned(),
            })
        })();
        // A dump that did not finish cleanly is not a backup.
        if let Some(p) = spec.stdout_file
            && !matches!(&outcome, Ok(o) if o.code == 0)
        {
            let _ = std::fs::remove_file(p);
        }
        outcome
    }
}

/// [`Backup`] over the engines' tools in the container, and the backup API for SQLite.
pub struct EngineBackup<'a> {
    /// Where the tools run.
    pub exec: &'a dyn Exec,
    /// Deadline of one exec.
    pub timeout: Duration,
    /// Cap on a dump's size.
    pub max_bytes: u64,
}

impl<'a> EngineBackup<'a> {
    /// The defaults of `dbrun.py`: 30 minutes, and the crate's size cap.
    #[must_use]
    pub fn new(exec: &'a dyn Exec) -> Self {
        Self {
            exec,
            timeout: EXEC_TIMEOUT,
            max_bytes: MAX_BACKUP_BYTES,
        }
    }
}

const PG_DUMP: &str =
    r#"read -r PGPASSWORD; export PGPASSWORD; exec pg_dump -Fc -h 127.0.0.1 -U "$1" -d "$2""#;
// --clean --create drops and recreates the whole database, so objects a plan created go too; it connects
// to template1 for that, and fails while other sessions (the app) still use the database.
const PG_RESTORE: &str = r#"read -r PGPASSWORD; export PGPASSWORD; exec pg_restore --clean --create --if-exists --no-owner -h 127.0.0.1 -U "$1" -d template1"#;
const MYSQL_RESTORE: &str = r#"read -r MYSQL_PWD; export MYSQL_PWD; c=$(command -v mariadb || command -v mysql); exec "$c" -h 127.0.0.1 -u "$1""#;

fn mysql_dump(mariadb: bool) -> String {
    let dump = if mariadb { "mariadb-dump" } else { "mysqldump" };
    // --databases + --add-drop-database: restore recreates the whole database.
    format!(
        r#"read -r MYSQL_PWD; export MYSQL_PWD; d=$(command -v {dump} || command -v mysqldump); exec "$d" --single-transaction --routines --triggers --events --add-drop-database -h 127.0.0.1 -u "$1" --databases "$2""#
    )
}

fn extension(engine: &str) -> Option<&'static str> {
    match engine {
        "sqlite" => Some("sqlite"),
        "postgresql" => Some("pgdump"),
        "mysql" | "mariadb" => Some("sql"),
        _ => None,
    }
}

/// An argument the tools receive: letters, digits and `_ - . $ #`, never starting with `-`.
fn safe_arg(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.starts_with('-')
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '$' | '#'))
}

struct Prepared<'a> {
    container: &'a str,
    user: &'a str,
    database: &'a str,
    stdin: Vec<u8>,
}

fn refuse(m: &str) -> BackupError {
    BackupError::Refused(m.to_string())
}

fn prepare(t: &BackupTarget) -> Result<Prepared<'_>, BackupError> {
    let container = t.container.as_deref().unwrap_or("");
    if !crate::profiles::container::valid_container_name(container) {
        return Err(refuse(
            "backup needs the profile's database container, named with a valid container name",
        ));
    }
    let user = t.user.as_deref().unwrap_or("");
    if !safe_arg(user) {
        return Err(refuse(
            "the profile's user has characters a backup command does not accept",
        ));
    }
    let database = t.database.as_deref().unwrap_or("");
    // An empty database is allowed only where the engine's tool takes none (PostgreSQL names the user's own).
    if !(database.is_empty() && t.engine == "postgresql" || safe_arg(database)) {
        return Err(refuse(
            "the profile's database has characters a backup command does not accept",
        ));
    }
    let pw = t.password.as_deref().unwrap_or("");
    if pw.contains(['\n', '\r', '\0']) {
        return Err(refuse(
            "the profile's password holds a line break, which cannot travel on the tool's stdin",
        ));
    }
    let mut stdin = pw.as_bytes().to_vec();
    stdin.push(b'\n');
    Ok(Prepared {
        container,
        user,
        database,
        stdin,
    })
}

impl EngineBackup<'_> {
    fn run(
        &self,
        t: &BackupTarget,
        script: &str,
        stdin_file: Option<&Path>,
        stdout_file: Option<&Path>,
        what: &str,
    ) -> Result<(), BackupError> {
        let p = prepare(t)?;
        let spec = ExecSpec {
            container: p.container,
            script,
            user: p.user,
            database: p.database,
            stdin: &p.stdin,
            stdin_file,
            stdout_file,
            timeout: self.timeout,
            max_stdout: self.max_bytes,
        };
        let out = self.exec.run(&spec).map_err(|e| {
            BackupError::Failed(t.redactor().redact(&format!("{what} command failed: {e}")))
        })?;
        // Python's `[-400:]` of stderr, redacted, on one line.
        if out.code != 0 {
            let tail = t.redactor().redact(&out.stderr_tail);
            let flat: String = tail.split_whitespace().collect::<Vec<_>>().join(" ");
            let n = flat.chars().count();
            let shown: String = flat.chars().skip(n.saturating_sub(STDERR_SHOWN)).collect();
            let why = match out.code {
                124 => "it timed out".to_string(),
                125 => "its output passed the size cap".to_string(),
                _ => shown,
            };
            return Err(BackupError::Failed(format!(
                "backup/restore command failed: {why}"
            )));
        }
        Ok(())
    }
}

impl Backup for EngineBackup<'_> {
    fn take(&self, t: &BackupTarget, plan_id: &str, dir: &Path) -> Result<String, BackupError> {
        require_user_database(&t.engine, t.database.as_deref(), "backup")?;
        let Some(ext) = extension(&t.engine) else {
            return Err(match t.engine.as_str() {
                "sqlserver" => BackupError::Refused(
                    "a SQL Server backup needs the connector's autocommit path (BACKUP DATABASE cannot run in a transaction), which is not available yet".into(),
                ),
                e => BackupError::Refused(format!("no backup method for {e}")),
            });
        };
        let dst = dir.join(format!("{plan_id}.{ext}"));
        match t.engine.as_str() {
            "sqlite" => sqlite_take(t, &dst),
            "postgresql" => self
                .run(t, PG_DUMP, None, Some(&dst), "backup")
                .map(|()| path_text(&dst)),
            _ => {
                if t.database.as_deref().is_none_or(str::is_empty) {
                    return Err(BackupError::Failed(
                        "a MySQL/MariaDB backup needs the profile's database; name it with <PROFILE>_DATABASE".into(),
                    ));
                }
                self.run(
                    t,
                    &mysql_dump(t.engine == "mariadb"),
                    None,
                    Some(&dst),
                    "backup",
                )
                .map(|()| path_text(&dst))
            }
        }
    }

    fn restore(
        &self,
        t: &BackupTarget,
        plan_id: &str,
        location: &str,
        dir: &Path,
    ) -> Result<(), BackupError> {
        require_user_database(&t.engine, t.database.as_deref(), "restore")?;
        let Some(ext) = extension(&t.engine) else {
            return Err(BackupError::Refused(format!(
                "no restore method for {}",
                t.engine
            )));
        };
        let expected = dir.join(format!("{plan_id}.{ext}"));
        if location != path_text(&expected) {
            return Err(refuse(
                "the recorded backup location is not where this plan's backup is taken; refusing to restore from it",
            ));
        }
        match t.engine.as_str() {
            "sqlite" => sqlite_restore(t, &expected),
            "postgresql" => self.run(t, PG_RESTORE, Some(&expected), None, "restore"),
            _ => self.run(t, MYSQL_RESTORE, Some(&expected), None, "restore"),
        }
    }
}

fn path_text(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[cfg(feature = "sqlite")]
fn sqlite_take(t: &BackupTarget, dst: &Path) -> Result<String, BackupError> {
    use rusqlite::{Connection, OpenFlags};
    let fail = |m: &str| BackupError::Failed(format!("sqlite backup failed: {m}"));
    let src = t
        .path
        .as_deref()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| refuse("a SQLite backup needs the profile's path"))?;
    let src = std::fs::canonicalize(src).map_err(|e| fail(&format!("{}", e.kind())))?;
    create_private(dst)
        .map_err(|e| fail(&format!("cannot create the backup file: {}", e.kind())))?;
    let result = (|| -> Result<(), String> {
        let a = Connection::open_with_flags(&src, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "cannot open the database read-only".to_string())?;
        let mut b = Connection::open(dst).map_err(|_| "cannot open the backup file".to_string())?;
        let backup = rusqlite::backup::Backup::new(&a, &mut b)
            .map_err(|_| "cannot start the backup".to_string())?;
        backup
            .run_to_completion(64, Duration::from_millis(50), None)
            .map_err(|_| "the backup did not complete".to_string())
    })();
    match result {
        Ok(()) => Ok(path_text(dst)),
        Err(m) => {
            let _ = std::fs::remove_file(dst);
            Err(fail(&m))
        }
    }
}

#[cfg(not(feature = "sqlite"))]
fn sqlite_take(_: &BackupTarget, _: &Path) -> Result<String, BackupError> {
    Err(BackupError::Refused(
        "this build has no SQLite support".into(),
    ))
}

#[cfg(feature = "sqlite")]
fn sqlite_restore(t: &BackupTarget, backup: &Path) -> Result<(), BackupError> {
    use rusqlite::{Connection, OpenFlags};
    let fail = |m: &str| BackupError::Failed(format!("sqlite restore failed: {m}"));
    let live = t
        .path
        .as_deref()
        .filter(|p| !p.is_empty())
        .ok_or_else(|| refuse("a SQLite restore needs the profile's path"))?;
    let live = std::fs::canonicalize(live).map_err(|e| fail(&format!("{}", e.kind())))?;
    // Only a real SQLite file is copied over the database: the header is checked on the opened file.
    let mut head = [0u8; 16];
    let mut f = std::fs::File::open(backup).map_err(|e| fail(&format!("{}", e.kind())))?;
    if !f.metadata().is_ok_and(|m| m.is_file()) {
        return Err(refuse("the backup is not a regular file"));
    }
    if f.read_exact(&mut head).is_err() || &head != b"SQLite format 3\0" {
        return Err(refuse("the backup is not a SQLite database file"));
    }
    drop(f);
    let a = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| fail("cannot open the backup"))?;
    let mut b = Connection::open_with_flags(&live, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_| fail("cannot open the database"))?;
    let run =
        rusqlite::backup::Backup::new(&a, &mut b).map_err(|_| fail("cannot start the restore"))?;
    run.run_to_completion(64, Duration::from_millis(50), None)
        .map_err(|_| fail("the restore did not complete (is the database in use?)"))
}

#[cfg(not(feature = "sqlite"))]
fn sqlite_restore(_: &BackupTarget, _: &Path) -> Result<(), BackupError> {
    Err(BackupError::Refused(
        "this build has no SQLite support".into(),
    ))
}
