//! The commands over the traits (spec 043 T009): `query`, `plan`, `apply`, `restore` and `script`, as
//! `cmd_query`, `cmd_plan`, `cmd_apply`, `cmd_restore` and `cmd_script` of `plugins/db/scripts/dbrun.py`
//! do them, with the output text and the exit codes of the original.
//!
//! A [`Runner`] holds the three things a command talks to: a [`Connector`] (executes), a [`Backup`]
//! (backs up and restores) and a [`CommandLog`] (records). Policy lives here and nowhere else:
//!
//! - the command log entry is written BEFORE the first statement runs; a log that cannot be written stops
//!   the command with nothing executed;
//! - `plan` and `apply` need a proven local, writable database, and `apply` re-checks it, so a plan made
//!   earlier is never trusted on its own (a database with links to other databases is not local);
//! - `apply` loads the plan through the tamper check, refuses a second apply of the same plan (an
//!   exclusive claim file, [`claim_apply`]), backs up, then sends ONE write request with the plan's expected
//!   counts; the result must say `ok` AND `committed`, anything else is a failure;
//! - `script` has no connector in its signature path and never executes.
//!
//! Every message that leaves a function passes the profile's [`Redactor`].
use super::backup::{Backup, BackupError, BackupTarget};
use super::log::{Clock, CommandLog, LogEntry, Subject};
use super::mask::mask_result;
use super::plan::{
    NewPlan, Plan, PlanError, ScriptSpec, claim_apply, create_plan, load_plan, plan_path,
    release_apply, write_plan, write_script,
};
use super::redact::Redactor;
use super::{classify_statement, cost_rules, require_user_database, split_statements};
use crate::connector::{ConnFacts, Connector, ExecResult, Mode, Request, Stage};
use serde_json::Value;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Exit codes of `dbrun.py`.
pub const EXIT_OK: i32 = 0;
/// An operational failure.
pub const EXIT_ERROR: i32 = 1;
/// A usage error.
pub const EXIT_USAGE: i32 = 2;
/// A policy refusal.
pub const EXIT_REFUSED: i32 = 3;
/// A connection failure.
pub const EXIT_CONNECT: i32 = 4;

/// A command that stopped. [`Fail::report`] is the line the runner prints on stderr.
#[derive(Debug, PartialEq, Eq)]
pub enum Fail {
    /// Policy said no. Exit 3.
    Refused(String),
    /// Operational failure with its exit code (1, or 4 for a connection).
    Failed(String, i32),
}

impl Fail {
    fn failed(m: impl Into<String>) -> Self {
        Self::Failed(m.into(), EXIT_ERROR)
    }

    /// The exit code.
    #[must_use]
    pub fn code(&self) -> i32 {
        match self {
            Self::Refused(_) => EXIT_REFUSED,
            Self::Failed(_, c) => *c,
        }
    }

    /// `REFUSED: ...` or `error: ...`, redacted.
    #[must_use]
    pub fn report(&self, redactor: &Redactor) -> String {
        match self {
            Self::Refused(m) => redactor.redact(&format!("REFUSED: {m}")),
            Self::Failed(m, _) => redactor.redact(&format!("error: {m}")),
        }
    }
}

impl From<PlanError> for Fail {
    fn from(e: PlanError) -> Self {
        match e {
            PlanError::Refused(m) => Self::Refused(m),
            PlanError::Failed(m) => Self::failed(m),
        }
    }
}

impl From<BackupError> for Fail {
    fn from(e: BackupError) -> Self {
        match e {
            BackupError::Refused(m) => Self::Refused(m),
            BackupError::Failed(m) => Self::failed(m),
        }
    }
}

impl From<super::log::LogError> for Fail {
    fn from(e: super::log::LogError) -> Self {
        Self::failed(e.0)
    }
}

impl From<super::Refused> for Fail {
    fn from(r: super::Refused) -> Self {
        Self::Refused(r.0)
    }
}

/// What a command printed and how it ended (a rollback prints and returns exit 1, as Python).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Done {
    /// Exit code.
    pub code: i32,
    /// Standard output.
    pub out: String,
    /// Standard error.
    pub err: String,
}

/// The database a command acts on: the profile and what the classification proved.
#[derive(Clone)]
pub struct Target {
    /// Profile name.
    pub name: String,
    /// Engine word.
    pub engine: String,
    /// Connection facts (password included; never printed).
    pub conn: ConnFacts,
    /// The database container, when there is one.
    pub container: Option<String>,
    /// Class word after classification.
    pub class: String,
    /// Whether writes are allowed (proven local).
    pub writable: bool,
    /// `file`, `host` or `container`.
    pub via: String,
    /// The classification evidence.
    pub evidence: Vec<String>,
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Target")
            .field("name", &self.name)
            .field("engine", &self.engine)
            .field("class", &self.class)
            .field("writable", &self.writable)
            .finish_non_exhaustive()
    }
}

impl Target {
    /// From a loaded profile and its classification.
    #[must_use]
    pub fn from_profile(p: &crate::profiles::Profile, c: &crate::profiles::Classification) -> Self {
        let via = match c.via {
            crate::profiles::Via::File => "file",
            crate::profiles::Via::Host => "host",
            crate::profiles::Via::Container => "container",
        };
        Self {
            name: p.name.clone(),
            engine: p.engine.clone(),
            conn: ConnFacts {
                host: p.host.clone(),
                port: p.port.clone(),
                database: p.database.clone(),
                service: p.service.clone(),
                sid: p.sid.clone(),
                user: p.user.clone(),
                password: p.password.as_ref().map(|s| s.expose().to_string()),
                path: p.path.clone(),
            },
            container: p.container.clone(),
            class: c.class.as_str().into(),
            writable: c.writable,
            via: via.into(),
            evidence: c.evidence.clone(),
        }
    }

    fn redactor(&self) -> Redactor {
        Redactor::from_parts(
            self.conn.password.as_deref(),
            self.conn.host.as_deref(),
            self.conn.user.as_deref(),
        )
    }

    fn subject(&self) -> Subject<'_> {
        Subject {
            name: &self.name,
            engine: &self.engine,
            class: &self.class,
            writable: self.writable,
            via: &self.via,
            evidence: &self.evidence,
        }
    }

    fn backup_target(&self) -> BackupTarget {
        BackupTarget {
            engine: self.engine.clone(),
            database: self.conn.database.clone(),
            user: self.conn.user.clone(),
            password: self.conn.password.clone(),
            host: self.conn.host.clone(),
            path: self.conn.path.clone(),
            container: self.container.clone(),
        }
    }

    fn require_local(&self, action: &str) -> Result<(), Fail> {
        if self.writable {
            return Ok(());
        }
        Err(Fail::Refused(format!(
            "{action} needs a proven local database; {} is {} ({}). For this database, write a change script with `dbrun script` for a person to run",
            self.name,
            self.class,
            self.evidence.join("; ")
        )))
    }

    fn require_user_database(&self, action: &str) -> Result<(), Fail> {
        Ok(require_user_database(
            &self.engine,
            self.conn.database.as_deref(),
            action,
        )?)
    }
}

/// `query` arguments (defaults as `dbrun.py query`: 30 s, 100 rows, table format).
#[derive(Debug, Clone)]
pub struct QueryArgs {
    /// The SQL text.
    pub sql: String,
    /// Why (goes to the log).
    pub reason: String,
    /// Show masked columns in clear (the user asked).
    pub reveal: bool,
    /// Run through the engine's EXPLAIN.
    pub explain: bool,
    /// Allow an unfiltered `COUNT` on a non-local database.
    pub allow_full_count: bool,
    /// Rows per statement.
    pub max_rows: usize,
    /// Seconds.
    pub timeout: f64,
    /// `table`, `json` or `csv`.
    pub format: String,
}

impl QueryArgs {
    /// Defaults with the given SQL and reason.
    #[must_use]
    pub fn new(sql: &str, reason: &str) -> Self {
        Self {
            sql: sql.into(),
            reason: reason.into(),
            reveal: false,
            explain: false,
            allow_full_count: false,
            max_rows: 100,
            timeout: 30.0,
            format: "table".into(),
        }
    }
}

/// The commands' collaborators.
pub struct Runner<'a> {
    /// Executes statements.
    pub connector: &'a dyn Connector,
    /// Backs up and restores.
    pub backup: &'a dyn Backup,
    /// The command log.
    pub log: &'a CommandLog,
    /// Time and ids.
    pub clock: &'a dyn Clock,
    /// Where plans live.
    pub plans_dir: PathBuf,
    /// Where backups live.
    pub backups_dir: PathBuf,
}

fn float_text(s: Option<f64>) -> String {
    s.map_or_else(|| "0".into(), |v| format!("{v}"))
}

/// Parses `--expect`: comma-separated entries, each an integer, or `skip` (also an empty entry) for "no
/// count check on this statement". Owner decision 2026-11-14: any other entry is REFUSED (Python read it as
/// "skip", so a typo such as `2x` silently dropped a check); only ASCII digits count.
///
/// # Errors
/// A refusal naming the position of the bad entry, never its text.
pub fn parse_expect(text: &str) -> Result<Vec<Option<i64>>, Fail> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    text.split(',')
        .enumerate()
        .map(|(i, x)| {
            let t = x.trim();
            if t.is_empty() || t.eq_ignore_ascii_case("skip") {
                return Ok(None);
            }
            let digits = t.strip_prefix('-').unwrap_or(t);
            if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
                return Err(Fail::Refused(format!(
                    "--expect entry {} is not an integer or `skip`",
                    i + 1
                )));
            }
            t.parse::<i64>().map(Some).map_err(|_| {
                Fail::Refused(format!(
                    "--expect entry {} does not fit a 64-bit integer",
                    i + 1
                ))
            })
        })
        .collect()
}

impl Runner<'_> {
    fn start(
        &self,
        t: &Target,
        action: &str,
        reason: &str,
        statements: &[String],
        extra: &str,
    ) -> Result<LogEntry, Fail> {
        Ok(self
            .log
            .start(self.clock, action, &t.subject(), reason, statements, extra)?)
    }

    /// `dbrun query`: read-only statements, cost rules, masked output.
    ///
    /// # Errors
    /// [`Fail`] for a refusal (a statement that is not read-only, a cost rule, Oracle `EXPLAIN` on a remote
    /// database) or an operational failure before anything ran.
    pub fn query(&self, t: &Target, a: &QueryArgs) -> Result<Done, Fail> {
        self.query_checked(t, a)
    }

    fn query_checked(&self, t: &Target, a: &QueryArgs) -> Result<Done, Fail> {
        let statements = split_statements(&a.sql, &t.engine)?;
        if statements.is_empty() {
            return Err(Fail::failed("no statements"));
        }
        for (i, stmt) in statements.iter().enumerate() {
            let (kind, why) = classify_statement(stmt, &t.engine);
            if kind != "read" {
                return Err(Fail::Refused(format!(
                    "statement {} is not read-only ({why}). `query` only reads; writes go through `plan`/`apply` on a proven local database, or `script` for a person to run elsewhere",
                    i + 1
                )));
            }
            if let Some(rule) = cost_rules(stmt, &t.class, a.allow_full_count, &t.engine) {
                return Err(Fail::Refused(format!("statement {}: {rule}", i + 1)));
            }
        }
        if a.explain && t.engine == "oracle" && t.class != "local" {
            return Err(Fail::Refused(
                "Oracle's EXPLAIN PLAN writes to PLAN_TABLE; not allowed on a remote database"
                    .into(),
            ));
        }
        let extra = format!(
            "{}{}",
            if a.reveal {
                "reveal: yes (the user asked)"
            } else {
                "masked"
            },
            if a.explain { " · explain" } else { "" }
        );
        let entry = self.start(t, "query", &a.reason, &statements, &extra)?;
        let mut req = Request::new(&t.engine, t.conn.clone(), Mode::Read, statements);
        req.timeout = a.timeout;
        req.max_rows = a.max_rows;
        req.explain = a.explain;
        let res = self.connector.run(&req);
        let mut done = Done::default();
        let (true, Some(results)) = (res.ok, &res.results) else {
            let (stage, error) = failure_parts(&res);
            self.log
                .result(&entry, &format!("FAILED at {stage}: {error}"))?;
            done.err = format!(
                "{}\n",
                t.redactor().redact(&format!("FAILED ({stage}): {error}"))
            );
            done.code = if res.stage == Some(Stage::Connect) {
                EXIT_CONNECT
            } else {
                EXIT_ERROR
            };
            return Ok(done);
        };
        let mut summary = Vec::new();
        for (i, r) in results.iter().enumerate() {
            let (Some(columns), Some(rows)) = (&r.columns, &r.rows) else {
                summary.push(format!("#{}: no result set", i + 1));
                continue;
            };
            let truncated = r.truncated.unwrap_or(false);
            let (rows, masked) = mask_result(columns, rows, a.reveal);
            summary.push(format!(
                "#{}: {} row(s){}{}",
                i + 1,
                rows.len(),
                if truncated { " (truncated)" } else { "" },
                if masked.is_empty() {
                    String::new()
                } else {
                    format!(", masked {} column(s)", masked.len())
                }
            ));
            if results.len() > 1 {
                let _ = writeln!(done.out, "-- statement {}", i + 1);
            }
            match a.format.as_str() {
                "json" => {
                    let masked_obj: Vec<(String, Value)> = masked
                        .iter()
                        .map(|(c, k)| (c.clone(), Value::String(k.clone())))
                        .collect();
                    let _ = writeln!(
                        done.out,
                        "{}",
                        py_json_object(&[
                            ("columns", &Value::from(columns.clone())),
                            ("rows", &Value::from(rows)),
                            ("truncated", &Value::Bool(truncated)),
                            ("masked", &py_object_value(&masked_obj)),
                        ])
                    );
                }
                "csv" => {
                    done.out
                        .push_str(&csv_line(columns.iter().map(String::as_str)));
                    for row in &rows {
                        done.out.push_str(&csv_line(
                            row.iter()
                                .map(|v| match v {
                                    Value::Null => String::new(),
                                    other => py_str(other),
                                })
                                .collect::<Vec<_>>()
                                .iter()
                                .map(String::as_str),
                        ));
                    }
                }
                _ => {
                    done.out.push_str(&table(columns, &rows, 120));
                    let mut notes = vec![format!("{} row(s)", rows.len())];
                    if truncated {
                        notes.push(format!("truncated at --max-rows {}", a.max_rows));
                    }
                    if !masked.is_empty() {
                        notes.push(format!(
                            "masked: {}",
                            masked
                                .iter()
                                .map(|(c, k)| format!("{c} ({k})"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                    let _ = writeln!(done.out, "({})", notes.join("; "));
                }
            }
        }
        self.log.result(
            &entry,
            &format!(
                "ok · {} · {} s",
                summary.join("; "),
                float_text(res.seconds)
            ),
        )?;
        Ok(done)
    }

    /// `dbrun plan`: records a plan for a proven local database; nothing executes.
    ///
    /// # Errors
    /// [`Fail`] for a database that is not proven local, a system database, DDL without a backup method, a
    /// split that disagrees, no statements, or a log that cannot be written (the plan file is then removed).
    pub fn plan(&self, t: &Target, sql: &str, reason: &str, expect: &str) -> Result<Done, Fail> {
        t.require_local("plan")?;
        t.require_user_database("plan")?;
        let expect = parse_expect(expect)?;
        let stamp = self.clock.now();
        let id = format!(
            "{}-{}-{}",
            stamp.date.replace('-', ""),
            stamp.iso.get(11..19).unwrap_or("000000").replace(':', ""),
            self.clock.token(4)
        );
        let supported = self.backup.supported(&t.engine);
        let new = NewPlan {
            id: &id,
            profile: &t.name,
            engine: &t.engine,
            reason,
            created: &stamp.iso,
            sql,
            expect,
            backup_supported: supported,
            evidence: &t.evidence,
        };
        let plan = create_plan(&self.plans_dir, &new)?;
        let path = plan_path(&self.plans_dir, &id)?;
        let logged = self
            .start(t, "plan", reason, &plan.statements, &format!("plan {id}"))
            .and_then(|e| {
                self.log
                    .result(&e, &format!("plan recorded at {}", path.display()))
                    .map_err(Fail::from)
            });
        if let Err(e) = logged {
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        let mut out = String::new();
        let _ = writeln!(out, "Plan {id} — {} ({}, proven local)\n", t.name, t.engine);
        let _ = writeln!(out, "Reason: {reason}");
        for (i, s) in plan.statements.iter().enumerate() {
            let exp = plan
                .expect
                .get(i)
                .copied()
                .flatten()
                .map_or_else(|| "not given".to_string(), |n| n.to_string());
            let _ = write!(
                out,
                "\n{}. expected rows: {exp}{}\n{s}\n",
                i + 1,
                if plan.ddl.contains(&(i + 1)) {
                    "  [DDL]"
                } else {
                    ""
                }
            );
        }
        let _ = writeln!(
            out,
            "\nBackup before applying: {}. Everything runs in one transaction; it commits only if every expected row count matches.",
            plan.backup
        );
        if supported {
            let _ = writeln!(
                out,
                "Restore afterwards: dbrun restore {} {id} --confirmed",
                t.name
            );
        } else {
            out.push_str("Restore afterwards: not available (no backup); undoing a committed plan means writing the reverse statements as a new plan\n");
        }
        let _ = writeln!(
            out,
            "\nShow this plan to the user. Only after they confirm: dbrun apply {} {id} --confirmed",
            t.name
        );
        Ok(Done {
            code: EXIT_OK,
            out,
            err: String::new(),
        })
    }

    /// `dbrun apply`: the confirmed plan runs in one transaction after a backup.
    ///
    /// # Errors
    /// [`Fail`] for no `--confirmed`, a database that is not proven local (re-proven now), a system database,
    /// a tampered, foreign or already applied plan, a connection that fails (exit 4), a backup that fails, or a
    /// log that cannot be written (nothing ran in any of these cases). A rollback is `Ok` with exit 1.
    pub fn apply(
        &self,
        t: &Target,
        plan_id: &str,
        confirmed: bool,
        timeout: f64,
    ) -> Result<Done, Fail> {
        if !confirmed {
            return Err(Fail::Refused(
                "apply needs --confirmed, which means the user confirmed this exact plan".into(),
            ));
        }
        let mut t = t.clone();
        t.require_local("apply")?;
        t.require_user_database("apply")?;
        let mut plan = load_plan(&self.plans_dir, &t.name, plan_id)?;
        if plan.engine != t.engine {
            return Err(Fail::Refused(format!(
                "plan {plan_id} was made for a {} database, not {}",
                plan.engine, t.engine
            )));
        }
        let mut probe = Request::new(&t.engine, t.conn.clone(), Mode::Test, Vec::new());
        probe.max_rows = 1;
        let probed = self.connector.run(&probe);
        if !probed.ok {
            let (_, error) = failure_parts(&probed);
            return Err(Fail::Failed(
                format!("can't connect: {error}"),
                EXIT_CONNECT,
            ));
        }
        if probed.links.unwrap_or(0) > 0 {
            t.writable = false;
            t.evidence.push(format!(
                "NOT PROVEN LOCAL: the database has {} link(s) to other databases",
                probed.links.unwrap_or(0)
            ));
        }
        t.require_local("apply")?;
        // 1. one apply per plan (a refused second apply leaves no log entry behind)
        claim_apply(&self.plans_dir, plan_id)?;
        // 2. the log, before anything changes; a log that cannot be written gives the claim back
        let entry = match self.start(
            &t,
            "apply",
            &plan.reason,
            &plan.statements,
            &format!("plan {}", plan.id),
        ) {
            Ok(e) => e,
            Err(e) => {
                release_apply(&self.plans_dir, plan_id);
                return Err(e);
            }
        };
        // 3. the backup
        let mut location = "none".to_string();
        if self.backup.supported(&t.engine) {
            match self
                .backup
                .take(&t.backup_target(), &plan.id, &self.backups_dir)
            {
                Ok(l) => {
                    location = l;
                    self.log.result(
                        &entry,
                        &format!("backup taken: the backup file of plan {}", plan.id),
                    )?;
                }
                Err(e) => {
                    release_apply(&self.plans_dir, plan_id);
                    let msg = t.redactor().redact(&e.to_string());
                    let _ = self
                        .log
                        .result(&entry, &format!("NOT APPLIED, backup failed: {msg}"));
                    return Err(e.into());
                }
            }
        }
        plan.backup_location = Some(location.clone());
        if let Err(e) = write_plan(&plan_path(&self.plans_dir, plan_id)?, &plan, false) {
            release_apply(&self.plans_dir, plan_id);
            return Err(e.into());
        }
        // 4. the write: one request, the plan's own counts
        let mut req = Request::new(
            &t.engine,
            t.conn.clone(),
            Mode::Write,
            plan.statements.clone(),
        );
        req.expect.clone_from(&plan.expect);
        req.timeout = timeout;
        req.max_rows = 1000;
        let res = self.connector.run(&req);
        let counts: Vec<i64> = res
            .results
            .as_ref()
            .map(|r| r.iter().map(|s| s.rowcount).collect())
            .unwrap_or_default();
        let committed =
            res.ok && res.committed == Some(true) && counts.len() == plan.statements.len();
        let mut done = Done::default();
        let mysql_ddl = !plan.ddl.is_empty() && matches!(t.engine.as_str(), "mysql" | "mariadb");
        if !committed {
            let (label, stage, error) = if res.ok {
                let why = if res.committed == Some(true) {
                    "the runner reported a commit but returned a different number of row counts than statements"
                } else {
                    "the runner did not report whether the transaction committed"
                };
                ("OUTCOME UNKNOWN", "execute".to_string(), why.to_string())
            } else {
                let (stage, error) = failure_parts(&res);
                // Only a verify failure is a rollback the connector confirmed, and not when MySQL or
                // MariaDB already committed the DDL implicitly. A connect failure means nothing ran.
                let rolled =
                    res.stage == Some(Stage::Verify) && res.committed != Some(true) && !mysql_ddl;
                let label = if rolled {
                    "ROLLED BACK"
                } else if res.stage == Some(Stage::Connect) {
                    "NOT APPLIED"
                } else {
                    "OUTCOME UNKNOWN"
                };
                (label, stage, error)
            };
            let proven = label == "ROLLED BACK" || label == "NOT APPLIED";
            if proven {
                release_apply(&self.plans_dir, plan_id);
                // Nothing changed, so this plan's backup file is not needed and must not block the next try.
                let file = Path::new(&location);
                if file.parent() == Some(self.backups_dir.as_path())
                    && file
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with(plan.id.as_str()))
                {
                    let _ = std::fs::remove_file(file);
                }
            }
            let _ = self
                .log
                .result(&entry, &format!("{label} at {stage}: {error}"));
            let _ = writeln!(
                done.out,
                "{}",
                t.redactor().redact(&format!("{label} ({stage}): {error}"))
            );
            if !proven {
                done.out.push_str("The plan is marked as applied because this failure may have changed the database: check the state, then make a new plan.\n");
            }
            if mysql_ddl {
                let _ = writeln!(
                    done.out,
                    "MySQL commits DDL immediately: check the state, and restore with `dbrun restore {} {} --confirmed` if needed",
                    t.name, plan.id
                );
            }
            done.code = EXIT_ERROR;
            return Ok(done);
        }
        self.log.result(
            &entry,
            &format!(
                "COMMITTED · row counts {counts:?} · {} s",
                float_text(res.seconds)
            ),
        )?;
        let _ = writeln!(
            done.out,
            "COMMITTED plan {} · row counts {counts:?}",
            plan.id
        );
        if let Some(w) = &res.warning {
            let _ = writeln!(done.out, "warning: {}", t.redactor().redact(w));
        }
        if location == "none" {
            done.out
                .push_str("Backup: none. To undo, write the reverse statements as a new plan\n");
        } else {
            let _ = writeln!(
                done.out,
                "Backup: the backup file of plan {}. To undo: dbrun restore {} {} --confirmed",
                plan.id, t.name, plan.id
            );
        }
        Ok(done)
    }

    /// `dbrun restore`: the backup of an applied plan comes back (the whole database).
    ///
    /// # Errors
    /// [`Fail`] for no `--confirmed`, a database that is not proven local, a plan that fails the tamper check or
    /// has no backup, a location that is not the plan's own, a tool failure, or a log that cannot be written.
    pub fn restore(&self, t: &Target, plan_id: &str, confirmed: bool) -> Result<Done, Fail> {
        if !confirmed {
            return Err(Fail::Refused(
                "restore needs --confirmed, which means the user asked for this restore".into(),
            ));
        }
        t.require_local("restore")?;
        t.require_user_database("restore")?;
        let plan: Plan = load_plan(&self.plans_dir, &t.name, plan_id)?;
        let location = plan.backup_location.clone().unwrap_or_default();
        if location.is_empty() || location == "none" {
            return Err(Fail::failed(format!(
                "plan {} has no backup to restore",
                plan.id
            )));
        }
        let entry = self.start(
            t,
            "restore",
            &format!("restore plan {}", plan.id),
            &[],
            &format!("from the backup file of plan {}", plan.id),
        )?;
        match self
            .backup
            .restore(&t.backup_target(), &plan.id, &location, &self.backups_dir)
        {
            Ok(()) => {
                self.log.result(&entry, "restored")?;
                Ok(Done {
                    code: EXIT_OK,
                    out: format!(
                        "RESTORED {} from the backup file of plan {}\n",
                        t.name, plan.id
                    ),
                    err: String::new(),
                })
            }
            Err(e) => {
                let _ = self.log.result(
                    &entry,
                    &format!("FAILED: {}", t.redactor().redact(&e.to_string())),
                );
                Err(e.into())
            }
        }
    }

    /// `dbrun script`: writes the change script for a person to run. No connector is used.
    ///
    /// # Errors
    /// [`Fail`] for an `UPDATE` or `DELETE` without `WHERE`, an existing `out`, no statements, a split that
    /// disagrees, or a log that cannot be written (the script file is then removed).
    pub fn script(&self, t: &Target, sql: &str, reason: &str, out: &Path) -> Result<Done, Fail> {
        let stamp = self.clock.now();
        let spec = ScriptSpec {
            profile: &t.name,
            engine: &t.engine,
            class: &t.class,
            reason,
            now: &stamp.iso,
            sql,
        };
        write_script(&spec, out)?;
        let statements = split_statements(sql, &t.engine)?;
        let logged = self
            .start(
                t,
                "script",
                reason,
                &statements,
                &format!("written to {} — not executed", out.display()),
            )
            .and_then(|e| {
                self.log
                    .result(&e, "script written, not executed")
                    .map_err(Fail::from)
            });
        if let Err(e) = logged {
            let _ = std::fs::remove_file(out);
            return Err(e);
        }
        Ok(Done {
            code: EXIT_OK,
            out: format!(
                "Wrote {} (not executed). Fill in the verification and rollback sections, then hand it over.\n",
                out.display()
            ),
            err: String::new(),
        })
    }
}

fn failure_parts(r: &ExecResult) -> (String, String) {
    (
        r.stage.map_or_else(
            || "connect".to_string(),
            |s| format!("{s:?}").to_lowercase(),
        ),
        r.error.clone().unwrap_or_else(|| "no error text".into()),
    )
}

/// Python's `str(v)` for a JSON value in a table or CSV cell.
#[must_use]
pub fn py_str(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Python's `print_table`: cells cut at `width`, columns padded to the widest cell.
#[must_use]
pub fn table(columns: &[String], rows: &[Vec<Value>], width: usize) -> String {
    let cell = |v: &Value| {
        let s = if v.is_null() {
            "NULL".to_string()
        } else {
            py_str(v).replace('\n', "⏎")
        };
        if s.chars().count() <= width {
            s
        } else {
            format!("{}…", s.chars().take(width - 1).collect::<String>())
        }
    };
    let mut grid: Vec<Vec<String>> = vec![
        columns
            .iter()
            .map(|c| cell(&Value::String(c.clone())))
            .collect(),
    ];
    grid.extend(rows.iter().map(|r| r.iter().map(cell).collect()));
    let widths: Vec<usize> = (0..columns.len())
        .map(|i| {
            grid.iter()
                .map(|r| r.get(i).map_or(0, |c| c.chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let pad = |r: &[String]| {
        r.iter()
            .zip(&widths)
            .map(|(c, w)| format!("{c}{}", " ".repeat(w - c.chars().count())))
            .collect::<Vec<_>>()
            .join(" | ")
    };
    let mut out = format!("{}\n", pad(&grid[0]));
    out.push_str(
        &widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("-+-"),
    );
    out.push('\n');
    for r in &grid[1..] {
        out.push_str(&pad(r));
        out.push('\n');
    }
    out
}

/// Python `csv.writer` (`QUOTE_MINIMAL`, `\r\n`).
#[must_use]
pub fn csv_line<'a>(cells: impl Iterator<Item = &'a str>) -> String {
    let q = |c: &str| {
        if c.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", c.replace('"', "\"\""))
        } else {
            c.to_string()
        }
    };
    format!("{}\r\n", cells.map(q).collect::<Vec<_>>().join(","))
}

/// `json.dumps` with Python's separators and `ensure_ascii`.
#[must_use]
pub fn py_json(v: &Value) -> String {
    match v {
        Value::String(s) => {
            let mut o = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => o.push_str("\\\""),
                    '\\' => o.push_str("\\\\"),
                    '\n' => o.push_str("\\n"),
                    '\r' => o.push_str("\\r"),
                    '\t' => o.push_str("\\t"),
                    c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                        let mut b = [0u16; 2];
                        for u in c.encode_utf16(&mut b) {
                            let _ = write!(o, "\\u{u:04x}");
                        }
                    }
                    c => o.push(c),
                }
            }
            o.push('"');
            o
        }
        Value::Array(a) => format!("[{}]", a.iter().map(py_json).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!("{}: {}", py_json(&Value::String(k.clone())), py_json(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    }
}

fn py_object_value(pairs: &[(String, Value)]) -> Value {
    Value::Object(pairs.iter().cloned().collect())
}

fn py_json_object(pairs: &[(&str, &Value)]) -> String {
    format!(
        "{{{}}}",
        pairs
            .iter()
            .map(|(k, v)| format!("\"{k}\": {}", py_json(v)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
