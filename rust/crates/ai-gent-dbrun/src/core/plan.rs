//! The plan layer: plan records, their tamper check, and the change script. Pure core logic over the
//! [`Connector`] trait, behind no engine feature; mirrors `cmd_plan`, `load_plan`, `cmd_apply` and
//! `cmd_script` of `plugins/db/scripts/dbrun.py`.
//!
//! - A plan is a JSON file `<id>.json` under a directory the caller passes in. Its `sha256` is taken
//!   over the statements joined with `"\n;\n"`, as Python does; [`load_plan`] refuses a plan made for
//!   another profile and a plan whose statements no longer hash to the recorded value.
//! - [`apply_plan`] loads the plan (so a tampered one runs nothing), takes the backup through a closure
//!   the caller provides (the backup method is engine work), then runs one write request.
//! - [`write_script`] writes the change script for a person to run elsewhere. It takes no connector:
//!   it cannot execute a statement.
//!
//! The SHA-256 here is a small std-only implementation ([`sha256_hex`]); the crate has no hashing
//! dependency. Its tests check it against published vectors and a digest computed outside the crate.
use super::{blank_literals, classify_statement, is_ddl, split_statements};
use crate::connector::{ConnFacts, Connector, ExecResult, Mode, Request};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Why a plan-layer operation did not happen.
#[derive(Debug, PartialEq, Eq)]
pub enum PlanError {
    /// The policy says no (wrong profile, modified plan, unsafe script). Maps to the runner's refusal.
    Refused(String),
    /// Anything else: bad id, missing plan, I/O.
    Failed(String),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(m) | Self::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for PlanError {}

/// A recorded plan: what the user is shown and later confirms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// `YYYYMMDD-HHMMSS-xxxx` (lowercase hex suffix).
    pub id: String,
    /// The profile the plan was made for.
    pub profile: String,
    /// The profile's engine.
    pub engine: String,
    /// Why the change is made.
    pub reason: String,
    /// When the plan was made (the caller's timestamp, ISO 8601).
    pub created: String,
    /// The statements, one per entry.
    pub statements: Vec<String>,
    /// The row count each statement must affect (`null` skips a statement).
    pub expect: Vec<Option<i64>>,
    /// 1-based positions of the DDL statements.
    pub ddl: Vec<usize>,
    /// `required` when a backup is taken before applying, else a note.
    pub backup: String,
    /// SHA-256 (hex) of the statements joined with `"\n;\n"`.
    pub sha256: String,
    /// Where the backup went, set by [`apply_plan`] (`none` without one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_location: Option<String>,
}

/// What [`create_plan`] needs besides the statements.
#[derive(Debug, Clone)]
pub struct NewPlan<'a> {
    /// Plan id; must match `YYYYMMDD-HHMMSS-xxxx`.
    pub id: &'a str,
    /// Profile name.
    pub profile: &'a str,
    /// Engine name.
    pub engine: &'a str,
    /// Why the change is made.
    pub reason: &'a str,
    /// Timestamp text to record.
    pub created: &'a str,
    /// The SQL text; split and classified like the runner does.
    pub sql: &'a str,
    /// Expected row counts per statement.
    pub expect: Vec<Option<i64>>,
    /// Whether the engine's databases can be backed up (`BACKUP_SUPPORT` in Python).
    pub backup_supported: bool,
}

/// SHA-256 of the statements joined as Python does: `"\n;\n".join(statements)`.
#[must_use]
pub fn statements_digest(statements: &[String]) -> String {
    sha256_hex(statements.join("\n;\n").as_bytes())
}

fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == 20
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'-'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'-'
        && b[16..]
            .iter()
            .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

fn plan_path(dir: &Path, id: &str) -> Result<PathBuf, PlanError> {
    if valid_id(id) {
        Ok(dir.join(format!("{id}.json")))
    } else {
        Err(PlanError::Failed(format!("not a plan id: {id}")))
    }
}

fn create_new_private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o.open(path)
}

/// Splits `sql`, builds the plan and writes it to `<dir>/<id>.json` (never overwriting). The
/// directory must exist; the caller decides where plans live.
///
/// # Errors
/// [`PlanError::Refused`] for a split that disagrees or DDL on an engine without backups;
/// [`PlanError::Failed`] for no statements, a bad id or an I/O error.
pub fn create_plan(dir: &Path, new: &NewPlan<'_>) -> Result<Plan, PlanError> {
    let path = plan_path(dir, new.id)?;
    let statements = split_statements(new.sql, new.engine).map_err(|r| PlanError::Refused(r.0))?;
    if statements.is_empty() {
        return Err(PlanError::Failed("no statements".into()));
    }
    let ddl: Vec<usize> = statements
        .iter()
        .enumerate()
        .filter(|(_, s)| is_ddl(s, new.engine))
        .map(|(i, _)| i + 1)
        .collect();
    if !ddl.is_empty() && !new.backup_supported {
        return Err(PlanError::Refused(format!(
            "statements {ddl:?} are DDL; dbrun can't back up a local {} database, so only DML inside a transaction is allowed. Use the project's migration tool, or take a backup yourself",
            new.engine
        )));
    }
    let plan = Plan {
        id: new.id.to_string(),
        profile: new.profile.to_string(),
        engine: new.engine.to_string(),
        reason: new.reason.to_string(),
        created: new.created.to_string(),
        sha256: statements_digest(&statements),
        statements,
        expect: new.expect.clone(),
        ddl,
        backup: if new.backup_supported {
            "required".into()
        } else {
            "none (DML in one transaction)".into()
        },
        backup_location: None,
    };
    write_plan(&path, &plan, true)?;
    Ok(plan)
}

fn write_plan(path: &Path, plan: &Plan, fresh: bool) -> Result<(), PlanError> {
    let text = serde_json::to_string_pretty(plan).map_err(|e| PlanError::Failed(e.to_string()))?;
    let io = |e: std::io::Error| PlanError::Failed(format!("{}: {}", path.display(), e.kind()));
    if fresh {
        create_new_private(path)
            .and_then(|mut f| f.write_all(text.as_bytes()))
            .map_err(io)
    } else {
        std::fs::write(path, text).map_err(io)
    }
}

/// Reads plan `id` from `dir` for `profile`.
///
/// # Errors
/// [`PlanError::Failed`] for a bad id, a missing or unreadable plan; [`PlanError::Refused`] for a plan
/// made for another profile or whose statements no longer hash to the recorded `sha256`.
pub fn load_plan(dir: &Path, profile: &str, id: &str) -> Result<Plan, PlanError> {
    let path = plan_path(dir, id)?;
    let text = std::fs::read_to_string(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => PlanError::Failed(format!("no plan {id}")),
        k => PlanError::Failed(format!("plan {id}: {k}")),
    })?;
    let plan: Plan = serde_json::from_str(&text)
        .map_err(|_| PlanError::Failed(format!("plan {id} is not a valid plan file")))?;
    if plan.profile != profile {
        return Err(PlanError::Refused(format!(
            "plan {id} was made for {}, not {profile}",
            plan.profile
        )));
    }
    if statements_digest(&plan.statements) != plan.sha256 {
        return Err(PlanError::Refused(format!(
            "plan {id} was modified after it was recorded"
        )));
    }
    Ok(plan)
}

/// How [`apply_plan`] runs, defaults as `dbrun.py apply` has them.
#[derive(Debug, Clone, Copy)]
pub struct ApplyOptions {
    /// Take a backup first (the engine and class support one).
    pub backup_supported: bool,
    /// Statement and lock timeout in seconds (`dbrun.py apply --timeout`, 120 by default).
    pub timeout: f64,
    /// Row cap of the write request (1000, as `dbrun.py apply` sends).
    pub max_rows: usize,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            backup_supported: true,
            timeout: 120.0,
            max_rows: 1000,
        }
    }
}

/// Applies a recorded plan: load (refusing a tampered one before anything runs), back up through
/// `backup` when `opts.backup_supported`, record the location, then run the statements as one write
/// request with the plan's `expect`. Confirmation, classification and the connect probe are the
/// caller's. Returns the plan and the connector's result.
///
/// # Errors
/// Whatever [`load_plan`] refuses; [`PlanError::Failed`] when the backup closure fails (nothing runs).
pub fn apply_plan(
    connector: &dyn Connector,
    dir: &Path,
    conn: &ConnFacts,
    profile: &str,
    id: &str,
    opts: ApplyOptions,
    backup: impl FnOnce(&Plan) -> Result<String, String>,
) -> Result<(Plan, ExecResult), PlanError> {
    let mut plan = load_plan(dir, profile, id)?;
    let location = if opts.backup_supported {
        backup(&plan).map_err(|e| PlanError::Failed(format!("backup failed: {e}")))?
    } else {
        "none".to_string()
    };
    plan.backup_location = Some(location);
    write_plan(&plan_path(dir, id)?, &plan, false)?;
    let mut req = Request::new(
        &plan.engine,
        conn.clone(),
        Mode::Write,
        plan.statements.clone(),
    );
    req.expect.clone_from(&plan.expect);
    req.timeout = opts.timeout;
    req.max_rows = opts.max_rows;
    let result = connector.run(&req);
    Ok((plan, result))
}

/// What [`write_script`] needs.
#[derive(Debug, Clone)]
pub struct ScriptSpec<'a> {
    /// Profile name.
    pub profile: &'a str,
    /// Engine name.
    pub engine: &'a str,
    /// The profile's environment class, for the header.
    pub class: &'a str,
    /// Why the change is made.
    pub reason: &'a str,
    /// Timestamp text for the header.
    pub now: &'a str,
    /// The SQL text.
    pub sql: &'a str,
}

/// Writes the change script to `out` for a person to review and run elsewhere. It has no connector and
/// never executes a statement. Refuses an `UPDATE`/`DELETE` without `WHERE`, an `out` that exists, and
/// an engine without a known transaction syntax.
///
/// # Errors
/// [`PlanError::Refused`] for an unsafe statement or a split that disagrees; [`PlanError::Failed`] for
/// no statements, an existing `out`, an unknown engine or an I/O error.
pub fn write_script(spec: &ScriptSpec<'_>, out: &Path) -> Result<String, PlanError> {
    let (profile, engine) = (spec.profile, spec.engine);
    let statements = split_statements(spec.sql, engine).map_err(|r| PlanError::Refused(r.0))?;
    if statements.is_empty() {
        return Err(PlanError::Failed("no statements".into()));
    }
    if out.exists() {
        return Err(PlanError::Failed(format!(
            "{} exists; choose another --out",
            out.display()
        )));
    }
    for (i, stmt) in statements.iter().enumerate() {
        let blanked = blank_literals(stmt, engine).to_uppercase();
        let words: Vec<&str> = blanked
            .split(|c: char| !(c.is_ascii_alphabetic() || c == '_'))
            .filter(|w| !w.is_empty())
            .collect();
        if let Some(&first) = words.first()
            && (first == "UPDATE" || first == "DELETE")
            && !words.contains(&"WHERE")
        {
            let art = if first.starts_with('U') { "an" } else { "a" };
            return Err(PlanError::Refused(format!(
                "statement {} is {art} {first} without WHERE; a change script names the rows it changes",
                i + 1
            )));
        }
    }
    let (begin, commit) = match engine {
        "postgresql" | "sqlite" => ("BEGIN;", "COMMIT;"),
        "mysql" | "mariadb" => ("START TRANSACTION;", "COMMIT;"),
        "sqlserver" => ("BEGIN TRANSACTION;", "COMMIT TRANSACTION;"),
        "oracle" => ("", "COMMIT;"),
        _ => return Err(PlanError::Failed(format!("unsupported engine {engine}"))),
    };
    let kinds: Vec<&str> = statements
        .iter()
        .map(|s| classify_statement(s, engine).0)
        .collect();
    let count = |k: &str| kinds.iter().filter(|x| **x == k).count();
    let ddl_note = if matches!(engine, "mysql" | "mariadb" | "oracle")
        && statements.iter().any(|s| is_ddl(s, engine))
    {
        "-- Note: this engine commits DDL implicitly; the backup is the rollback for DDL statements."
    } else {
        ""
    };
    let mut lines: Vec<String> = vec![
        format!("-- Change script for {profile} ({engine}, {})", spec.class),
        format!("-- Prepared by ai-gent dbrun on {}. NOT executed: a person reviews and runs it.", spec.now),
        format!("-- Reason: {}", spec.reason),
        format!("-- Statements: {} ({} write, {} read)", statements.len(), count("write"), count("read")),
        "--".into(),
        "-- Before running: take a backup, run the verification queries below and compare the counts,".into(),
        "-- run inside a transaction where the engine allows it, and keep the rollback section at hand.".into(),
        String::new(),
        "-- == Pre-checks (run first; each must return the expected count) ==".into(),
        "-- <SELECT COUNT(*) ... WHERE <the same condition as each change>  -- expected: N>".into(),
        String::new(),
        "-- == Change ==".into(),
    ];
    if !ddl_note.is_empty() {
        lines.push(ddl_note.into());
    }
    if !begin.is_empty() {
        lines.push(begin.into());
    }
    lines.extend(
        statements
            .iter()
            .map(|s| format!("{};", s.trim_end().trim_end_matches(';'))),
    );
    lines.push("-- Compare the affected row counts with the pre-checks; ROLLBACK instead of COMMIT if they differ.".into());
    lines.push(commit.into());
    lines.extend(
        [
            "",
            "-- == Verification (after the change) ==",
            "-- <SELECTs that prove it worked>",
            "",
            "-- == Rollback ==",
            "-- <statements that undo the change, or the backup to restore — take it first>",
            "",
        ]
        .map(String::from),
    );
    let text = lines.join("\n");
    let io = |e: std::io::Error| PlanError::Failed(format!("{}: {}", out.display(), e.kind()));
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(io)?;
    }
    std::fs::write(out, &text).map_err(io)?;
    Ok(text)
}

/// SHA-256 of `data` as lowercase hex.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write as _;
    sha256(data)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// The first `n` primes.
fn primes(n: usize) -> Vec<u128> {
    let mut found: Vec<u128> = Vec::with_capacity(n);
    let mut c = 2;
    while found.len() < n {
        if found.iter().all(|p| c % p != 0) {
            found.push(c);
        }
        c += 1;
    }
    found
}

/// Largest `x` with `x.pow(degree) <= n` (binary search; `n` stays below 2^110).
fn int_root(n: u128, degree: u32) -> u128 {
    let (mut lo, mut hi) = (0u128, 1u128 << 40);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if mid.pow(degree) <= n {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

/// The low 32 bits of the fractional part of a root, which is how FIPS 180-4 defines its constants.
fn low32(x: u128) -> u32 {
    u32::try_from(x & 0xffff_ffff).expect("masked to 32 bits")
}

fn sha256(data: &[u8]) -> [u8; 32] {
    // FIPS 180-4 §4.2.2 and §5.3.3: K are the fractional parts of the cube roots of the first 64
    // primes, the initial hash the square roots of the first 8, both scaled by 2^32.
    let k: Vec<u32> = primes(64)
        .iter()
        .map(|p| low32(int_root(p << 96, 3)))
        .collect();
    let mut h: Vec<u32> = primes(8)
        .iter()
        .map(|p| low32(int_root(p << 64, 2)))
        .collect();
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    let bits = u64::try_from(data.len())
        .expect("length fits u64")
        .wrapping_mul(8);
    msg.extend_from_slice(&bits.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v: [u32; 8] = h.clone().try_into().expect("8 words");
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (hi, vi) in h.iter_mut().zip(v) {
            *hi = hi.wrapping_add(vi);
        }
    }
    let mut out = [0u8; 32];
    for (chunk, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        *chunk = word.to_be_bytes();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    /// Digests of the joined statements, computed outside the crate (coreutils `sha256sum` over the
    /// exact bytes `"\n;\n".join(statements)`; the same function as Python's `hashlib.sha256`).
    #[test]
    fn statements_digest_agrees_with_the_python_join() {
        assert_eq!(
            statements_digest(&["a".into(), "b".into()]),
            "dc89cece39801f25fafccfa60627bcf93d36e943bcdc2e524046eb610d68e1c4"
        );
        // Two blocks and a multi-byte character.
        assert_eq!(
            statements_digest(&[
                "UPDATE t SET a = 1 WHERE id = 2".into(),
                "DELETE FROM t WHERE id = 3 -- é".into()
            ]),
            "a856271d26e72b273424ccb6f8bff32b95edcb97a2cc1ff931b9b548bd651631"
        );
    }
}
