//! Explicit digest-pinned recovery isolates only incomplete trailing bytes.
use super::journal::{Store, canonical, check_path, directory, hash, options, sync_dir};
use super::{Error, Result, conflict, corrupt, durable, invalid, io_error, observations};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RecoveryRequest {
    schema_version: u32,
    request_id: Uuid,
    repository_id: Uuid,
    operation: String,
    payload: Expected,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    registration_digest: String,
    usage_digest: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LogPlan {
    original_digest: String,
    prefix_digest: String,
    original_bytes: u64,
    prefix_bytes: u64,
    discarded_tail_bytes: u64,
    existed: bool,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    kind: String,
    request: RecoveryRequest,
    registration: LogPlan,
    usage: LogPlan,
    registration_sequence: u64,
    usage_sequence: u64,
}
fn directory_path(store: &Store) -> Result<PathBuf> {
    Ok(store.dir.parent().ok_or_else(invalid)?.join("quarantine"))
}
fn manifest_path(store: &Store, id: Uuid) -> Result<PathBuf> {
    Ok(directory_path(store)?.join(format!("recovery-{id}.json")))
}
pub(super) fn check_request_id(store: &Store, id: Uuid) -> Result<()> {
    let path = manifest_path(store, id)?;
    check_path(&store.root, &path, false)?;
    if path.exists() {
        return Err(conflict());
    }
    Ok(())
}
fn parse(input: &[u8]) -> Result<RecoveryRequest> {
    durable::record_limit(input)?;
    let request: RecoveryRequest = serde_json::from_slice(input).map_err(|_| invalid())?;
    if request.schema_version != 1 {
        return Err(Error::new(
            "unsupported_version",
            "only recovery version 1 is supported",
            2,
        ));
    }
    if request.operation != "recover"
        || request.request_id.is_nil()
        || request.repository_id.is_nil()
        || !super::identity::digest(&request.payload.registration_digest)
        || !super::identity::digest(&request.payload.usage_digest)
    {
        return Err(invalid());
    }
    Ok(request)
}
fn prefix(bytes: &[u8]) -> &[u8] {
    let end = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    &bytes[..end]
}
fn plan(bytes: Option<&[u8]>) -> LogPlan {
    let original = bytes.unwrap_or_default();
    let prefix = prefix(original);
    LogPlan {
        original_digest: hash(original),
        prefix_digest: hash(prefix),
        original_bytes: original.len() as u64,
        prefix_bytes: prefix.len() as u64,
        discarded_tail_bytes: (original.len() - prefix.len()) as u64,
        existed: bytes.is_some(),
    }
}
fn response(manifest: &Manifest, dry_run: bool) -> Result<Value> {
    Ok(
        json!({"schema_version":1,"operation":"recover","ok":true,"result":{
        "dry_run":dry_run,"manifest_digest":hash(&canonical(manifest)?),"request":manifest.request,
        "registration":manifest.registration,"usage":manifest.usage,
        "registration_sequence":manifest.registration_sequence,"usage_sequence":manifest.usage_sequence,
    },"warnings":["Only an incomplete final frame is isolated. Original evidence is retained; no task is accepted."]}),
    )
}
pub(super) fn recover(root: &Path, input: Option<&[u8]>, dry_run: bool) -> Result<Value> {
    let request = input.map(parse).transpose()?;
    if !dry_run && request.is_none() {
        return Err(invalid());
    }
    let store = Store::open(root, false)?;
    if !store.log.exists() {
        return Err(Error::new(
            "evidence_missing",
            "recovery requires a registered journal",
            3,
        ));
    }
    let _lock = store.lock(false)?;
    let registration = durable::read(&store.root, &store.log, durable::MAX_LOG)?;
    let usage_path = observations::path(&store);
    let usage = durable::read(&store.root, &usage_path, durable::MAX_LOG)?;
    if let Some(request) = &request
        && let Some(result) = resume(
            &store,
            request,
            registration.as_deref(),
            usage.as_deref(),
            dry_run,
        )?
    {
        return Ok(result);
    }
    let ledger = super::journal::decode(prefix(registration.as_deref().unwrap_or_default()))?;
    let repository_id = ledger.repository_id.ok_or_else(|| {
        Error::new(
            "evidence_missing",
            "no complete registration frame survives recovery",
            3,
        )
    })?;
    let records = observations::decode(prefix(usage.as_deref().unwrap_or_default()), &ledger)?;
    super::receipts::verify_evidence(
        &store,
        prefix(registration.as_deref().unwrap_or_default()),
        prefix(usage.as_deref().unwrap_or_default()),
    )?;
    let registration_plan = plan(registration.as_deref());
    let usage_plan = plan(usage.as_deref());
    let request = request.unwrap_or_else(|| RecoveryRequest {
        schema_version: 1,
        request_id: Uuid::new_v4(),
        repository_id,
        operation: "recover".into(),
        payload: Expected {
            registration_digest: registration_plan.original_digest.clone(),
            usage_digest: usage_plan.original_digest.clone(),
        },
    });
    if request.repository_id != repository_id
        || request.payload.registration_digest != registration_plan.original_digest
        || request.payload.usage_digest != usage_plan.original_digest
        || ledger.requests.contains_key(&request.request_id)
        || records
            .iter()
            .any(|record| record.request_id == request.request_id)
    {
        return Err(conflict());
    }
    super::receipts::check_request_id(&store, request.request_id)?;
    let manifest = Manifest {
        schema_version: 1,
        kind: "torn_tail_recovery".into(),
        request,
        registration: registration_plan,
        usage: usage_plan,
        registration_sequence: ledger.sequence,
        usage_sequence: records.len() as u64,
    };
    if !dry_run {
        let quarantine = directory_path(&store)?;
        directory(&store.root, &quarantine)?;
        backup(&store, &manifest.registration, registration.as_deref())?;
        backup(&store, &manifest.usage, usage.as_deref())?;
        let bytes = canonical(&manifest)?;
        durable::publish(
            &store,
            &manifest_path(&store, manifest.request.request_id)?,
            &bytes,
        )?;
        apply(&store, &manifest, &usage_path)?;
    }
    response(&manifest, dry_run)
}
fn resume(
    store: &Store,
    request: &RecoveryRequest,
    registration: Option<&[u8]>,
    usage: Option<&[u8]>,
    dry_run: bool,
) -> Result<Option<Value>> {
    let path = manifest_path(store, request.request_id)?;
    let Some(bytes) = durable::read(&store.root, &path, super::MAX_RECORD as u64)? else {
        return Ok(None);
    };
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
    if manifest.request != *request {
        return Err(conflict());
    }
    validate_manifest(&manifest)?;
    if canonical(&manifest)? != bytes {
        return Err(corrupt());
    }
    validate_current(registration, &manifest.registration)?;
    validate_current(usage, &manifest.usage)?;
    verify_copy(store, &manifest.registration)?;
    verify_copy(store, &manifest.usage)?;
    validate_prefixes(registration, usage, request.repository_id)?;
    super::receipts::verify_evidence(
        store,
        prefix(registration.unwrap_or_default()),
        prefix(usage.unwrap_or_default()),
    )?;
    if !dry_run {
        apply(store, &manifest, &observations::path(store))?;
    }
    response(&manifest, dry_run).map(Some)
}
fn validate_manifest(manifest: &Manifest) -> Result<()> {
    if manifest.schema_version != 1 || manifest.kind != "torn_tail_recovery" {
        return Err(corrupt());
    }
    parse(&canonical(&manifest.request)?).map_err(|_| corrupt())?;
    for (plan, expected) in [
        (
            &manifest.registration,
            &manifest.request.payload.registration_digest,
        ),
        (&manifest.usage, &manifest.request.payload.usage_digest),
    ] {
        if &plan.original_digest != expected
            || !super::identity::digest(&plan.prefix_digest)
            || plan.original_bytes > durable::MAX_LOG
            || plan.prefix_bytes > plan.original_bytes
            || plan.discarded_tail_bytes != plan.original_bytes - plan.prefix_bytes
            || (!plan.existed && plan.original_bytes != 0)
        {
            return Err(corrupt());
        }
    }
    Ok(())
}
fn validate_current(bytes: Option<&[u8]>, plan: &LogPlan) -> Result<()> {
    if plan.existed && bytes.is_none() {
        return Err(conflict());
    }
    let current = bytes.unwrap_or_default();
    if hash(current) == plan.original_digest && current.len() as u64 == plan.original_bytes {
        return Ok(());
    }
    let prefix_len = usize::try_from(plan.prefix_bytes).map_err(|_| corrupt())?;
    // A completed recovery can be retried after later valid work, without truncating it.
    if current.len() < prefix_len
        || hash(&current[..prefix_len]) != plan.prefix_digest
        || (!current.is_empty() && current.last() != Some(&b'\n'))
    {
        return Err(conflict());
    }
    Ok(())
}

fn validate_prefixes(
    registration: Option<&[u8]>,
    usage: Option<&[u8]>,
    repository: Uuid,
) -> Result<()> {
    let ledger = super::journal::decode(prefix(registration.unwrap_or_default()))?;
    if ledger.repository_id != Some(repository) {
        return Err(conflict());
    }
    observations::decode(prefix(usage.unwrap_or_default()), &ledger)?;
    Ok(())
}
fn copy_path(store: &Store, plan: &LogPlan) -> Result<PathBuf> {
    Ok(directory_path(store)?.join(format!(
        "{}.jsonl",
        plan.original_digest
            .strip_prefix("sha256:")
            .ok_or_else(corrupt)?
    )))
}
fn backup(store: &Store, plan: &LogPlan, bytes: Option<&[u8]>) -> Result<()> {
    if plan.discarded_tail_bytes > 0 {
        durable::publish(store, &copy_path(store, plan)?, bytes.ok_or_else(corrupt)?)?;
    }
    Ok(())
}
fn verify_copy(store: &Store, plan: &LogPlan) -> Result<()> {
    if plan.discarded_tail_bytes > 0 {
        let bytes = durable::read(&store.root, &copy_path(store, plan)?, durable::MAX_LOG)?
            .ok_or_else(corrupt)?;
        if hash(&bytes) != plan.original_digest
            || bytes.len() as u64 != plan.original_bytes
            || hash(prefix(&bytes)) != plan.prefix_digest
            || prefix(&bytes).len() as u64 != plan.prefix_bytes
        {
            return Err(corrupt());
        }
    }
    Ok(())
}
fn apply(store: &Store, manifest: &Manifest, usage_path: &Path) -> Result<()> {
    for (path, plan) in [
        (store.log.as_path(), &manifest.registration),
        (usage_path, &manifest.usage),
    ] {
        if plan.discarded_tail_bytes > 0 {
            durable::boundary("recovery_truncate")?;
            let file = options().write(true).open(path).map_err(io_error)?;
            let current =
                durable::read(&store.root, path, durable::MAX_LOG)?.ok_or_else(corrupt)?;
            if hash(&current) == plan.original_digest {
                file.set_len(plan.prefix_bytes).map_err(io_error)?;
            }
            durable::boundary("recovery_flush")?;
            file.sync_all().map_err(io_error)?;
        }
    }
    durable::boundary("recovery_directory_sync")?;
    sync_dir(&store.dir)
}
