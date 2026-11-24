//! Bounded, checksummed registration log. Corruption blocks writes rather than discarding evidence.
use super::identity::{Ledger, Operation, Request, Run};
use super::{Error, MAX_RECORD, Result, conflict, corrupt, io_error, limit};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAX_JOURNAL: u64 = 64 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    request: Request,
    result: Run,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    schema_version: u32,
    sequence: u64,
    payload_bytes: usize,
    checksum: String,
    payload: Entry,
}

pub(super) fn canonical<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    fn emit(v: &serde_json::Value, out: &mut Vec<u8>) {
        match v {
            serde_json::Value::Object(m) => {
                out.push(b'{');
                let mut keys: Vec<_> = m.keys().collect();
                keys.sort();
                for (i, k) in keys.into_iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    out.extend(serde_json::to_vec(k).expect("JSON key serializes"));
                    out.push(b':');
                    emit(&m[k], out);
                }
                out.push(b'}');
            }
            serde_json::Value::Array(a) => {
                out.push(b'[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    emit(v, out);
                }
                out.push(b']');
            }
            _ => out.extend(serde_json::to_vec(v).expect("JSON value serializes")),
        }
    }
    let value = serde_json::to_value(value).map_err(|_| super::invalid())?;
    let mut out = Vec::new();
    emit(&value, &mut out);
    Ok(out)
}
pub(super) fn hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(super) fn check_path(root: &Path, path: &Path, directory: bool) -> Result<()> {
    let relative = path.strip_prefix(root).map_err(|_| super::invalid())?;
    let mut current = root.to_path_buf();
    for part in relative.components() {
        if !matches!(part, std::path::Component::Normal(_)) {
            return Err(super::invalid());
        }
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(m) => {
                if m.file_type().is_symlink()
                    || (current != path && !m.is_dir())
                    || (current == path && if directory { !m.is_dir() } else { !m.is_file() })
                {
                    return Err(Error::new(
                        "scope_mismatch",
                        "registration path is not a permitted regular file or directory",
                        4,
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error(e)),
        }
    }
    Ok(())
}
pub(super) fn options() -> OpenOptions {
    let mut o = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    o
}
pub(super) fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)
            .and_then(|f| f.sync_all())
            .map_err(io_error)?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}
pub(super) fn directory(root: &Path, path: &Path) -> Result<()> {
    check_path(root, path, true)?;
    if !path.exists() {
        let mut b = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        match b.create(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io_error(e)),
        }
        check_path(root, path, true)?;
    }
    // Also flush on retry after a previous directory-sync failure.
    sync_dir(path.parent().ok_or_else(super::invalid)?)
}
pub(super) fn ignore_usage(root: &Path, directory: &Path) -> Result<()> {
    let path = directory.join(".gitignore");
    check_path(root, &path, false)?;
    match options().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(b"*\n")
                .and_then(|()| file.sync_all())
                .map_err(io_error)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut file = options()
                .read(true)
                .write(true)
                .open(&path)
                .map_err(io_error)?;
            if !file.metadata().map_err(io_error)?.is_file() {
                return Err(super::invalid());
            }
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(64)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            if bytes != b"*\n" {
                return Err(Error::new(
                    "policy_denied",
                    "private usage ignore marker differs; preserve it for review",
                    3,
                ));
            }
            file.sync_all().map_err(io_error)?;
        }
        Err(error) => return Err(io_error(error)),
    }
    sync_dir(directory)
}

pub(super) struct Store {
    pub(super) root: PathBuf,
    pub(super) dir: PathBuf,
    pub(super) log: PathBuf,
}
impl Store {
    pub(super) fn open(root: &Path, create: bool) -> Result<Self> {
        let root = root.canonicalize().map_err(io_error)?;
        if !root.is_dir() {
            return Err(super::invalid());
        }
        let mut dir = root.clone();
        for part in [".ai-gent", "usage", "journal"] {
            dir.push(part);
            if create {
                directory(&root, &dir)?;
            } else {
                check_path(&root, &dir, true)?;
            }
        }
        let log = dir.join("registrations.jsonl");
        check_path(&root, &log, false)?;
        Ok(Self { root, dir, log })
    }
    pub(super) fn lock(&self, create: bool) -> Result<File> {
        let path = self.dir.join("registrations.lock");
        check_path(&self.root, &path, false)?;
        let f = options()
            .write(true)
            .create(create)
            .truncate(false)
            .open(path)
            .map_err(io_error)?;
        if !f.metadata().map_err(io_error)?.is_file() {
            return Err(super::invalid());
        }
        let begin = Instant::now();
        loop {
            match f.try_lock() {
                Ok(()) => return Ok(f),
                Err(std::fs::TryLockError::WouldBlock) => {
                    if begin.elapsed() >= Duration::from_secs(5) {
                        let mut e = Error::new("lock_timeout", "registration journal is busy", 5);
                        e.retryable = true;
                        return Err(e);
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(std::fs::TryLockError::Error(e)) => return Err(io_error(e)),
            }
        }
    }
    pub(super) fn load(&self) -> Result<Ledger> {
        let f = match options().read(true).open(&self.log) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Ledger::default()),
            Err(e) => return Err(io_error(e)),
        };
        if !f.metadata().map_err(io_error)?.is_file() {
            return Err(super::invalid());
        }
        let mut bytes = Vec::new();
        f.take(MAX_JOURNAL + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() as u64 > MAX_JOURNAL {
            return Err(limit());
        }
        decode(&bytes)
    }
    fn append(&self, sequence: u64, request: Request, result: Run) -> Result<()> {
        let payload = Entry { request, result };
        let bytes = canonical(&payload)?;
        let frame = Frame {
            schema_version: 1,
            sequence,
            payload_bytes: bytes.len(),
            checksum: hash(&bytes),
            payload,
        };
        let mut line = serde_json::to_vec(&frame).map_err(|_| super::invalid())?;
        line.push(b'\n');
        if line.len() > MAX_RECORD {
            return Err(limit());
        }
        check_path(&self.root, &self.log, false)?;
        let mut f = options()
            .append(true)
            .create(true)
            .open(&self.log)
            .map_err(io_error)?;
        let size = f.metadata().map_err(io_error)?.len();
        if size + line.len() as u64 > MAX_JOURNAL {
            return Err(limit());
        }
        super::durable::append(&mut f, &line)?;
        super::durable::boundary("append_directory_sync")?;
        sync_dir(&self.dir)
    }
    fn task_fingerprint(&self, request: &Request) -> Result<String> {
        let Operation::Register(r) = &request.operation else {
            return Ok(String::new());
        };
        let primary = self.fingerprint_task(&r.task)?;
        if let Some(scope) = &r.execution_scope
            && scope.kind != super::identity::Attribution::Exclusive
        {
            let mut fingerprints = vec![(r.task.clone(), primary)];
            for task in &scope.beneficiaries {
                if task != &r.task {
                    fingerprints.push((task.clone(), self.fingerprint_task(task)?));
                }
            }
            return Ok(hash(&canonical(&(scope, fingerprints))?));
        }
        Ok(primary)
    }
    fn fingerprint_task(&self, task: &super::identity::TaskKey) -> Result<String> {
        let path = self
            .root
            .join("specs")
            .join(&task.feature_id)
            .join("tasks.md");
        check_path(&self.root, &path, false)?;
        let f = options().read(true).open(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::new(
                    "evidence_missing",
                    "task is not declared in this repository",
                    3,
                )
            } else {
                io_error(e)
            }
        })?;
        if !f.metadata().map_err(io_error)?.is_file() {
            return Err(super::invalid());
        }
        let mut bytes = Vec::new();
        f.take(MAX_RECORD as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(io_error)?;
        if bytes.len() > MAX_RECORD {
            return Err(limit());
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| super::invalid())?;
        let lines: Vec<_> = text
            .lines()
            .filter_map(|line| {
                let s = line.trim_start();
                ["- [ ] ", "- [x] ", "- [X] "]
                    .into_iter()
                    .find_map(|prefix| s.strip_prefix(prefix))
                    .filter(|s| s.split_whitespace().next() == Some(task.task_id.as_str()))
                    .map(str::trim_end)
            })
            .collect();
        if lines.len() != 1 {
            return Err(Error::new(
                "evidence_missing",
                "task must be declared exactly once in this repository",
                3,
            ));
        }
        Ok(hash(lines[0].as_bytes()))
    }
}
pub(super) fn decode(bytes: &[u8]) -> Result<Ledger> {
    if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
        return Err(corrupt());
    }
    let mut state = Ledger::default();
    if bytes.is_empty() {
        return Ok(state);
    }
    for line in bytes[..bytes.len() - 1].split(|c| *c == b'\n') {
        if line.len() > MAX_RECORD {
            return Err(limit());
        }
        let frame: Frame = serde_json::from_slice(line).map_err(|_| corrupt())?;
        if frame.schema_version != 1 || frame.sequence != state.sequence + 1 {
            return Err(corrupt());
        }
        let payload = canonical(&frame.payload)?;
        if payload.len() != frame.payload_bytes || hash(&payload) != frame.checksum {
            return Err(corrupt());
        }
        let Entry { request, result } = frame.payload;
        request.validate(request.name()).map_err(|_| corrupt())?;
        if state.requests.contains_key(&request.request_id) {
            return Err(corrupt());
        }
        state
            .apply(&request, &result.task_fingerprint, Some(&result))
            .map_err(|_| corrupt())?;
    }
    Ok(state)
}
pub(super) fn execute(root: &Path, request: &Request) -> Result<Run> {
    let store = Store::open(root, true)?;
    let _lock = store.lock(true)?;
    ignore_usage(&store.root, store.dir.parent().ok_or_else(super::invalid)?)?;
    let mut state = store.load()?;
    super::receipts::verify_current(&store)?;
    super::receipts::check_request_id(&store, request.request_id)?;
    super::transfer::check_request_id(&store, request.request_id)?;
    super::recovery::check_request_id(&store, request.request_id)?;
    if let Some((prior, result)) = state.requests.get(&request.request_id) {
        if prior != request {
            return Err(conflict());
        }
        // A prior invocation may have written but failed to flush; retry must flush before ACK.
        options()
            .write(true)
            .open(&store.log)
            .and_then(|f| f.sync_all())
            .map_err(io_error)?;
        sync_dir(&store.dir)?;
        return Ok(result.clone());
    }
    super::observations::check_request_id(&store, &state, request.request_id)?;
    let fingerprint = store.task_fingerprint(request)?;
    let result = state.apply(request, &fingerprint, None)?;
    store.append(state.sequence, request.clone(), result.clone())?;
    Ok(result)
}
