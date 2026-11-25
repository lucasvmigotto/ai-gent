//! Test-only prototype for proposed storage-v2 metadata, not a source verifier.
//!
//! This module is compiled only by `cfg(test)`. It never opens files, enables a
//! migration, validates source bytes or establishes provider authenticity.
use super::identity::digest;
use super::journal::{canonical, hash};
use super::{Result, corrupt, limit};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::BTreeSet;
use uuid::Uuid;

const MAX_HEAD: usize = 4 * 1024;
const MAX_MANIFEST: usize = 128 * 1024;
const MAX_SEGMENT: u64 = 8 * 1024 * 1024;
const MAX_LEGACY: u64 = 64 * 1024 * 1024;
const MAX_STREAM: u64 = 256 * 1024 * 1024;
const MAX_BOTH: u64 = 512 * 1024 * 1024;
const MAX_FRAMES: u64 = 131_072;
const MAX_CLOSED: usize = 128;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Head {
    storage_schema_version: u32,
    generation: u64,
    manifest_digest: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    storage_schema_version: u32,
    kind: String,
    repository_id: Uuid,
    generation: u64,
    predecessor_manifest_digest: Option<String>,
    streams: Streams,
    migration_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Streams {
    registrations: Stream,
    observations: Stream,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stream {
    sequence: u64,
    closed: Vec<Closed>,
    active: Active,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Closed {
    object_id: Uuid,
    first_sequence: u64,
    last_sequence: u64,
    byte_length: u64,
    raw_sha256: String,
    frame_count: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Active {
    object_id: Uuid,
    first_sequence: u64,
    committed_sequence: u64,
    committed_bytes: u64,
    committed_raw_sha256: String,
}

/// Parse bounded canonical metadata and check internal consistency only.
///
/// The returned metadata is not proof of object contents, predecessor linkage,
/// migration provenance, file safety, durability or model/provider identity.
fn parse_metadata(
    head_bytes: &[u8],
    manifest_bytes: &[u8],
    repository_id: Uuid,
) -> Result<Manifest> {
    if head_bytes.len() > MAX_HEAD || manifest_bytes.len() > MAX_MANIFEST {
        return Err(limit());
    }
    let head: Head = parse_canonical(head_bytes)?;
    let manifest: Manifest = parse_canonical(manifest_bytes)?;
    if head.storage_schema_version != 2
        || manifest.storage_schema_version != 2
        || manifest.kind != "segmented_usage_journal"
        || repository_id.is_nil()
        || manifest.repository_id != repository_id
        || manifest.generation == 0
        || head.generation != manifest.generation
        || !digest(&head.manifest_digest)
        || head.manifest_digest != hash(manifest_bytes)
        || manifest.migration_id.is_some_and(|id| id.is_nil())
    {
        return Err(corrupt());
    }
    match &manifest.predecessor_manifest_digest {
        None if manifest.generation == 1 => {}
        Some(prior) if manifest.generation > 1 && digest(prior) => {}
        _ => return Err(corrupt()),
    }
    let mut objects = BTreeSet::new();
    let migrated = manifest.migration_id.is_some();
    let registrations = manifest
        .streams
        .registrations
        .validate(&mut objects, migrated)?;
    let observations = manifest
        .streams
        .observations
        .validate(&mut objects, migrated)?;
    if registrations.checked_add(observations).ok_or_else(limit)? > MAX_BOTH {
        return Err(limit());
    }
    Ok(manifest)
}

fn parse_canonical<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    let parsed: T = serde_json::from_slice(bytes).map_err(|_| corrupt())?;
    if canonical(&parsed)? != bytes {
        return Err(corrupt());
    }
    Ok(parsed)
}

fn insert_object(objects: &mut BTreeSet<Uuid>, object: Uuid) -> Result<()> {
    if object.is_nil() || !objects.insert(object) {
        return Err(corrupt());
    }
    Ok(())
}

impl Stream {
    fn validate(&self, objects: &mut BTreeSet<Uuid>, migrated: bool) -> Result<u64> {
        if self.closed.len() > MAX_CLOSED || self.sequence > MAX_FRAMES {
            return Err(limit());
        }
        let mut previous = 0_u64;
        let mut bytes = 0_u64;
        for (index, object) in self.closed.iter().enumerate() {
            insert_object(objects, object.object_id)?;
            let count = object
                .last_sequence
                .checked_sub(object.first_sequence)
                .and_then(|length| length.checked_add(1))
                .ok_or_else(corrupt)?;
            if object.first_sequence != previous.checked_add(1).ok_or_else(corrupt)?
                || count != object.frame_count
                || object.byte_length == 0
                || object.byte_length < object.frame_count
                || !digest(&object.raw_sha256)
            {
                return Err(corrupt());
            }
            let bound = if migrated && index == 0 {
                MAX_LEGACY
            } else {
                MAX_SEGMENT
            };
            if object.byte_length > bound {
                return Err(limit());
            }
            bytes = bytes.checked_add(object.byte_length).ok_or_else(limit)?;
            if bytes > MAX_STREAM {
                return Err(limit());
            }
            previous = object.last_sequence;
        }
        insert_object(objects, self.active.object_id)?;
        if self.active.first_sequence != previous.checked_add(1).ok_or_else(corrupt)?
            || self.active.committed_sequence != self.sequence
            || !digest(&self.active.committed_raw_sha256)
        {
            return Err(corrupt());
        }
        if self.active.committed_bytes == 0 {
            if self.sequence != previous || self.active.committed_raw_sha256 != hash(b"") {
                return Err(corrupt());
            }
        } else {
            let count = self.sequence.checked_sub(previous).ok_or_else(corrupt)?;
            if count == 0 || self.active.committed_bytes < count {
                return Err(corrupt());
            }
        }
        if self.active.committed_bytes > MAX_SEGMENT {
            return Err(limit());
        }
        bytes = bytes
            .checked_add(self.active.committed_bytes)
            .ok_or_else(limit)?;
        if bytes > MAX_STREAM {
            return Err(limit());
        }
        Ok(bytes)
    }
}

#[path = "segmented_tests.rs"]
mod tests;
