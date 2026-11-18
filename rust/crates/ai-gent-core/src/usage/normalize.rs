//! Source arithmetic and conservative selection of disjoint contributions.
pub(super) use super::identity::{Attribution, ExecutionScope};
use super::identity::{Configuration, Ledger, Phase, TaskKey};
use super::{Error, Result, conflict, invalid};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

const MAX_COUNT: u64 = 9_007_199_254_740_991;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Counters {
    pub input_total: Option<u64>,
    pub cached_input: Option<u64>,
    pub cache_creation_input: Option<u64>,
    pub output_total: Option<u64>,
    pub reasoning_output: Option<u64>,
}
impl Counters {
    pub(super) fn values(self) -> [Option<u64>; 5] {
        [
            self.input_total,
            self.cached_input,
            self.cache_creation_input,
            self.output_total,
            self.reasoning_output,
        ]
    }
    pub(super) fn from_values(v: [Option<u64>; 5]) -> Self {
        Self {
            input_total: v[0],
            cached_input: v[1],
            cache_creation_input: v[2],
            output_total: v[3],
            reasoning_output: v[4],
        }
    }
    fn validate(self) -> Result<()> {
        if self.values().iter().flatten().any(|v| *v > MAX_COUNT) {
            return Err(counter_error());
        }
        for (part, total) in [
            (self.cached_input, self.input_total),
            (self.cache_creation_input, self.input_total),
            (self.reasoning_output, self.output_total),
        ] {
            if matches!((part,total),(Some(p),Some(t)) if p>t) {
                return Err(counter_error());
            }
        }
        if let (Some(cached), Some(created), Some(total)) = (
            self.cached_input,
            self.cache_creation_input,
            self.input_total,
        ) && add(cached, created)? > total
        {
            return Err(counter_error());
        }
        let minimum_input = self.input_total.unwrap_or(0).max(add(
            self.cached_input.unwrap_or(0),
            self.cache_creation_input.unwrap_or(0),
        )?);
        let minimum_output = self
            .output_total
            .unwrap_or(0)
            .max(self.reasoning_output.unwrap_or(0));
        add(minimum_input, minimum_output)?;
        Ok(())
    }
    pub(super) fn total(self) -> Result<Option<u64>> {
        match (self.input_total, self.output_total) {
            (Some(a), Some(b)) => Ok(Some(add(a, b)?)),
            _ => Ok(None),
        }
    }
}
pub(super) fn add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .filter(|sum| *sum <= MAX_COUNT)
        .ok_or_else(counter_error)
}
fn counter_error() -> Error {
    Error::new(
        "invalid_schema",
        "usage counters or declared normalization are inconsistent",
        2,
    )
}
pub(super) fn coverage_error() -> Error {
    Error::new(
        "policy_denied",
        "usage coverage regresses or overlaps ambiguously",
        3,
    )
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Quality {
    Complete,
    Partial,
    Imported,
    Missing,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Attestation {
    Local,
    ProviderReconciled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Contribution {
    Authoritative,
    Observation,
    Superseded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Scope {
    RequestDelta,
    TurnTotal,
    SessionCumulative,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    pub agent_app: String,
    pub agent_version: String,
    pub requested_provider: Option<String>,
    pub requested_model: Option<String>,
    pub effective_provider: Option<String>,
    pub effective_model: Option<String>,
    pub effort: Option<String>,
    #[serde(rename = "identity_quality")]
    pub quality: IdentityQuality,
    pub config_digest: String,
    pub model_vendor: Option<String>,
    pub service_tier: Option<String>,
    pub provider_region: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum IdentityQuality {
    Verified,
    Reported,
    Unknown,
}
impl Identity {
    fn configuration(&self) -> Configuration {
        Configuration {
            agent_app: self.agent_app.clone(),
            agent_version: self.agent_version.clone(),
            requested_provider: self.requested_provider.clone(),
            requested_model: self.requested_model.clone(),
            effective_provider: self.effective_provider.clone(),
            effective_model: self.effective_model.clone(),
            effort: self.effort.clone(),
            config_digest: self.config_digest.clone(),
            model_vendor: self.model_vendor.clone(),
            service_tier: self.service_tier.clone(),
            provider_region: self.provider_region.clone(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Source {
    pub adapter: String,
    pub adapter_version: String,
    pub namespace: String,
    pub request_id: Option<String>,
    pub event_id: String,
    pub scope: Scope,
    pub semantics: String,
    pub reference: String,
    pub digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    pub schema_version: u32,
    pub event_id: Uuid,
    pub task: Option<TaskKey>,
    pub attempt_id: Uuid,
    pub run_id: Uuid,
    pub parent_run_id: Option<Uuid>,
    pub segment_id: Uuid,
    pub phase: Phase,
    pub sequence: u64,
    pub identity: Identity,
    pub source: Source,
    pub raw_usage: BTreeMap<String, Option<u64>>,
    pub counters: Counters,
    pub quality: Quality,
    pub attribution: Attribution,
    pub attestation: Attestation,
    pub contribution: Contribution,
    pub supersedes: Vec<Uuid>,
    pub extensions: BTreeMap<String, serde_json::Value>,
    pub execution_scope: ExecutionScope,
    pub observed_at: String,
    pub occurred_at: Option<String>,
    pub host_session_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IngestRequest {
    pub schema_version: u32,
    pub request_id: Uuid,
    pub repository_id: Uuid,
    pub operation: String,
    pub payload: Observation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct Normalized {
    pub counters: Counters,
    pub quality: Quality,
    pub counter_quality: Quality,
    pub known_semantics: bool,
}
fn label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 512
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:/@+=#".contains(&b))
}
fn timestamp(s: &str) -> bool {
    // Fixed UTC wire shape; date/time ranges are checked as well as separators.
    if s.len() < 20 || s.len() > 30 || !s.ends_with('Z') {
        return false;
    }
    let bytes = s.as_bytes();
    if [4, 7, 10, 13, 16].into_iter().any(|i| {
        bytes[i]
            != match i {
                4 | 7 => b'-',
                10 => b'T',
                _ => b':',
            }
    }) {
        return false;
    }
    if !(0..19)
        .filter(|i| ![4, 7, 10, 13, 16].contains(i))
        .all(|i| bytes[i].is_ascii_digit())
    {
        return false;
    }
    if s.len() > 20
        && (bytes[19] != b'.'
            || !bytes[20..bytes.len() - 1].iter().all(u8::is_ascii_digit)
            || bytes.len() == 21)
    {
        return false;
    }
    let n = |a, b| s[a..b].parse::<u32>().unwrap_or(0);
    let year = n(0, 4);
    let month = n(5, 7);
    let day = n(8, 10);
    let days = match month {
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    day >= 1 && day <= days && n(11, 13) < 24 && n(14, 16) < 60 && n(17, 19) < 60
}
impl Observation {
    fn validate_metadata(&self) -> Result<()> {
        if self.schema_version != 1 {
            return Err(Error::new(
                "unsupported_version",
                "only usage observation version 1 is supported",
                2,
            ));
        }
        if self.event_id.is_nil()
            || self.run_id.is_nil()
            || self.attempt_id.is_nil()
            || self.segment_id.is_nil()
            || self.sequence > MAX_COUNT
            || self.parent_run_id.is_some_and(|id| id.is_nil())
            || self.raw_usage.len() > 64
            || self
                .raw_usage
                .iter()
                .any(|(key, value)| !label(key) || value.is_some_and(|n| n > MAX_COUNT))
            || [
                &self.source.adapter,
                &self.source.adapter_version,
                &self.source.namespace,
                &self.source.event_id,
                &self.source.semantics,
            ]
            .iter()
            .any(|s| !label(s))
            || self.source.request_id.as_ref().is_some_and(|s| !label(s))
            || self.host_session_id.as_ref().is_some_and(|s| !label(s))
            || !super::identity::digest(&self.source.digest)
            || !self
                .source
                .reference
                .strip_prefix("evidence:")
                .is_some_and(|body| {
                    !body.is_empty()
                        && body.len() <= 512
                        && body
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"/_:.=-".contains(&b))
                })
            || !timestamp(&self.observed_at)
            || self.occurred_at.as_ref().is_some_and(|s| !timestamp(s))
            || !self.extensions.is_empty()
            || !self.supersedes.is_empty()
            || self.contribution == Contribution::Superseded
        {
            return Err(invalid());
        }
        self.identity.configuration().validate()?;
        if self.quality == Quality::Missing && self.raw_usage.values().any(Option::is_some) {
            return Err(counter_error());
        }
        Ok(())
    }
    pub fn normalize(&self) -> Result<Normalized> {
        self.validate_metadata()?;
        let raw = |key: &str| self.raw_usage.get(key).copied().flatten();
        let (counters, known) = match self.source.semantics.as_str() {
            "inclusive-input-v1" => (
                Counters {
                    input_total: raw("input_tokens"),
                    cached_input: raw("cached_input_tokens"),
                    cache_creation_input: raw("cache_creation_input_tokens"),
                    output_total: raw("output_tokens"),
                    reasoning_output: raw("reasoning_output_tokens"),
                },
                true,
            ),
            "anthropic-disjoint-input-v1" => {
                let input = match (
                    raw("input_tokens"),
                    raw("cache_read_input_tokens"),
                    raw("cache_creation_input_tokens"),
                ) {
                    (Some(a), Some(b), Some(c)) => Some(add(add(a, b)?, c)?),
                    _ => None,
                };
                (
                    Counters {
                        input_total: input,
                        cached_input: raw("cache_read_input_tokens"),
                        cache_creation_input: raw("cache_creation_input_tokens"),
                        output_total: raw("output_tokens"),
                        reasoning_output: raw("reasoning_output_tokens"),
                    },
                    true,
                )
            }
            _ => (Counters::default(), false),
        };
        counters.validate()?;
        if counters != self.counters {
            return Err(counter_error());
        }
        if let (Some(claim), Some(total)) = (raw("total_tokens"), counters.total()?)
            && claim != total
        {
            return Err(counter_error());
        }
        let math = if counters.input_total.is_some() && counters.output_total.is_some() {
            Quality::Complete
        } else if counters.values().iter().any(Option::is_some) {
            Quality::Partial
        } else {
            Quality::Missing
        };
        let quality = match (self.quality, math) {
            (Quality::Imported, _) => Quality::Imported,
            (Quality::Missing, _) | (_, Quality::Missing) => Quality::Missing,
            (Quality::Partial, _) | (_, Quality::Partial) => Quality::Partial,
            _ => Quality::Complete,
        };
        Ok(Normalized {
            counters,
            quality,
            counter_quality: math,
            known_semantics: known,
        })
    }
    pub fn bind(&self, ledger: &Ledger, repository: Uuid) -> Result<()> {
        if ledger.repository_id != Some(repository) {
            return Err(conflict());
        }
        let run = ledger
            .runs
            .get(&self.run_id)
            .ok_or_else(|| Error::new("evidence_missing", "usage run is not registered", 3))?;
        let scope = run.scope();
        let task_matches = match scope.kind {
            Attribution::Exclusive => self.task.as_ref() == Some(&run.task),
            Attribution::Shared | Attribution::Unattributed => self.task.is_none(),
        };
        if !task_matches
            || self.attempt_id != run.attempt_id
            || self.parent_run_id != run.parent_run_id
            || self.phase != run.phase
            || (run.host_session_id.is_some() && self.host_session_id != run.host_session_id)
        {
            return Err(conflict());
        }
        if self.attribution != scope.kind || self.execution_scope != scope {
            return Err(conflict());
        }
        let segment = run
            .segments
            .iter()
            .find(|s| s.segment_id == self.segment_id)
            .ok_or_else(|| Error::new("evidence_missing", "usage segment is not registered", 3))?;
        let requested = &segment.configuration;
        let actual = self.identity.configuration();
        if requested.agent_app != actual.agent_app
            || requested.agent_version != actual.agent_version
            || requested.requested_provider != actual.requested_provider
            || requested.requested_model != actual.requested_model
            || requested.effort != actual.effort
            || requested.config_digest != actual.config_digest
        {
            return Err(conflict());
        }
        for (configured, observed) in [
            (&requested.effective_provider, &actual.effective_provider),
            (&requested.effective_model, &actual.effective_model),
            (&requested.model_vendor, &actual.model_vendor),
            (&requested.service_tier, &actual.service_tier),
            (&requested.provider_region, &actual.provider_region),
        ] {
            if configured.is_some() && observed.is_some() && configured != observed {
                return Err(conflict());
            }
        }
        if self.identity.quality == IdentityQuality::Verified {
            return Err(Error::new(
                "capability_missing",
                "verified model identity requires an authorized collector",
                3,
            ));
        }
        if self.attestation != Attestation::Local {
            return Err(Error::new(
                "capability_missing",
                "provider reconciliation requires independent evidence",
                3,
            ));
        }
        Ok(())
    }
    pub(super) fn stream(&self) -> String {
        format!(
            "{}\0{}\0{}\0{}",
            self.source.adapter,
            self.source.adapter_version,
            self.source.namespace,
            self.host_session_id.as_deref().unwrap_or("")
        )
    }
    pub(super) fn source_key(&self) -> String {
        format!(
            "{}\0{}\0{}\0{:?}",
            self.stream(),
            self.source.request_id.as_deref().unwrap_or(""),
            self.source.event_id,
            self.source.scope
        )
    }
    pub(super) fn group(&self) -> String {
        format!("{}\0{}\0{}", self.run_id, self.segment_id, self.stream())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_totals_remain_unknown_even_with_known_subsets() {
        let counters = Counters::from_values([None, Some(12), Some(3), None, Some(4)]);
        assert!(counters.validate().is_ok());
        assert_eq!(counters.total().unwrap(), None);
    }

    #[test]
    fn cache_and_reasoning_subsets_cannot_exceed_parents() {
        for values in [
            [Some(10), Some(8), Some(3), Some(4), None],
            [Some(10), None, None, Some(4), Some(5)],
        ] {
            assert!(Counters::from_values(values).validate().is_err());
        }
    }

    #[test]
    fn unknown_parent_cannot_hide_an_unsafe_minimum_total() {
        let counters = Counters::from_values([None, Some(MAX_COUNT), None, None, Some(1)]);
        assert!(counters.validate().is_err());
        assert!(add(MAX_COUNT, 1).is_err());
        assert_eq!(add(MAX_COUNT, 0).unwrap(), MAX_COUNT);
    }

    #[test]
    fn zero_is_known_and_calendar_validation_is_strict() {
        let zero = Counters::from_values([Some(0), None, None, Some(0), None]);
        assert_eq!(zero.total().unwrap(), Some(0));
        assert!(timestamp("2024-02-29T00:00:00.001Z"));
        assert!(!timestamp("2025-02-29T00:00:00Z"));
        assert!(!timestamp("2026-11-18T24:00:00Z"));
    }
}
