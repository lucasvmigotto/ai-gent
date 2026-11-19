# Implementation Plan: Live acceptance testing on a shared roadmap page (`qa:uat`)

**Branch**: `feat/qa-uat` (proposed; per `git:workflow`, off `dev`) | **Date**: 2026-11-19 | **Spec**: `spec.md` | **Milestone**: 3.x, after 3.0.0 (Decision D8)

## Summary

A new skill `uat` in the existing `qa` plugin (`/qa:uat`, Decision D1), with references for the roadmap rules, the independent review, access, triage, messages and follow-up, and a new `ai-gent uat` subcommand in the main Rust binary (Decision D7) that does the deterministic work: lint the roadmap source, render the page from an embedded template, diff versions, build and verify the preflight record, digest register snapshots into status reports, and write the consolidated report. The agent keeps the judgment work: deriving steps from evidence, the review, triage, replies and the conversation with the owner. The page is HTML and inline JavaScript that runs in the claude.ai viewer; it is a template, not Rust.

## Where it lives: a skill in `qa`, not a new plugin

- `qa` already owns "what no single side owns" and the Verified status (`plugins/qa/references/qa-quality.md`), and `qa:strategy` already lists UAT as a layer, defines personas per role and asks "who does UAT". Live acceptance testing is that layer's execution.
- It shares `qa`'s references (`qa-quality.md`, `pipeline-digest.md`, `spec-kit.md`, `asking.md`, `reading-code.md`) and its personas; a new plugin would duplicate them as symlinks and add a `plugin.json`, an evals suite, docs rows and an install choice for one skill.
- Neighbors and why not them: `project` (pipeline and specs, not testing with people), `frontend` (builds product UIs; the page is a test instrument), `agents` (orchestration of agents, not people), `repo` (tracker writes; used for defects through `repo:issues`).
- Cost of the choice: `qa`'s other skills are tier-agnostic, while this one needs the Artifact tool (tier 1). Reduced mode (FR-090) keeps it usable elsewhere.

## Technical Context

**Skill**: Markdown (`SKILL.md` and references), English (`shared/language-policy.md`); messages to the team in the owner's language.
**New code**: Rust only, in the existing workspace (`rust/`, edition 2024, pinned toolchain, `clap`, `serde`, `serde_json`, `thiserror`; tests with `insta` and `assert_cmd`), as `ai-gent uat <action>` with `--json` and documented exit codes (`docs/product/rust-decisions.md` D9). No Python or shell is added (D7, D16 there). The page template is HTML with inline CSS and JavaScript, embedded with `include_str!` so the package needs no extra runtime file. Mode B reads Spec Kit features with the existing `ai_gent_core::spec::parse` (Decision D8).
**Agent tools**: Artifact (publish, read), `ArtifactData` (`list` and `query`, including at a lower access level for the preflight test read; writes only on the owner's request), Agent (independent read-only reviewer), and, only when the owner turns it on, a session timer.
**Storage**: the session folder `.ai-gent/uat/<slug>/` (git-excluded), files listed in `data-model.md`. Nothing in `specs/`, `docs/` or git (Decision D2).
**Testing**: Rust unit tests and `insta` goldens in `ai-gent-core`; CLI tests with `assert_cmd` in `rust/crates/ai-gent/tests/uat.rs`; fixtures in `rust/fixtures/uat/`. The page is tested by a behavior eval in `plugins/qa/evals/` (rendered fixture page, structural checks and axe where the eval sandbox allows) plus a manual pilot; **no new CI job** (Decision D8). The existing `cargo test`, `clippy` and `fmt` gates in `scripts/check.sh` and CI cover the Rust module.

## The `ai-gent uat` subcommand

The binary cannot call the Artifact tools; the agent saves what `ArtifactData` returns to snapshot files and passes their paths. All actions are read-only on the repository and write only under `.ai-gent/uat/<slug>/` or stdout.

| Action | Input | Output | Exit | FR |
|---|---|---|---|---|
| `uat lint <roadmap.json>` | roadmap source | findings: sections of FR-001, R0 write-check and role-confirmation steps, missing `who`/`expected`, claims without labels, the `ASSUMPTION` list, unknown role or validator without an assigned role, duplicate or malformed ids, test record shared across scripts, rule without a refused-role step, exclusivity or wait missing where declared needed, page text with code-like tokens (paths, SQL keywords, identifiers, e-mails), mode B scenarios without a step (via `spec::parse`), per-document size budget | 0 clean or warnings only, 1 errors, 2 usage | 001-010, 091, 098 |
| `uat render <roadmap.json> [--out page.html]` | roadmap source | the page from the embedded template; attachments controls only when `settings.attachments.on`; refuses on lint errors | 0, 1, 2 | 011-026, 050, 051 |
| `uat diff <old.json> <new.json>` | two versions | renumbered, removed or reused ids refused; revision bumps for changed steps; retired steps; appended steps; changelog entry draft; list of steps whose claims changed (for the review) | 0, 1 refused, 2 | 005, 009, 022 |
| `uat preflight checklist <roadmap.json>` | validators, settings | the checklist part of `preflight.md` (`contracts/reports.md`): level needed, caveats, menu change, `pending` confirmations | 0, 1 action needed | 092, 097 |
| `uat preflight verify <roadmap.json> --declared <capabilities.json> --read <level>=<snapshot.json>…` | the declaration published and one test-read snapshot per level | declaration compared with `data-model.md`; seen versus expected per collection and level; the proves / does-not-prove text; exit 1 on any mismatch | 0, 1 mismatch, 2 | 093 |
| `uat digest <roadmap.json> <snapshot.json>… [--since <snapshot.json>…] [--follow-up follow-up.json]` | roadmap and register snapshot pages | status report per `contracts/reports.md`, with the last real check from the snapshot's time; the "no news" line when nothing changed | 0 | 019-021, 075, 091, 095, 098, 099 |
| `uat report <roadmap.json> <snapshot.json>… [--triage triage.md] [--decisions decisions.md] [--preflight preflight.md]` | the same plus the session records | `report.md` | 0 | 060, 079 |

Code layout, following `delegate`, `spec` and `wt`:

| Path | Role |
|---|---|
| `rust/crates/ai-gent-core/src/uat/mod.rs` | public API |
| `rust/crates/ai-gent-core/src/uat/model.rs` | roadmap, step, validator, register and snapshot types (`serde`) |
| `rust/crates/ai-gent-core/src/uat/lint.rs` | roadmap checks |
| `rust/crates/ai-gent-core/src/uat/render.rs`, `uat/template/page.html` | template filling, HTML escaping |
| `rust/crates/ai-gent-core/src/uat/diff.rs` | id stability and revisions |
| `rust/crates/ai-gent-core/src/uat/preflight.rs` | checklist and verification |
| `rust/crates/ai-gent-core/src/uat/digest.rs`, `report.rs` | derived values (`data-model.md`) and reports |
| `rust/crates/ai-gent/src/cmd/uat.rs` | `clap` arguments, `--json`, exit codes |
| `rust/crates/ai-gent/src/cli.rs` | `Uat(...)` variant |
| `rust/crates/ai-gent/tests/uat.rs`, `rust/fixtures/uat/` | CLI tests and fixtures |

If `ai-gent` is missing, the skill says so with the install command and stops (rust-decisions D8); it never rebuilds the logic in shell.

## Skill files

| File | Role |
|---|---|
| `plugins/qa/skills/uat/SKILL.md` | phases, confirmation points (FR-070), what the agent never does (FR-071, FR-078, FR-080); pointers to references |
| `plugins/qa/skills/uat/references/roadmap.md` | deriving scripts and steps (modes A and B), evidence labels, observability, target roles, R0 (write check, role confirmation), exclusivity, waits, test records |
| `plugins/qa/skills/uat/references/review.md` | the independent reviewer's brief (read-only, claim by claim, findings format) |
| `plugins/qa/skills/uat/references/access.md` | the preflight checklist, the test read and what it proves and does not, the refusal notice, the Editor caveat |
| `plugins/qa/skills/uat/references/triage.md` | classes, read-only investigation, reply drafting, decision list, `repo:issues` hand-off |
| `plugins/qa/skills/uat/references/messages.md` | team message templates (lists, plain language) |
| `plugins/qa/skills/uat/references/follow-up.md` | on request by default; turning the session timer on, its limits, the last-real-check rule |
| `plugins/qa/skills/uat/references/reduced-mode.md` | the local page and the per-validator checklist without the Artifact tool |
| `plugins/qa/evals/uat-*/` | trigger cases (reaches `qa:uat`, not `qa:e2e` or `qa:strategy`) and behavior cases on a fixture roadmap and snapshots |

Description draft (single line, under 400 characters, no `: `): "Run live acceptance testing with business validators — a roadmap of scripts and steps written from evidence and reviewed independently, published as a shared page where each person marks OK or NOK and suggests improvements, followed by triage of every NOK and team status messages. Use for "UAT", "acceptance testing with users", "test roadmap", "validation session"."

## Phases of the skill

1. **Inputs** (one question round): scope and mode, validators with their assigned roles, environment (US1). **Confirm.**
2. **Discovery**: read `spec.md` and acceptance scenarios, contracts, `backend.md` authorization matrix, `qa.md` personas, code and tests (`compact:code` for large files), test data read-only (`db:inspect` when a database is involved). Label every claim.
3. **Draft** `roadmap.json`; `ai-gent uat lint` until clean; the `ASSUMPTION` list to the owner.
4. **Independent review** by a separate read-only subagent (`references/review.md`); fix findings; record the review. **Confirm.**
5. **Preflight checklist** (`ai-gent uat preflight checklist`), line by line with the owner. Attachments and their budget only if the owner turns them on here. **Confirm each line.**
6. **Render and publish** (`ai-gent uat render`; capabilities from `data-model.md`); write `meta/roadmap`. **Confirm.**
7. **Preflight test read**: `ArtifactData` at `view`, `interact`, `admin`; `ai-gent uat preflight verify`; mismatch blocks. **Confirm.**
8. **Invitation** draft. **The owner sends it.**
9. **Follow-up** on request: snapshot via `ArtifactData`, `ai-gent uat digest`, triage of new NOKs, replies and status drafts, write-check tracking. Periodic only if the owner turns it on (`references/follow-up.md`). **Confirm turning it on.**
10. **Roadmap changes**: edit `roadmap.json`, `ai-gent uat diff`, review changed claims, compare the published page with `page.html`, republish without `capabilities`, update `meta`. **Confirm each.**
11. **Closing**: list open items, `ai-gent uat report`, closing message, decision list, optional `repo:issues` for defects, remove the timer if on. **Confirm.**

## Integrations

- `qa:strategy`: personas and the UAT row of `qa.md`; `qa:uat` reads them and reports an `[UPSTREAM GAP]` to the owner when roles are missing.
- `project:spec`: mode B reads Spec Kit features through `spec::parse`; a wrong or untestable scenario is reported as `[UPSTREAM GAP]`, never edited.
- `qa:e2e`: a NOK confirmed as a defect is a candidate for an automated negative or regression journey; the report lists them.
- `repo:issues`: defects become issues, asked first; `repo:sync` untouched.
- `agents:*`: the independent reviewer is a plain read-only subagent; `agents:delegate` is not used for it (claims need repository and data access that delegates do not get).
- `git:workflow`: not used; the skill commits nothing (Decision D2).
- `db:connect` / `db:inspect`: read-only checks of test data during discovery and triage, through the audited runner, never production.

## Impact outside the skill

- `README.md` (the `qa` row and skill list), the docs-site skill reference (generated from the skill source), `CHANGELOG.md` `## Unreleased`: through `skill-docs` after the build. `AGENTS.md` unchanged (no new rule).
- `plugins/qa/.claude-plugin/plugin.json` description gains `uat`; version bumped by `release.py`, not by hand.
- `scripts/check.sh` checks the new SKILL.md (name, description) and reference links; the Rust gates already cover the module; `scripts/package.py --check` unchanged (the template is embedded).
- `shared/pipeline.md`: unchanged (Decision D2). `.gitignore`: unchanged (`.ai-gent/` is already excluded locally). CI: no new job (Decision D8).

## Risks

- The Artifact runtime contract and sharing levels move; the page and the checklist follow the contract version the skill was written against, and the skill re-reads the capability guidance before rendering.
- The test read proves the rules only for reading; a wrong write rule would surface as validators' refused writes. Mitigation: the declaration check and the R0 write check per validator (FR-093).
- Validators keep using NOK for change requests; mitigation: the how-to section, the improvement control on every step, and the reply template.
- Validators shared as Editors can republish the page; mitigation: FR-097 detects it before the agent's next republish; open question 2.
- Binary size: the module is small and adds no dependency.

## Constitution Check

N/A — no constitution in this repository; `AGENTS.md` rules apply (propose first, `scripts/check.sh` before every commit).
