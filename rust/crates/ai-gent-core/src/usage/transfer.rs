//! Portable, sanitized evidence archives are isolated from the local execution ledger.
use super::identity::{Ledger, Operation, Registration, Request, Run, State};
use super::journal::{Store, canonical, check_path, directory, hash, ignore_usage};
use super::normalize::Observation;
use super::{Error, MAX_RECORD, Result, conflict, corrupt, durable, invalid, io_error, limit};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use uuid::Uuid;
const MAX_ENTRIES: usize = 4096;
const MAX_ROWS: usize = 2048;
const MAX_BUNDLE: usize = MAX_RECORD - 4096;
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    UsageEvidenceBundle,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Quality {
    Imported,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Assurance {
    LocalUnverified,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Coverage {
    Unclosed,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    schema_version: u32,
    kind: Kind,
    repository_id: Uuid,
    quality: Quality,
    assurance: Assurance,
    coverage: Coverage,
    runs: Vec<Run>,
    observations: Vec<Observation>,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportRequest {
    schema_version: u32,
    request_id: Uuid,
    repository_id: Uuid,
    operation: String,
    payload: ImportPayload,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ImportPayload {
    bundle_digest: String,
    bundle: Value,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    request_id: Uuid,
    repository_id: Uuid,
    request_digest: String,
    bundle_digest: String,
}
fn pseudonym(value: &str) -> String {
    hash(value.as_bytes())
}
fn sanitize(event: &mut Observation) {
    let namespace = format!(
        "{}\0{}",
        event.source.namespace,
        event.host_session_id.as_deref().unwrap_or("")
    );
    event.host_session_id = None;
    event.source.namespace = pseudonym(&namespace);
    event.source.adapter = pseudonym(&event.source.adapter);
    event.source.adapter_version = pseudonym(&event.source.adapter_version);
    event.source.request_id = event.source.request_id.as_ref().map(|id| pseudonym(id));
    event.source.event_id = pseudonym(&event.source.event_id);
    event.source.reference = format!(
        "evidence:sha256/{}",
        pseudonym(&event.source.reference).trim_start_matches("sha256:")
    );
    if !matches!(
        event.source.semantics.as_str(),
        "inclusive-input-v1" | "anthropic-disjoint-input-v1"
    ) {
        event.source.semantics = pseudonym(&event.source.semantics);
    }
    event.raw_usage = std::mem::take(&mut event.raw_usage)
        .into_iter()
        .map(|(key, value)| {
            let key = if matches!(
                key.as_str(),
                "input_tokens"
                    | "output_tokens"
                    | "total_tokens"
                    | "cached_input_tokens"
                    | "cache_read_input_tokens"
                    | "cache_creation_input_tokens"
                    | "reasoning_output_tokens"
            ) {
                key
            } else {
                pseudonym(&key)
            };
            (key, value)
        })
        .collect();
}
fn validate(bundle: &Bundle) -> Result<()> {
    if bundle.schema_version != 1 {
        return Err(Error::new(
            "unsupported_version",
            "only transfer bundle version 1 is supported",
            2,
        ));
    }
    if bundle.repository_id.is_nil()
        || bundle.runs.is_empty()
        || bundle.runs.len() > MAX_ROWS
        || bundle.observations.len() > MAX_ROWS
    {
        return Err(invalid());
    }
    let mut ledger = Ledger {
        repository_id: Some(bundle.repository_id),
        ..Ledger::default()
    };
    let mut segments = BTreeSet::new();
    let mut attempts = BTreeMap::new();
    let mut ordinals = BTreeMap::new();
    for run in &bundle.runs {
        if ledger.runs.contains_key(&run.run_id) {
            return Err(invalid());
        }
        validate_run(run, bundle.repository_id, &mut segments)?;
        let identity = (
            &run.task,
            run.attempt_ordinal,
            run.parent_attempt_id,
            &run.task_fingerprint,
        );
        if attempts
            .insert(run.attempt_id, identity)
            .is_some_and(|prior| prior != identity)
            || ordinals
                .insert((&run.task, run.attempt_ordinal), run.attempt_id)
                .is_some_and(|prior| prior != run.attempt_id)
        {
            return Err(conflict());
        }
        ledger.runs.insert(run.run_id, run.clone());
    }
    for run in &bundle.runs {
        if let Some(parent) = run.parent_run_id {
            let parent = ledger.runs.get(&parent).ok_or_else(invalid)?;
            if parent.task != run.task && !parent.scope().beneficiaries.contains(&run.task) {
                return Err(conflict());
            }
        }
        if let Some(parent) = run.parent_attempt_id {
            let parent = attempts.get(&parent).ok_or_else(invalid)?;
            if parent.0 != &run.task || parent.1 >= run.attempt_ordinal {
                return Err(conflict());
            }
        }
        let mut seen = BTreeSet::new();
        let mut current = Some(run.run_id);
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(conflict());
            }
            current = ledger.runs.get(&id).ok_or_else(invalid)?.parent_run_id;
        }
        let mut seen = BTreeSet::new();
        let mut current = Some(run.attempt_id);
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(conflict());
            }
            current = attempts.get(&id).ok_or_else(invalid)?.2;
        }
    }
    for event in &bundle.observations {
        validate_privacy(event)?;
        event.normalize()?;
        event.bind(&ledger, bundle.repository_id)?;
    }
    super::contributions::summarize(&bundle.observations)?;
    Ok(())
}
fn validate_run(run: &Run, repository_id: Uuid, segments: &mut BTreeSet<Uuid>) -> Result<()> {
    let segment_generation =
        u64::try_from(run.segments.len().saturating_sub(1)).map_err(|_| invalid())?;
    let state_generation = run
        .generation
        .checked_sub(segment_generation)
        .ok_or_else(invalid)?;
    let generation_matches = match run.state {
        State::Registered => state_generation == 0,
        State::Running => state_generation == 1,
        State::Finished => state_generation == 2,
        State::Failed | State::Cancelled => matches!(state_generation, 1 | 2),
    };
    if !generation_matches
        || run.execution_scope.is_none()
        || run.host_session_id.is_some()
        || run.run_id.is_nil()
        || run.attempt_id.is_nil()
        || run.attempt_ordinal == 0
        || run.attempt_ordinal > 9_007_199_254_740_991
        || run.generation > 9_007_199_254_740_991
        || run.segments.is_empty()
        || run.segments.len() > MAX_ROWS
        || run.usage_quality != "missing"
        || !super::identity::digest(&run.task_fingerprint)
    {
        return Err(invalid());
    }
    let request = Request {
        schema_version: 1,
        request_id: run.run_id,
        repository_id,
        operation: Operation::Register(Registration {
            task: run.task.clone(),
            execution_scope: run.execution_scope.clone(),
            host_session_id: None,
            phase: run.phase,
            role: run.role,
            purpose: run.purpose,
            configuration: run.segments[0].configuration.clone(),
            attempt_id: None,
            parent_attempt_id: run.parent_attempt_id,
            parent_run_id: run.parent_run_id,
            run_id: Some(run.run_id),
        }),
    };
    request.validate("register")?;
    if run
        .segments
        .windows(2)
        .any(|pair| pair[0].configuration == pair[1].configuration)
    {
        return Err(conflict());
    }
    for segment in &run.segments {
        segment.configuration.validate()?;
        let quality = if segment.configuration.effective_provider.is_some()
            && segment.configuration.effective_model.is_some()
        {
            "reported"
        } else {
            "unknown"
        };
        if segment.segment_id.is_nil()
            || !segments.insert(segment.segment_id)
            || segment.identity_quality != quality
        {
            return Err(invalid());
        }
    }
    Ok(())
}
fn validate_privacy(event: &Observation) -> Result<()> {
    if event.host_session_id.is_some()
        || !super::identity::digest(&event.source.namespace)
        || !super::identity::digest(&event.source.adapter)
        || !super::identity::digest(&event.source.adapter_version)
        || !super::identity::digest(&event.source.event_id)
        || event
            .source
            .request_id
            .as_ref()
            .is_some_and(|id| !super::identity::digest(id))
        || !event
            .source
            .reference
            .strip_prefix("evidence:sha256/")
            .is_some_and(|s| super::identity::digest(&format!("sha256:{s}")))
    {
        return Err(invalid());
    }
    if !matches!(
        event.source.semantics.as_str(),
        "inclusive-input-v1" | "anthropic-disjoint-input-v1"
    ) && !super::identity::digest(&event.source.semantics)
    {
        return Err(invalid());
    }
    if event.raw_usage.keys().any(|key| {
        !matches!(
            key.as_str(),
            "input_tokens"
                | "output_tokens"
                | "total_tokens"
                | "cached_input_tokens"
                | "cache_read_input_tokens"
                | "cache_creation_input_tokens"
                | "reasoning_output_tokens"
        ) && !super::identity::digest(key)
    }) {
        return Err(invalid());
    }
    Ok(())
}
fn bundle_bytes(bundle: &Bundle) -> Result<Vec<u8>> {
    validate(bundle)?;
    let bytes = canonical(bundle)?;
    if bytes.len() > MAX_BUNDLE {
        return Err(limit());
    }
    Ok(bytes)
}
fn decode(value: &Value) -> Result<Bundle> {
    let bundle: Bundle = serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    if canonical(&bundle)? != canonical(value)? {
        return Err(invalid());
    }
    validate(&bundle)?;
    Ok(bundle)
}
fn dir(store: &Store) -> Result<PathBuf> {
    Ok(store.dir.parent().ok_or_else(invalid)?.join("imports"))
}
fn bundle_path(store: &Store, digest: &str) -> Result<PathBuf> {
    Ok(dir(store)?.join(format!(
        "{}.json",
        digest.strip_prefix("sha256:").ok_or_else(invalid)?
    )))
}
fn read_archives(store: &Store) -> Result<BTreeMap<Uuid, Manifest>> {
    let path = dir(store)?;
    check_path(&store.root, &path, true)?;
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(io_error(error)),
    };
    let mut manifests = BTreeMap::new();
    let mut bundles = BTreeMap::new();
    let mut total = 0u64;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ENTRIES {
            return Err(limit());
        }
        let entry = entry.map_err(io_error)?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(corrupt)?;
        if name.starts_with(".pending-") {
            continue;
        }
        let bytes =
            durable::read(&store.root, &entry.path(), MAX_RECORD as u64)?.ok_or_else(corrupt)?;
        total += bytes.len() as u64;
        if total > durable::MAX_LOG {
            return Err(limit());
        }
        if let Some(id) = name
            .strip_prefix("import-")
            .and_then(|name| name.strip_suffix(".json"))
        {
            let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
            if manifest.schema_version != 1
                || manifest.request_id.is_nil()
                || manifest.repository_id.is_nil()
                || manifest.request_id.to_string() != id
                || !super::identity::digest(&manifest.request_digest)
                || !super::identity::digest(&manifest.bundle_digest)
                || canonical(&manifest)? != bytes
                || manifests.insert(manifest.request_id, manifest).is_some()
            {
                return Err(corrupt());
            }
        } else {
            let digest = format!("sha256:{}", name.strip_suffix(".json").ok_or_else(corrupt)?);
            if !super::identity::digest(&digest) || hash(&bytes) != digest {
                return Err(corrupt());
            }
            let value: Value = serde_json::from_slice(&bytes).map_err(|_| corrupt())?;
            let bundle = decode(&value).map_err(|_| corrupt())?;
            if canonical(&bundle)? != bytes {
                return Err(corrupt());
            }
            bundles.insert(
                digest,
                (
                    bundle.repository_id,
                    serde_json::to_value(bundle).map_err(|_| corrupt())?,
                ),
            );
        }
    }
    for manifest in manifests.values() {
        let (repository_id, bundle) = bundles.get(&manifest.bundle_digest).ok_or_else(corrupt)?;
        let request = ImportRequest {
            schema_version: 1,
            request_id: manifest.request_id,
            repository_id: manifest.repository_id,
            operation: "import".into(),
            payload: ImportPayload {
                bundle_digest: manifest.bundle_digest.clone(),
                bundle: bundle.clone(),
            },
        };
        if repository_id != &manifest.repository_id
            || hash(&canonical(&request)?) != manifest.request_digest
        {
            return Err(corrupt());
        }
    }
    Ok(manifests)
}
pub(super) fn check_request_id(store: &Store, id: Uuid) -> Result<()> {
    if read_archives(store)?.contains_key(&id) {
        return Err(conflict());
    }
    Ok(())
}
pub(super) fn export(root: &Path) -> Result<Value> {
    let store = Store::open(root, false)?;
    if !store.log.exists() {
        return Err(Error::new(
            "evidence_missing",
            "export requires registered local evidence",
            3,
        ));
    }
    let _lock = store.lock(false)?;
    let ledger = store.load()?;
    let records = super::observations::read(&store, &ledger)?;
    super::receipts::verify_current(&store)?;
    let mut runs: Vec<_> = ledger.runs.values().cloned().collect();
    for run in &mut runs {
        run.host_session_id = None;
        run.execution_scope = Some(run.scope());
    }
    let observations = records
        .into_iter()
        .map(|r| {
            let mut event = r.payload;
            sanitize(&mut event);
            event
        })
        .collect();
    let bundle = Bundle {
        schema_version: 1,
        kind: Kind::UsageEvidenceBundle,
        repository_id: ledger.repository_id.ok_or_else(invalid)?,
        quality: Quality::Imported,
        assurance: Assurance::LocalUnverified,
        coverage: Coverage::Unclosed,
        runs,
        observations,
    };
    let bytes = bundle_bytes(&bundle)?;
    Ok(
        json!({"schema_version":1,"operation":"export","ok":true,"result":{"bundle_digest":hash(&bytes),"bundle":bundle},"warnings":["Portable evidence is imported/unverified and does not prove provider usage or completion."]}),
    )
}
pub(super) fn import(root: &Path, input: &[u8], dry_run: bool) -> Result<Value> {
    durable::record_limit(input)?;
    let value: Value = serde_json::from_slice(input).map_err(|_| invalid())?;
    let request: ImportRequest = serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    if request.schema_version != 1 {
        return Err(Error::new(
            "unsupported_version",
            "only import version 1 is supported",
            2,
        ));
    }
    if request.request_id.is_nil()
        || request.repository_id.is_nil()
        || request.operation != "import"
        || !super::identity::digest(&request.payload.bundle_digest)
        || canonical(&request)? != canonical(&value)?
    {
        return Err(invalid());
    }
    let bundle = decode(&request.payload.bundle)?;
    let bytes = canonical(&bundle)?;
    if bytes.len() > MAX_BUNDLE {
        return Err(limit());
    }
    if request.repository_id != bundle.repository_id
        || hash(&bytes) != request.payload.bundle_digest
    {
        return Err(conflict());
    }
    let preview = Store::open(root, false)?;
    check_path(&preview.root, &dir(&preview)?, true)?;
    let preview_lock = if preview.dir.join("registrations.lock").exists() {
        Some(preview.lock(false)?)
    } else {
        None
    };
    let request_digest = hash(&canonical(&request)?);
    check_target(&preview, &request, &request_digest)?;
    let summary = super::contributions::summarize(&bundle.observations)?;
    let result = json!({"schema_version":1,"operation":"import","ok":true,"result":{"dry_run":dry_run,"bundle_digest":request.payload.bundle_digest,"source_repository_id":bundle.repository_id,"quality":"imported","assurance":"local_unverified","coverage":"unclosed","contributes_to_local_totals":false,"run_count":bundle.runs.len(),"observation_count":summary.observation_count,"summary":summary},"warnings":["Imported archives are isolated from current execution records and do not certify source identity or billing."]});
    if dry_run {
        return Ok(result);
    }
    drop(preview_lock);
    let store = Store::open(root, true)?;
    let _lock = store.lock(true)?;
    let manifests = check_target(&store, &request, &request_digest)?;
    let manifest = Manifest {
        schema_version: 1,
        request_id: request.request_id,
        repository_id: request.repository_id,
        request_digest,
        bundle_digest: request.payload.bundle_digest.clone(),
    };
    let manifest_bytes = canonical(&manifest)?;
    let archive_path = bundle_path(&store, &request.payload.bundle_digest)?;
    let manifest_path = dir(&store)?.join(format!("import-{}.json", request.request_id));
    let additions = [
        (&archive_path, bytes.len()),
        (&manifest_path, manifest_bytes.len()),
    ];
    let additional_count = additions.iter().filter(|(path, _)| !path.exists()).count();
    let additional_bytes: u64 = additions
        .iter()
        .filter(|(path, _)| !path.exists())
        .map(|(_, length)| *length as u64)
        .sum();
    let (mut entry_count, mut existing_bytes) = (0usize, 0u64);
    match std::fs::read_dir(dir(&store)?) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(io_error)?;
                entry_count += 1;
                existing_bytes =
                    existing_bytes.saturating_add(entry.metadata().map_err(io_error)?.len());
                if entry_count > MAX_ENTRIES || existing_bytes > durable::MAX_LOG {
                    return Err(limit());
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(error)),
    }
    if entry_count + additional_count > MAX_ENTRIES
        || existing_bytes.saturating_add(additional_bytes) > durable::MAX_LOG
    {
        return Err(limit());
    }
    ignore_usage(&store.root, store.dir.parent().ok_or_else(invalid)?)?;
    directory(&store.root, &dir(&store)?)?;
    if manifests.len() >= MAX_ENTRIES / 2 && !manifests.contains_key(&request.request_id) {
        return Err(limit());
    }
    durable::publish(
        &store,
        &bundle_path(&store, &request.payload.bundle_digest)?,
        &bytes,
    )?;
    durable::boundary("import_manifest_publication")?;
    durable::publish(&store, &manifest_path, &manifest_bytes)?;
    Ok(result)
}

fn check_target(
    store: &Store,
    request: &ImportRequest,
    request_digest: &str,
) -> Result<BTreeMap<Uuid, Manifest>> {
    let ledger = store.load()?;
    let records = super::observations::read(store, &ledger)?;
    super::receipts::verify_current(store)?;
    if ledger.requests.contains_key(&request.request_id)
        || records.iter().any(|r| r.request_id == request.request_id)
    {
        return Err(conflict());
    }
    super::receipts::check_request_id(store, request.request_id)?;
    super::recovery::check_request_id(store, request.request_id)?;
    let manifests = read_archives(store)?;
    if manifests
        .get(&request.request_id)
        .is_some_and(|m| m.request_digest != request_digest)
    {
        return Err(conflict());
    }
    Ok(manifests)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> (tempfile::TempDir, Vec<u8>) {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("specs/046-test")).unwrap();
        std::fs::write(
            root.path().join("specs/046-test/tasks.md"),
            "- [ ] T001 Build\n",
        )
        .unwrap();
        let configuration = json!({"agent_app":"codex","agent_version":"fixture","requested_provider":"openai","requested_model":"fixture","effective_provider":null,"effective_model":null,"effort":null,"config_digest":format!("sha256:{}","a".repeat(64)),"model_vendor":null,"service_tier":null,"provider_region":null});
        let registration = json!({"schema_version":1,"request_id":Uuid::new_v4(),"repository_id":"20000000-0000-4000-8000-000000000001","operation":"register","payload":{"task":{"repository_id":"20000000-0000-4000-8000-000000000001","feature_id":"046-test","task_id":"T001"},"host_session_id":null,"phase":"builder","role":"builder","purpose":"implementation","configuration":configuration,"attempt_id":null,"parent_attempt_id":null,"parent_run_id":null,"run_id":null}});
        super::super::execute(
            root.path(),
            "register",
            &serde_json::to_vec(&registration).unwrap(),
        )
        .unwrap();
        let exported = export(root.path()).unwrap();
        let request = json!({"schema_version":1,"request_id":Uuid::new_v4(),"repository_id":registration["repository_id"],"operation":"import","payload":{"bundle_digest":exported["result"]["bundle_digest"],"bundle":exported["result"]["bundle"]}});
        (root, serde_json::to_vec(&request).unwrap())
    }
    #[test]
    fn import_failure_between_bundle_and_manifest_is_retryable_without_false_ack() {
        let (_source, request) = request();
        let root = tempfile::tempdir().unwrap();
        durable::fail_once("import_manifest_publication");
        assert_eq!(
            import(root.path(), &request, false)
                .unwrap_err()
                .exit_code(),
            5
        );
        assert_eq!(
            std::fs::read_dir(root.path().join(".ai-gent/usage/imports"))
                .unwrap()
                .count(),
            1
        );
        assert!(import(root.path(), &request, false).is_ok());
        assert_eq!(
            std::fs::read_dir(root.path().join(".ai-gent/usage/imports"))
                .unwrap()
                .count(),
            2
        );
    }
    #[test]
    fn import_metadata_validation_rejects_untrusted_run_claims_and_cycles() {
        let (_source, request) = request();
        let input: Value = serde_json::from_slice(&request).unwrap();
        for field in [
            "usage_quality",
            "role",
            "parent_run_id",
            "parent_attempt_id",
            "generation",
        ] {
            let mut bundle = input["payload"]["bundle"].clone();
            bundle["runs"][0][field] = match field {
                "role" => json!("reviewer"),
                "parent_run_id" => bundle["runs"][0]["run_id"].clone(),
                "parent_attempt_id" => bundle["runs"][0]["attempt_id"].clone(),
                "generation" => json!(50),
                _ => json!("certified"),
            };
            assert!(decode(&bundle).is_err(), "{field}");
        }
    }
    #[test]
    fn maximum_exportable_bundle_keeps_import_envelope_headroom() {
        let (_source, input) = request();
        let request: Value = serde_json::from_slice(&input).unwrap();
        let mut bundle = decode(&request["payload"]["bundle"]).unwrap();
        let mut template = bundle.runs[0].clone();
        let label = "a".repeat(256);
        let config = &mut template.segments[0].configuration;
        config.agent_app = label.clone();
        config.agent_version = label.clone();
        config.requested_provider = Some(label.clone());
        config.requested_model = Some(label.clone());
        config.effective_provider = Some(label.clone());
        config.effective_model = Some(label.clone());
        config.effort = Some(label.clone());
        config.model_vendor = Some(label.clone());
        config.service_tier = Some(label.clone());
        config.provider_region = Some(label);
        template.segments[0].identity_quality = "reported".into();
        bundle.runs.clear();
        loop {
            let mut run = template.clone();
            run.run_id = Uuid::new_v4();
            run.attempt_id = Uuid::new_v4();
            run.segments[0].segment_id = Uuid::new_v4();
            run.attempt_ordinal = u64::try_from(bundle.runs.len() + 1).unwrap();
            bundle.runs.push(run);
            if canonical(&bundle).unwrap().len() > MAX_BUNDLE {
                break;
            }
        }
        assert_eq!(bundle_bytes(&bundle).unwrap_err().exit_code(), 2);
        bundle.runs.pop();
        let bytes = bundle_bytes(&bundle).unwrap();
        let input = json!({"schema_version":1,"request_id":Uuid::new_v4(),"repository_id":bundle.repository_id,"operation":"import","payload":{"bundle_digest":hash(&bytes),"bundle":bundle}});
        let request_bytes = serde_json::to_vec(&input).unwrap();
        assert!(request_bytes.len() <= MAX_RECORD);
        let target = tempfile::tempdir().unwrap();
        assert!(import(target.path(), &request_bytes, true).is_ok());
        assert!(!target.path().join(".ai-gent").exists());
    }
    #[test]
    fn import_rejects_native_segment_and_attempt_order_bypasses() {
        let (_source, input) = request();
        let request: Value = serde_json::from_slice(&input).unwrap();
        let mut bundle = decode(&request["payload"]["bundle"]).unwrap();
        let mut duplicate = bundle.runs[0].segments[0].clone();
        duplicate.segment_id = Uuid::new_v4();
        bundle.runs[0].segments.push(duplicate);
        bundle.runs[0].generation = 1;
        assert!(
            validate(&bundle).is_err(),
            "unchanged segment configuration must be denied"
        );
        let mut bundle = decode(&request["payload"]["bundle"]).unwrap();
        let mut parent = bundle.runs[0].clone();
        parent.run_id = Uuid::new_v4();
        parent.attempt_id = Uuid::new_v4();
        parent.attempt_ordinal = 2;
        parent.segments[0].segment_id = Uuid::new_v4();
        bundle.runs[0].parent_attempt_id = Some(parent.attempt_id);
        bundle.runs.push(parent);
        assert!(
            validate(&bundle).is_err(),
            "attempt parent must precede child ordinal"
        );
    }
}
