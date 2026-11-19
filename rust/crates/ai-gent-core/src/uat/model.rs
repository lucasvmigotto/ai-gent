//! The roadmap source (`.ai-gent/uat/<slug>/roadmap.json`, `specs/055-test-roadmap/data-model.md`).
//!
//! Every field is optional on read (`serde(default)`), so a draft with gaps still loads and `lint`
//! reports the gaps instead of the parser refusing the file.
use serde::{Deserialize, Serialize};

/// Evidence labels a claim may carry.
pub const LABELS: [&str; 3] = ["OBSERVED", "INFERRED", "ASSUMPTION"];

/// A statement of fact in the roadmap and where it came from (FR-007).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Claim {
    /// The statement.
    pub text: String,
    /// `OBSERVED`, `INFERRED` or `ASSUMPTION`.
    pub label: String,
    /// A path and line, a spec anchor, a query description or a run; never shown on the page.
    #[serde(rename = "ref")]
    pub reference: String,
}

/// How long to wait before looking, and at what (FR-016).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wait {
    /// Seconds to wait.
    pub seconds: u32,
    /// What to look at afterwards.
    pub look_at: String,
}

/// One action and its expected result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Step {
    /// `r<N>-<M>`; never renumbered or reused (FR-005).
    pub id: String,
    /// Display rank.
    pub order: u32,
    /// Revision, +1 when the text changes after a publish.
    pub rev: u32,
    /// `active` or `retired`.
    pub status: String,
    /// `normal`, `write-check` or `role-confirmation`.
    pub kind: String,
    /// The target role key.
    pub who: String,
    /// A named validator key, optional.
    pub validator: String,
    /// What to do.
    pub action: String,
    /// What the target role sees on screen (FR-006).
    pub expected: String,
    /// The role that can see the result, when it is not `who`. A step like that is a defect.
    pub seen_by: String,
    /// What must exist before the step.
    pub pre: String,
    /// The exclusivity note shown on the step.
    pub exclusive_note: String,
    /// The wait, when one is needed.
    pub wait: Option<Wait>,
    /// What evidence a NOK should bring.
    pub evidence_expected: String,
    /// `specs/NNN-*/spec.md#US<n>-AS<m>` anchors.
    pub traces: Vec<String>,
    /// The rule key this step exercises, if any.
    pub rule: String,
    /// The target role must be refused by this step (FR-004).
    pub refused: bool,
    /// The claims the step rests on.
    pub claims: Vec<Claim>,
}

impl Step {
    /// Whether the step is retired.
    #[must_use]
    pub fn retired(&self) -> bool {
        self.status == "retired"
    }
}

/// A group of steps (FR-003).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Script {
    /// `r0` ... `rN`; never reused.
    pub id: String,
    /// Shown on the page.
    pub title: String,
    /// `prep`, `flow`, `rule` or `complement`.
    pub kind: String,
    /// Test records, named `<script>-<yyyymmdd>-...`; none is shared by two scripts.
    pub test_data: Vec<String>,
    /// The script needs the test record to itself.
    pub exclusive: bool,
    /// The step that cleans up, if any.
    pub cleanup_step: String,
    /// The steps.
    pub steps: Vec<Step>,
}

/// What a kind of person may do.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Role {
    /// The key.
    pub key: String,
    /// Shown on the page.
    pub label: String,
    /// Plain-language capabilities.
    pub can: Vec<String>,
    /// Ordinal for the coverage view.
    pub privilege: u32,
}

/// A person who tests (FR-091).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Validator {
    /// The key.
    pub key: String,
    /// Shown on the page; never an e-mail or account (D9).
    pub label: String,
    /// The role the owner assigned; required.
    pub assigned_role: String,
    /// Account or e-mail; kept local, never on the page.
    pub invite: String,
    /// The scripts the person is expected to run.
    pub expected_scripts: Vec<String>,
    /// The person is outside the owner's organization (FR-092).
    pub outside_org: bool,
    /// The sharing level the owner says they will give: `viewer`, `commenter`, `contributor` or `editor`.
    /// The agent cannot see anyone's level; this is the owner's word (FR-092).
    pub planned_sharing: String,
}

/// An authorization or business rule under test, in plain words.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// The key.
    pub key: String,
    /// The rule in plain words.
    pub text: String,
    /// An authorization rule needs a step whose target must be refused (FR-004).
    pub authorization: bool,
}

/// Something that works differently from what a validator may expect.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Limitation {
    /// The text.
    pub text: String,
    /// The steps it concerns.
    pub steps: Vec<String>,
}

/// A mode B scenario that cannot be run by hand, and why (FR-008).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotTestable {
    /// `specs/NNN-*/spec.md#US<n>-AS<m>`.
    pub trace: String,
    /// Why.
    pub reason: String,
}

/// The environment under test (FR-080).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Environment {
    /// Its name, shown on the page.
    pub name: String,
    /// Never `production`.
    pub class: String,
}

/// One changelog entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Change {
    /// The roadmap version it made.
    pub version: u32,
    /// When.
    pub at: String,
    /// What changed.
    pub summary: String,
    /// The steps it touched.
    pub steps: Vec<String>,
}

/// One independent review (FR-009).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Review {
    /// The roadmap version reviewed.
    pub version: u32,
    /// The reviewer.
    pub by: String,
    /// When.
    pub at: String,
    /// The result.
    pub result: String,
    /// The findings.
    pub findings: Vec<String>,
}

/// Per-session switches; everything off by default (D4, D6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Attachments.
    pub attachments: Attachments,
    /// Periodic follow-up.
    pub follow_up: FollowUp,
    /// The per-document budget in bytes, from the runtime contract; unset skips the size check.
    pub doc_budget_bytes: Option<u64>,
}

/// Attachment settings (FR-094).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Attachments {
    /// On only at the owner's request.
    pub on: bool,
    /// The session's size budget.
    pub budget_mb: u32,
}

/// Follow-up settings (FR-077).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FollowUp {
    /// On only at the owner's request.
    pub on: bool,
    /// The interval.
    pub every_minutes: u32,
}

/// The roadmap source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roadmap {
    /// The session's name, a path-safe slug.
    pub slug: String,
    /// The page title.
    pub title: String,
    /// +1 per publish that changes text.
    pub version: u32,
    /// The environment.
    pub environment: Environment,
    /// `A` whole application, `B` features.
    pub mode: String,
    /// Spec Kit feature numbers or directories (mode B).
    pub features: Vec<String>,
    /// How to use the page (FR-001, first section).
    pub how_to: String,
    /// The roles.
    pub roles: Vec<Role>,
    /// The people.
    pub validators: Vec<Validator>,
    /// The rules in plain words.
    pub rules: Vec<Rule>,
    /// The scripts, R0 first.
    pub scripts: Vec<Script>,
    /// Known limitations.
    pub known_limitations: Vec<Limitation>,
    /// Mode B scenarios not testable by hand.
    pub not_testable: Vec<NotTestable>,
    /// The changelog.
    pub changelog: Vec<Change>,
    /// The reviews.
    pub review: Vec<Review>,
    /// The Artifact link.
    pub artifact_url: String,
    /// Switches.
    pub settings: Settings,
}

impl Roadmap {
    /// Parse a roadmap source.
    ///
    /// # Errors
    /// When the text is not JSON of this shape.
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("not a roadmap: {e}"))
    }

    /// Every step with its script, retired ones included.
    pub fn steps(&self) -> impl Iterator<Item = (&Script, &Step)> {
        self.scripts
            .iter()
            .flat_map(|s| s.steps.iter().map(move |st| (s, st)))
    }

    /// The steps that are not retired.
    pub fn active_steps(&self) -> impl Iterator<Item = (&Script, &Step)> {
        self.steps().filter(|(_, st)| !st.retired())
    }
}
