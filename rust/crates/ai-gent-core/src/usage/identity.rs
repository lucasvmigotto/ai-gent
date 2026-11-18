//! Immutable registered scopes and model segments; all caller claims remain reported/unknown.
use super::{Error, Result, conflict, invalid};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "Explicit identity names are part of the versioned wire contract."
)]
pub(super) struct TaskKey {
    pub repository_id: Uuid,
    pub feature_id: String,
    pub task_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Configuration {
    pub agent_app: String,
    pub agent_version: String,
    pub requested_provider: Option<String>,
    pub requested_model: Option<String>,
    pub effective_provider: Option<String>,
    pub effective_model: Option<String>,
    pub effort: Option<String>,
    pub config_digest: String,
    pub model_vendor: Option<String>,
    pub service_tier: Option<String>,
    pub provider_region: Option<String>,
}
fn selector(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 256
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:/@+".contains(&c))
}
impl Configuration {
    fn validate(&self) -> Result<()> {
        if !selector(&self.agent_app)
            || !selector(&self.agent_version)
            || [
                &self.requested_provider,
                &self.requested_model,
                &self.effective_provider,
                &self.effective_model,
                &self.effort,
                &self.model_vendor,
                &self.service_tier,
                &self.provider_region,
            ]
            .iter()
            .any(|s| s.as_ref().is_some_and(|s| !selector(s)))
            || !digest(&self.config_digest)
        {
            return Err(invalid());
        }
        Ok(())
    }
}
pub(super) fn digest(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Planner,
    Coordinator,
    Builder,
    Reviewer,
    Repair,
    Verification,
    Recovery,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Role {
    Planner,
    Coordinator,
    Builder,
    Reviewer,
    Reader,
}
impl Phase {
    fn role(self) -> Role {
        match self {
            Self::Planner => Role::Planner,
            Self::Coordinator | Self::Recovery => Role::Coordinator,
            Self::Builder | Self::Repair => Role::Builder,
            Self::Reviewer => Role::Reviewer,
            Self::Verification => Role::Reader,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Purpose {
    Implementation,
    Retry,
    Review,
    Repair,
    Planning,
    Coordination,
    Verification,
    Recovery,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum State {
    Registered,
    Running,
    Finished,
    Failed,
    Cancelled,
}
impl State {
    fn terminal(self) -> bool {
        matches!(self, Self::Finished | Self::Failed | Self::Cancelled)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Registration {
    pub task: TaskKey,
    pub host_session_id: Option<String>,
    pub phase: Phase,
    pub role: Role,
    pub purpose: Purpose,
    pub configuration: Configuration,
    pub attempt_id: Option<Uuid>,
    pub parent_attempt_id: Option<Uuid>,
    pub parent_run_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Switch {
    pub run_id: Uuid,
    pub expected_generation: u64,
    pub configuration: Configuration,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Transition {
    pub run_id: Uuid,
    pub expected_generation: u64,
    pub state: State,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "operation",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum Operation {
    Register(Registration),
    Segment(Switch),
    Transition(Transition),
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "Explicit identity names are part of the versioned wire contract."
)]
pub(super) struct Request {
    pub schema_version: u32,
    pub request_id: Uuid,
    pub repository_id: Uuid,
    #[serde(flatten)]
    pub operation: Operation,
}
impl Request {
    pub fn validate(&self, name: &str) -> Result<()> {
        if self.schema_version != 1 {
            return Err(Error::new(
                "unsupported_version",
                "only registration schema version 1 is supported",
                2,
            ));
        }
        if self.request_id.is_nil() || self.repository_id.is_nil() {
            return Err(invalid());
        }
        match &self.operation {
            Operation::Register(r) if name == "register" => {
                if r.task.repository_id != self.repository_id {
                    return Err(conflict());
                }
                let f = &r.task.feature_id;
                if f.len() > 128
                    || f.len() < 5
                    || !f.as_bytes()[..3].iter().all(u8::is_ascii_digit)
                    || f.as_bytes()[3] != b'-'
                    || !f[4..].split('-').all(|p| {
                        !p.is_empty()
                            && p.bytes()
                                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                    })
                    || r.task.task_id.len() < 4
                    || r.task.task_id.len() > 32
                    || !r.task.task_id.starts_with('T')
                    || !r.task.task_id.as_bytes()[1..]
                        .iter()
                        .all(u8::is_ascii_digit)
                    || r.phase.role() != r.role
                    || [r.attempt_id, r.parent_attempt_id, r.parent_run_id, r.run_id]
                        .iter()
                        .flatten()
                        .any(Uuid::is_nil)
                    || (r.attempt_id.is_some() && r.parent_attempt_id.is_some())
                {
                    return Err(invalid());
                }
                if r.host_session_id.as_ref().is_some_and(|id| !selector(id)) {
                    return Err(invalid());
                }
                r.configuration.validate()
            }
            Operation::Segment(s) if name == "segment" && !s.run_id.is_nil() => {
                s.configuration.validate()
            }
            Operation::Transition(t) if name == "transition" && !t.run_id.is_nil() => Ok(()),
            _ => Err(invalid()),
        }
    }
    pub fn name(&self) -> &'static str {
        match self.operation {
            Operation::Register(_) => "register",
            Operation::Segment(_) => "segment",
            Operation::Transition(_) => "transition",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "Explicit identity names are part of the versioned wire contract."
)]
pub(super) struct Segment {
    pub segment_id: Uuid,
    pub configuration: Configuration,
    pub identity_quality: String,
}
impl Segment {
    fn new(configuration: Configuration, id: Uuid) -> Self {
        let quality = if configuration.effective_provider.is_some()
            && configuration.effective_model.is_some()
        {
            "reported"
        } else {
            "unknown"
        };
        Self {
            segment_id: id,
            configuration,
            identity_quality: quality.into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "Explicit identity names are part of the versioned wire contract."
)]
pub(super) struct Run {
    pub task: TaskKey,
    pub host_session_id: Option<String>,
    pub task_fingerprint: String,
    pub attempt_id: Uuid,
    pub attempt_ordinal: u64,
    pub parent_attempt_id: Option<Uuid>,
    pub run_id: Uuid,
    pub parent_run_id: Option<Uuid>,
    pub phase: Phase,
    pub role: Role,
    pub purpose: Purpose,
    pub generation: u64,
    pub state: State,
    pub segments: Vec<Segment>,
    pub usage_quality: String,
}
#[derive(Default)]
pub(super) struct Ledger {
    pub repository_id: Option<Uuid>,
    pub sequence: u64,
    pub runs: BTreeMap<Uuid, Run>,
    pub requests: BTreeMap<Uuid, (Request, Run)>,
}
fn missing() -> Error {
    Error::new(
        "evidence_missing",
        "referenced task, attempt or parent run is not registered",
        3,
    )
}
fn stale() -> Error {
    Error::new("stale_generation", "run generation changed", 4)
}
impl Ledger {
    pub fn apply(
        &mut self,
        request: &Request,
        fingerprint: &str,
        seed: Option<&Run>,
    ) -> Result<Run> {
        if self
            .repository_id
            .is_some_and(|id| id != request.repository_id)
        {
            return Err(conflict());
        }
        let run = match &request.operation {
            Operation::Register(r) => self.register(r, fingerprint, seed)?,
            Operation::Segment(s) => {
                let mut old = self.runs.get(&s.run_id).ok_or_else(missing)?.clone();
                if old.generation != s.expected_generation {
                    return Err(stale());
                }
                if old.state.terminal()
                    || old
                        .segments
                        .last()
                        .is_some_and(|p| p.configuration == s.configuration)
                {
                    return Err(Error::new(
                        "policy_denied",
                        "cannot append this execution segment",
                        3,
                    ));
                }
                let id = seed
                    .and_then(|r| r.segments.last())
                    .map_or_else(Uuid::new_v4, |s| s.segment_id);
                if self
                    .runs
                    .values()
                    .flat_map(|r| &r.segments)
                    .any(|s| s.segment_id == id)
                {
                    return Err(conflict());
                }
                old.segments.push(Segment::new(s.configuration.clone(), id));
                old.generation += 1;
                old
            }
            Operation::Transition(t) => {
                let mut old = self.runs.get(&t.run_id).ok_or_else(missing)?.clone();
                if old.generation != t.expected_generation {
                    return Err(stale());
                }
                let allowed = matches!(
                    (old.state, t.state),
                    (
                        State::Registered,
                        State::Running | State::Failed | State::Cancelled
                    ) | (
                        State::Running,
                        State::Finished | State::Failed | State::Cancelled
                    )
                );
                if !allowed {
                    return Err(Error::new(
                        "policy_denied",
                        "invalid execution state transition",
                        3,
                    ));
                }
                old.state = t.state;
                old.generation += 1;
                old
            }
        };
        if seed.is_some_and(|expected| expected != &run) {
            return Err(super::corrupt());
        }
        self.repository_id = Some(request.repository_id);
        self.runs.insert(run.run_id, run.clone());
        self.requests
            .insert(request.request_id, (request.clone(), run.clone()));
        self.sequence += 1;
        Ok(run)
    }
    fn register(&self, r: &Registration, fingerprint: &str, seed: Option<&Run>) -> Result<Run> {
        if !digest(fingerprint) {
            return Err(invalid());
        }
        if let Some(parent) = r.parent_run_id {
            let p = self.runs.get(&parent).ok_or_else(missing)?;
            if p.task != r.task {
                return Err(conflict());
            }
        }
        if let Some(parent) = r.parent_attempt_id {
            let p = self
                .runs
                .values()
                .find(|p| p.attempt_id == parent)
                .ok_or_else(missing)?;
            if p.task != r.task {
                return Err(conflict());
            }
        }
        let (attempt_id, ordinal) = if let Some(id) = r.attempt_id {
            let existing = self
                .runs
                .values()
                .find(|p| p.attempt_id == id)
                .ok_or_else(missing)?;
            if existing.task != r.task || existing.task_fingerprint != fingerprint {
                return Err(conflict());
            }
            (id, existing.attempt_ordinal)
        } else {
            let ordinal = self
                .runs
                .values()
                .filter(|p| p.task == r.task)
                .map(|p| p.attempt_ordinal)
                .max()
                .unwrap_or(0)
                + 1;
            let id = seed.map_or_else(Uuid::new_v4, |s| s.attempt_id);
            if self.runs.values().any(|p| p.attempt_id == id) {
                return Err(conflict());
            }
            (id, ordinal)
        };
        let run_id = r
            .run_id
            .unwrap_or_else(|| seed.map_or_else(Uuid::new_v4, |s| s.run_id));
        if run_id.is_nil() || self.runs.contains_key(&run_id) {
            return Err(conflict());
        }
        let segment_id = seed
            .and_then(|s| s.segments.first())
            .map_or_else(Uuid::new_v4, |s| s.segment_id);
        if attempt_id.is_nil()
            || segment_id.is_nil()
            || self
                .runs
                .values()
                .flat_map(|r| &r.segments)
                .any(|s| s.segment_id == segment_id)
        {
            return Err(conflict());
        }
        Ok(Run {
            task: r.task.clone(),
            host_session_id: r.host_session_id.clone(),
            task_fingerprint: fingerprint.into(),
            attempt_id,
            attempt_ordinal: ordinal,
            parent_attempt_id: r.parent_attempt_id.or_else(|| {
                r.attempt_id.and_then(|id| {
                    self.runs
                        .values()
                        .find(|p| p.attempt_id == id)
                        .and_then(|p| p.parent_attempt_id)
                })
            }),
            run_id,
            parent_run_id: r.parent_run_id,
            phase: r.phase,
            role: r.role,
            purpose: r.purpose,
            generation: 0,
            state: State::Registered,
            segments: vec![Segment::new(r.configuration.clone(), segment_id)],
            usage_quality: "missing".into(),
        })
    }
}
