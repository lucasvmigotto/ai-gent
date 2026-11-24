//! Immutable local snapshots bind durable journal bytes without accepting tasks.
use super::identity::Ledger;
use super::journal::{Store, canonical, check_path, directory, hash, options, sync_dir};
use super::normalize::{Counters, IngestRequest};
use super::{Error, Result, conflict, corrupt, durable, invalid, io_error, limit, observations};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_RECEIPTS: usize = 4096;
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SnapshotRequest {
    schema_version: u32,
    request_id: Uuid,
    repository_id: Uuid,
    operation: String,
    payload: SnapshotScope,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SnapshotScope {
    expected_registration_sequence: u64,
    expected_usage_sequence: u64,
    predecessor_digest: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SnapshotSummary {
    observation_count: u64,
    contribution_count: u64,
    counters: Counters,
    exclusive_counters: Counters,
    shared_counters: Counters,
    unattributed_counters: Counters,
    known_lower_bounds: Counters,
    total_tokens: Option<u64>,
    known_total_lower_bound: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    kind: String,
    request_id: Uuid,
    request_digest: String,
    repository_id: Uuid,
    registration_sequence: u64,
    usage_sequence: u64,
    registration_digest: String,
    usage_digest: String,
    predecessor_digest: Option<String>,
    summary: SnapshotSummary,
    assurance: String,
    coverage: String,
}
fn directory_path(store: &Store) -> Result<PathBuf> {
    Ok(store.dir.parent().ok_or_else(invalid)?.join("receipts"))
}
fn read(store: &Store) -> Result<BTreeMap<String, Receipt>> {
    let path = directory_path(store)?;
    check_path(&store.root, &path, true)?;
    let entries = match std::fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(io_error(error)),
    };
    let mut receipts = BTreeMap::new();
    let mut requests = BTreeSet::new();
    let mut count = 0;
    for entry in entries {
        count += 1;
        if count > MAX_RECEIPTS * 2 {
            return Err(limit());
        }
        let entry = entry.map_err(io_error)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(corrupt)?;
        if name.starts_with(".pending-") {
            continue;
        }
        let filename = name.strip_suffix(".json").ok_or_else(corrupt)?;
        let digest = format!("sha256:{filename}");
        if !super::identity::digest(&digest) {
            return Err(corrupt());
        }
        let bytes = durable::read(&store.root, &entry.path(), super::MAX_RECORD as u64)?
            .ok_or_else(corrupt)?;
        if hash(&bytes) != digest {
            return Err(corrupt());
        }
        let receipt: Receipt = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
        if canonical(&receipt)? != bytes
            || receipt.schema_version != 1
            || receipt.kind != "usage_snapshot"
            || receipt.assurance != "local_unverified"
            || receipt.coverage != "unclosed"
            || receipt.request_id.is_nil()
            || receipt.repository_id.is_nil()
            || !requests.insert(receipt.request_id)
            || [
                &receipt.request_digest,
                &receipt.registration_digest,
                &receipt.usage_digest,
            ]
            .iter()
            .any(|digest| !super::identity::digest(digest))
            || receipt
                .predecessor_digest
                .as_ref()
                .is_some_and(|digest| !super::identity::digest(digest))
        {
            return Err(corrupt());
        }
        receipts.insert(digest, receipt);
        if receipts.len() > MAX_RECEIPTS {
            return Err(limit());
        }
    }
    let mut root_count = 0;
    let mut children = BTreeMap::new();
    for (digest, receipt) in &receipts {
        if let Some(parent) = &receipt.predecessor_digest {
            let prior = receipts.get(parent).ok_or_else(corrupt)?;
            if children.insert(parent, digest).is_some()
                || receipt.repository_id != prior.repository_id
                || receipt.registration_sequence < prior.registration_sequence
                || receipt.usage_sequence < prior.usage_sequence
            {
                return Err(corrupt());
            }
        } else {
            root_count += 1;
        }
    }
    if !receipts.is_empty() && root_count != 1 {
        return Err(corrupt());
    }
    // A one-root, one-child graph can still contain a disconnected cycle.
    if let Some(first) = receipts
        .iter()
        .find(|(_, receipt)| receipt.predecessor_digest.is_none())
    {
        let mut visited = BTreeSet::new();
        let mut cursor = Some(first.0);
        while let Some(digest) = cursor {
            if !visited.insert(digest) {
                return Err(corrupt());
            }
            cursor = children.get(digest).copied();
        }
        if visited.len() != receipts.len() {
            return Err(corrupt());
        }
    }
    Ok(receipts)
}
fn latest(receipts: &BTreeMap<String, Receipt>) -> Option<&String> {
    let predecessors: BTreeSet<_> = receipts
        .values()
        .filter_map(|receipt| receipt.predecessor_digest.as_ref())
        .collect();
    receipts
        .keys()
        .find(|digest| !predecessors.contains(digest))
}
pub(super) fn head(store: &Store) -> Result<Option<String>> {
    Ok(latest(&read(store)?).cloned())
}
pub(super) fn check_request_id(store: &Store, id: Uuid) -> Result<()> {
    if read(store)?
        .values()
        .any(|receipt| receipt.request_id == id)
    {
        return Err(conflict());
    }
    Ok(())
}
fn response(digest: &str, receipt: &Receipt) -> Value {
    json!({"schema_version":1,"operation":"snapshot","ok":true,"result":{"receipt_digest":digest,"receipt":receipt},"warnings":["Local evidence snapshot does not certify provider usage or task acceptance."]})
}
pub(super) fn snapshot(root: &Path, input: &[u8]) -> Result<Value> {
    durable::record_limit(input)?;
    let value: Value = serde_json::from_slice(input).map_err(|_| invalid())?;
    if value["payload"].get("predecessor_digest").is_none() {
        return Err(invalid());
    }
    let request: SnapshotRequest = serde_json::from_value(value).map_err(|_| invalid())?;
    if request.schema_version != 1 {
        return Err(Error::new(
            "unsupported_version",
            "only snapshot version 1 is supported",
            2,
        ));
    }
    if request.operation != "snapshot"
        || request.request_id.is_nil()
        || request.repository_id.is_nil()
        || request
            .payload
            .predecessor_digest
            .as_ref()
            .is_some_and(|digest| !super::identity::digest(digest))
    {
        return Err(invalid());
    }
    let store = Store::open(root, false)?;
    if !store.log.exists() {
        return Err(Error::new(
            "evidence_missing",
            "snapshot requires a registered repository",
            3,
        ));
    }
    let _lock = store.lock(false)?;
    let ledger = store.load()?;
    let records = observations::read(&store, &ledger)?;
    // Retry acknowledgement must validate the bound source prefixes too.
    verify_current(&store)?;
    let receipts = read(&store)?;
    let request_digest = hash(&canonical(&request)?);
    if let Some((digest, receipt)) = receipts
        .iter()
        .find(|(_, receipt)| receipt.request_id == request.request_id)
    {
        if receipt.request_digest != request_digest {
            return Err(conflict());
        }
        let path = directory_path(&store)?.join(format!(
            "{}.json",
            digest.strip_prefix("sha256:").ok_or_else(corrupt)?
        ));
        durable::publish(&store, &path, &canonical(receipt)?)?;
        return Ok(response(digest, receipt));
    }
    if ledger.repository_id != Some(request.repository_id)
        || ledger.requests.contains_key(&request.request_id)
        || records
            .iter()
            .any(|record| record.request_id == request.request_id)
        || latest(&receipts) != request.payload.predecessor_digest.as_ref()
        || ledger.sequence != request.payload.expected_registration_sequence
        || records.len() as u64 != request.payload.expected_usage_sequence
    {
        return Err(conflict());
    }
    super::recovery::check_request_id(&store, request.request_id)?;
    super::transfer::check_request_id(&store, request.request_id)?;
    if receipts.len() >= MAX_RECEIPTS {
        return Err(limit());
    }
    let receipt = make_receipt(&store, &ledger, &records, &request, request_digest)?;
    let bytes = canonical(&receipt)?;
    durable::record_limit(&bytes)?;
    let digest = hash(&bytes);
    let receipt_dir = directory_path(&store)?;
    directory(&store.root, &receipt_dir)?;
    let path = receipt_dir.join(format!(
        "{}.json",
        digest.strip_prefix("sha256:").ok_or_else(corrupt)?
    ));
    durable::publish(&store, &path, &bytes)?;
    Ok(response(&digest, &receipt))
}
fn make_receipt(
    store: &Store,
    ledger: &Ledger,
    records: &[IngestRequest],
    request: &SnapshotRequest,
    request_digest: String,
) -> Result<Receipt> {
    let registration =
        durable::read(&store.root, &store.log, durable::MAX_LOG)?.ok_or_else(corrupt)?;
    let usage = durable::read(&store.root, &observations::path(store), durable::MAX_LOG)?
        .unwrap_or_default();
    // Flush every evidence file before publishing a receipt, including after a prior failed ACK.
    for path in [&store.log, &observations::path(store)] {
        if path.exists() {
            options()
                .write(true)
                .open(path)
                .and_then(|file| file.sync_all())
                .map_err(io_error)?;
        }
    }
    sync_dir(&store.dir)?;
    verify_evidence(store, &registration, &usage)?;
    let events: Vec<_> = records
        .iter()
        .map(|record| record.payload.clone())
        .collect();
    let summary = super::contributions::summarize(&events)?;
    Ok(Receipt {
        schema_version: 1,
        kind: "usage_snapshot".into(),
        request_id: request.request_id,
        request_digest,
        repository_id: request.repository_id,
        registration_sequence: ledger.sequence,
        usage_sequence: records.len() as u64,
        registration_digest: hash(&registration),
        usage_digest: hash(&usage),
        predecessor_digest: request.payload.predecessor_digest.clone(),
        summary: SnapshotSummary {
            observation_count: u64::try_from(summary.observation_count).map_err(|_| limit())?,
            contribution_count: u64::try_from(summary.contribution_count).map_err(|_| limit())?,
            counters: summary.counters,
            exclusive_counters: summary.exclusive_counters,
            shared_counters: summary.shared_counters,
            unattributed_counters: summary.unattributed_counters,
            known_lower_bounds: summary.known_lower_bounds,
            total_tokens: summary.total_tokens,
            known_total_lower_bound: summary.known_total_lower_bound,
        },
        assurance: "local_unverified".into(),
        coverage: "unclosed".into(),
    })
}

/// Verify that published receipts still bind the surviving journal prefixes.
pub(super) fn verify_evidence(store: &Store, registration: &[u8], usage: &[u8]) -> Result<()> {
    verify_receipts(&read(store)?, registration, usage)
}
pub(super) fn verify_current(store: &Store) -> Result<()> {
    let receipts = read(store)?;
    if receipts.is_empty() {
        return Ok(());
    }
    let registration =
        durable::read(&store.root, &store.log, durable::MAX_LOG)?.unwrap_or_default();
    let usage = durable::read(&store.root, &observations::path(store), durable::MAX_LOG)?
        .unwrap_or_default();
    verify_receipts(&receipts, &registration, &usage)
}
fn verify_receipts(
    receipts: &BTreeMap<String, Receipt>,
    registration: &[u8],
    usage: &[u8],
) -> Result<()> {
    for receipt in receipts.values() {
        for (bytes, sequence, expected) in [
            (
                registration,
                receipt.registration_sequence,
                &receipt.registration_digest,
            ),
            (usage, receipt.usage_sequence, &receipt.usage_digest),
        ] {
            let end = if sequence == 0 {
                0
            } else {
                let index = usize::try_from(sequence - 1).map_err(|_| corrupt())?;
                bytes
                    .iter()
                    .enumerate()
                    .filter(|(_, byte)| **byte == b'\n')
                    .nth(index)
                    .map(|(index, _)| index + 1)
                    .ok_or_else(corrupt)?
            };
            if hash(&bytes[..end]) != *expected {
                return Err(corrupt());
            }
        }
    }
    Ok(())
}
