//! Separate append log keeps registration-v1 bytes readable while both logs share one lock.
use super::contributions::summarize;
use super::identity::Ledger;
use super::journal::{Store, canonical, check_path, hash, options, sync_dir};
use super::normalize::{IngestRequest, Observation};
use super::{Error, MAX_RECORD, Result, conflict, corrupt, io_error, limit};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use uuid::Uuid;
const MAX_LOG: u64 = 64 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    schema_version: u32,
    sequence: u64,
    payload_bytes: usize,
    checksum: String,
    payload: IngestRequest,
}
pub(super) fn path(store: &Store) -> PathBuf {
    store.dir.join("observations.jsonl")
}
pub(super) fn read(store: &Store, registration: &Ledger) -> Result<Vec<IngestRequest>> {
    let path = path(store);
    check_path(&store.root, &path, false)?;
    let mut file = match options().read(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(error)),
    };
    if !file.metadata().map_err(io_error)?.is_file() {
        return Err(corrupt());
    }
    super::frames::preflight(&mut file, MAX_LOG)?;
    decode_reader(
        BufReader::with_capacity(64 * 1024, file.take(MAX_LOG + 1)),
        registration,
        MAX_LOG,
    )
}
pub(super) fn decode(bytes: &[u8], registration: &Ledger) -> Result<Vec<IngestRequest>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bytes.last() != Some(&b'\n') {
        return Err(corrupt());
    }
    decode_reader(std::io::Cursor::new(bytes), registration, u64::MAX)
}
fn decode_reader(
    reader: impl BufRead,
    registration: &Ledger,
    max: u64,
) -> Result<Vec<IngestRequest>> {
    let mut records = Vec::new();
    let mut ids = BTreeSet::new();
    let mut frames = super::frames::Frames::with_limit(reader, max);
    while let Some(line) = frames.next()? {
        let frame: Frame = serde_json::from_slice(line).map_err(|_| corrupt())?;
        if frame.schema_version != 1 || frame.sequence != records.len() as u64 + 1 {
            return Err(corrupt());
        }
        let canonical = canonical(&frame.payload)?;
        if canonical.len() != frame.payload_bytes || hash(&canonical) != frame.checksum {
            return Err(corrupt());
        }
        let request = frame.payload;
        if request.schema_version != 1
            || request.operation != "ingest"
            || request.request_id.is_nil()
            || !ids.insert(request.request_id)
            || registration.requests.contains_key(&request.request_id)
        {
            return Err(corrupt());
        }
        request.payload.normalize().map_err(|_| corrupt())?;
        request
            .payload
            .bind(registration, request.repository_id)
            .map_err(|_| corrupt())?;
        records.push(request);
    }
    summarize(&events(&records)).map_err(|_| corrupt())?;
    Ok(records)
}

fn events(records: &[IngestRequest]) -> Vec<Observation> {
    records
        .iter()
        .map(|record| record.payload.clone())
        .collect()
}
fn append(store: &Store, sequence: u64, request: &IngestRequest) -> Result<()> {
    let payload = canonical(request)?;
    let frame = Frame {
        schema_version: 1,
        sequence,
        payload_bytes: payload.len(),
        checksum: hash(&payload),
        payload: request.clone(),
    };
    let mut bytes = serde_json::to_vec(&frame).map_err(|_| super::invalid())?;
    bytes.push(b'\n');
    if bytes.len() > MAX_RECORD {
        return Err(limit());
    }
    let path = path(store);
    check_path(&store.root, &path, false)?;
    let mut file = options()
        .append(true)
        .create(true)
        .open(path)
        .map_err(io_error)?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(corrupt());
    }
    if metadata.len() + bytes.len() as u64 > MAX_LOG {
        return Err(limit());
    }
    super::durable::append(&mut file, &bytes)?;
    super::durable::boundary("append_directory_sync")?;
    sync_dir(&store.dir)
}
pub(super) fn check_request_id(store: &Store, ledger: &Ledger, id: Uuid) -> Result<()> {
    if read(store, ledger)?.iter().any(|r| r.request_id == id) {
        return Err(conflict());
    }
    Ok(())
}
fn response(records: &[IngestRequest], index: usize) -> Result<Value> {
    let selected = &records[..=index];
    let request = &records[index];
    let prior = &selected[..index];
    let before = summarize(&events(prior))?;
    let summary = summarize(&events(selected))?;
    let duplicate = summary.observation_count == before.observation_count;
    Ok(
        json!({"schema_version":1,"operation":"ingest","ok":true,"result":{
        "event_id":request.payload.event_id,"event_digest":hash(&canonical(&request.payload)?),
        "duplicate":duplicate,"normalized":request.payload.normalize()?,"summary":summary},"warnings":["Source identity and coverage are not independently verified."]}),
    )
}
pub(super) fn parse(input: &[u8]) -> Result<IngestRequest> {
    let value: Value = serde_json::from_slice(input).map_err(|_| super::invalid())?;
    let fields = |value: &Value, keys: &[&str]| keys.iter().all(|key| value.get(key).is_some());
    let payload = &value["payload"];
    if !fields(
        payload,
        &[
            "schema_version",
            "event_id",
            "task",
            "attempt_id",
            "run_id",
            "parent_run_id",
            "segment_id",
            "phase",
            "sequence",
            "identity",
            "source",
            "raw_usage",
            "counters",
            "quality",
            "attribution",
            "attestation",
            "contribution",
            "supersedes",
            "extensions",
            "execution_scope",
            "observed_at",
            "occurred_at",
            "host_session_id",
        ],
    ) || !fields(
        &payload["counters"],
        &[
            "input_total",
            "cached_input",
            "cache_creation_input",
            "output_total",
            "reasoning_output",
        ],
    ) || !fields(
        &payload["identity"],
        &[
            "agent_app",
            "agent_version",
            "requested_provider",
            "requested_model",
            "effective_provider",
            "effective_model",
            "effort",
            "identity_quality",
            "config_digest",
            "model_vendor",
            "service_tier",
            "provider_region",
        ],
    ) || !fields(
        &payload["source"],
        &[
            "adapter",
            "adapter_version",
            "namespace",
            "request_id",
            "event_id",
            "scope",
            "semantics",
            "reference",
            "digest",
        ],
    ) {
        return Err(super::invalid());
    }
    serde_json::from_value(value).map_err(|_| super::invalid())
}
pub(super) fn ingest(root: &Path, request: &IngestRequest) -> Result<Value> {
    let store = Store::open(root, false)?;
    if !store.log.exists() {
        return Err(Error::new(
            "evidence_missing",
            "usage requires a registered run",
            3,
        ));
    }
    let _lock = store.lock(false)?;
    let ledger = store.load()?;
    let mut records = read(&store, &ledger)?;
    super::transfer::check_request_id(&store, request.request_id)?;
    super::receipts::verify_current(&store)?;
    super::receipts::check_request_id(&store, request.request_id)?;
    super::recovery::check_request_id(&store, request.request_id)?;
    if ledger.requests.contains_key(&request.request_id) {
        return Err(conflict());
    }
    if let Some(index) = records
        .iter()
        .position(|r| r.request_id == request.request_id)
    {
        if records[index] != *request {
            return Err(conflict());
        }
        options()
            .write(true)
            .open(path(&store))
            .and_then(|f| f.sync_all())
            .map_err(io_error)?;
        sync_dir(&store.dir)?;
        return response(&records, index);
    }
    if records
        .iter()
        .any(|r| r.payload.event_id == request.payload.event_id && r.payload != request.payload)
    {
        return Err(conflict());
    }
    request.payload.normalize()?;
    request.payload.bind(&ledger, request.repository_id)?;
    records.push(request.clone());
    let result = response(&records, records.len() - 1)?;
    append(&store, records.len() as u64, request)?;
    Ok(result)
}
pub(super) fn replay(root: &Path) -> Result<Value> {
    let store = Store::open(root, false)?;
    let (ledger, records) = if store.log.exists() {
        let _lock = store.lock(false)?;
        let ledger = store.load()?;
        let records = read(&store, &ledger)?;
        // Assemble under the same lock used by registration and ingestion.
        super::receipts::verify_current(&store)?;
        let mut value = replay_value(&ledger, &records)?;
        value["result"]["receipt_head"] = json!(super::receipts::head(&store)?);
        return Ok(value);
    } else {
        if path(&store).exists() {
            return Err(corrupt());
        }
        (Ledger::default(), Vec::new())
    };
    replay_value(&ledger, &records)
}
pub(super) fn replay_value(ledger: &Ledger, records: &[IngestRequest]) -> Result<Value> {
    Ok(
        json!({"schema_version":1,"operation":"replay","ok":true,"result":{
        "repository_id":ledger.repository_id,"sequence":ledger.sequence,
        "usage_sequence":records.len(),"runs":ledger.runs.values().collect::<Vec<_>>(),
        "usage":summarize(&events(records))?},"warnings":[]}),
    )
}
