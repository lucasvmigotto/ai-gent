#!/usr/bin/env python3
"""Tests scripts/consistency.py on throwaway trees. Called by check.sh."""
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import consistency  # noqa: E402


def tree(files: dict[str, str]) -> Path:
    root = Path(tempfile.mkdtemp())
    for name, text in files.items():
        p = root / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
    return root


SKILL = "plugins/git/skills/workflow/SKILL.md"


class Sections(unittest.TestCase):
    def test_heading_found(self):
        root = tree({SKILL: "---\nname: workflow\n---\n## Landing\n", "plugins/repo/a.md": "`git:workflow` → *Landing*\n"})
        self.assertEqual(consistency.check_sections(root), [])

    def test_bold_lead_in_and_reference_found(self):
        root = tree({
            SKILL: "---\nname: workflow\n---\n- **A new repository** starts\n",
            "plugins/git/skills/workflow/references/x.md": "# Issues and work items\n",
            "shared/a.md": "`git:workflow` says (*A new repository*) and `git:workflow`, *Issues and work items*\n",
        })
        self.assertEqual(consistency.check_sections(root), [])

    def test_missing_heading_fails(self):
        root = tree({SKILL: "---\nname: workflow\n---\n## Landing\n", "shared/a.md": "see `git:workflow` → *Merging*\n"})
        problems = consistency.check_sections(root)
        self.assertEqual(len(problems), 1)
        self.assertIn("no section 'Merging'", problems[0])


class Retired(unittest.TestCase):
    def test_clean(self):
        self.assertEqual(consistency.check_retired(tree({"shared/a.md": "Ask up to four questions per prompt.\n"})), [])

    def test_each_phrasing_fails(self):
        for text in ("Ask 3–5 questions.", "one question at a time", "at most 5 questions", "all 12 plugins", "47 skills"):
            with self.subTest(text=text):
                self.assertEqual(len(consistency.check_retired(tree({"README.md": text + "\n"}))), 1)


class Containers(unittest.TestCase):
    def test_creating_without_cleanup_fails(self):
        root = tree({"plugins/x/skills/y/SKILL.md": "Give each track a named cache volume.\n"})
        self.assertEqual(len(consistency.check_cleanup(root)), 1)

    def test_creating_with_cleanup_passes(self):
        for text in ("remove them per `../../references/containers.md` §9a", "then `wt.py remove <branch>`",
                     "run the project's `clean` recipe"):
            with self.subTest(text=text):
                root = tree({"plugins/x/skills/y/SKILL.md": "Start one with `wt.py add`.\n",
                             "plugins/x/skills/y/references/r.md": text + "\n"})
                self.assertEqual(consistency.check_cleanup(root), [])

    def test_prune_taught_as_a_command_fails(self):
        self.assertEqual(len(consistency.check_prune(tree({"shared/a.md": "Run `podman system prune -a`.\n"}))), 1)
        self.assertEqual(len(consistency.check_prune(tree({"shared/a.md": "`$CONTAINER_ENGINE image prune`\n"}))), 1)

    def test_prune_said_never_passes(self):
        root = tree({"shared/a.md": "Never `podman system prune`; `git fetch --prune` is fine.\n"})
        self.assertEqual(consistency.check_prune(root), [])


CARDS = "specs/004-daemon/cards.md"


class SecretData(unittest.TestCase):
    SOURCES = {"shared/secret-patterns.json": json.dumps({"patterns": [{"kind": "k", "source": "x", "flags": "", "replacement": "<r>"}]}),
               "shared/secret-cases.json": json.dumps({"vars": {}, "cases": []})}

    def test_nothing_to_check_without_the_shared_files(self):
        self.assertEqual(consistency.check_secret_data(tree({})), [])

    def test_missing_or_stale_twin_fails_and_sync_fixes_it(self):
        root = tree(self.SOURCES)
        self.assertEqual(len(consistency.check_secret_data(root)), 1)
        (root / consistency.SECRET_DATA).parent.mkdir(parents=True, exist_ok=True)
        (root / consistency.SECRET_DATA).write_text(consistency.secret_data_text(root))
        self.assertEqual(consistency.check_secret_data(root), [])
        (root / "shared/secret-cases.json").write_text(json.dumps({"vars": {}, "cases": [{"name": "n"}]}))
        self.assertEqual(len(consistency.check_secret_data(root)), 1)

    def test_a_result_skip_survives_the_sync(self):
        src = dict(self.SOURCES)
        src["shared/secret-patterns.json"] = json.dumps({"patterns": [{"kind": "k", "source": "x", "flags": "", "replacement": "<r>", "skipIn": ["result"]}]})
        root = tree(src)
        self.assertIn('"skipIn": [\n        "result"\n      ]', consistency.secret_data_text(root))
        (root / consistency.SECRET_DATA).parent.mkdir(parents=True, exist_ok=True)
        (root / consistency.SECRET_DATA).write_text(consistency.secret_data_text(root))
        self.assertEqual(consistency.check_secret_data(root), [])

    def test_the_repository_twin_is_in_sync(self):
        self.assertEqual(consistency.check_secret_data(consistency.ROOT), [])


class TokenTagsCopy(unittest.TestCase):
    def test_nothing_to_check_without_the_rust_workspace(self):
        self.assertEqual(consistency.check_token_tags_copy(tree({"shared/token-tags.json": "{}"})), [])

    def test_missing_or_edited_copy_fails_and_equal_passes(self):
        root = tree({"shared/token-tags.json": "{}", "rust/Cargo.toml": ""})
        self.assertEqual(len(consistency.check_token_tags_copy(root)), 1)
        copy = root / consistency.TOKEN_TAGS_COPY
        copy.parent.mkdir(parents=True, exist_ok=True)
        copy.write_text("{}")
        self.assertEqual(consistency.check_token_tags_copy(root), [])
        copy.write_text('{"x": 1}')
        self.assertEqual(len(consistency.check_token_tags_copy(root)), 1)

    def test_the_repository_copy_is_in_sync(self):
        self.assertEqual(consistency.check_token_tags_copy(consistency.ROOT), [])


class PipelinePaths(unittest.TestCase):
    TABLE = "| Path | Owner |\n|---|---|\n| `docs/product/brief.md` | `project:init` |\n| `specs/README.md` | `project:spec` |\n"

    def model(self, *paths: str) -> str:
        return "export const BRIEF = 'docs/product/brief.md'\n" + "".join(f"  {{ path: {p}, label: 'x' }},\n" for p in paths)

    def test_paths_in_the_table_pass(self):
        root = tree({"shared/pipeline.md": self.TABLE, consistency.PIPELINE_MODEL: self.model("BRIEF", "'specs/README.md'")})
        self.assertEqual(consistency.check_pipeline_paths(root), [])

    def test_a_path_missing_from_the_table_fails(self):
        root = tree({"shared/pipeline.md": self.TABLE, consistency.PIPELINE_MODEL: self.model("BRIEF", "'docs/product/other.md'")})
        self.assertEqual(len(consistency.check_pipeline_paths(root)), 1)

    def test_the_repository_agrees(self):
        self.assertEqual(consistency.check_pipeline_paths(consistency.ROOT), [])


class PipelineDigest(unittest.TestCase):
    FULL = ("# P\n\n## Artifacts and owners\n\n| Path | Owner | R | N |\n|---|---|---|---|\n"
            "| `docs/product/brief.md` | `project:init` | all | x |\n| `specs/README.md` | `project:spec` | all | x |\n\n"
            "## Rules every stage follows\n\n1. **A.** x\n2. **B.** y\n   - *Planned* — a;\n"
            "   - *Verified* — b;\n")
    DIGEST = ("# D\n\nread `pipeline.md`.\n\n| Path | Owner | R |\n|---|---|---|\n"
              "| `docs/product/brief.md` | `project:init` | all |\n| `specs/README.md` | `project:spec` | all |\n\n"
              "## Statuses\n\nPlanned, Verified.\n\nSee §\"Rules every stage follows\" #2.\n")

    def run_check(self, full=None, digest=None):
        root = tree({"shared/pipeline.md": full or self.FULL, "shared/pipeline-digest.md": digest or self.DIGEST})
        return consistency.check_pipeline_digest(root)

    def test_a_faithful_digest_passes(self):
        self.assertEqual(self.run_check(), [])

    def test_a_path_the_contract_has_and_the_digest_lacks_fails(self):
        self.assertEqual(len(self.run_check(digest=self.DIGEST.replace("| `specs/README.md` | `project:spec` | all |\n", ""))), 1)

    def test_a_digest_path_the_contract_lacks_fails(self):
        d = self.DIGEST.replace("`specs/README.md`", "`specs/OTHER.md`")
        self.assertTrue(any("OTHER" in p for p in self.run_check(digest=d)))

    def test_a_different_owner_fails(self):
        d = self.DIGEST.replace("`project:spec`", "`project:init`")
        self.assertTrue(any("owner" in p for p in self.run_check(digest=d)))

    def test_a_missing_status_fails(self):
        d = self.DIGEST.replace("Planned, Verified.", "Planned.")
        self.assertTrue(any("Verified" in p for p in self.run_check(digest=d)))

    def test_a_pointer_to_no_heading_fails(self):
        d = self.DIGEST.replace("Rules every stage follows", "Nowhere")
        self.assertTrue(any("Nowhere" in p for p in self.run_check(digest=d)))

    def test_a_pointer_to_no_rule_fails(self):
        self.assertTrue(any("#9" in p for p in self.run_check(digest=self.DIGEST.replace("#2", "#9"))))

    def test_the_repository_agrees(self):
        self.assertEqual(consistency.check_pipeline_digest(consistency.ROOT), [])


    def test_the_contract_applicability_section_must_be_in_the_digest(self):
        full = self.FULL + "\n## Applicability: x\n\nrows\n"
        full = full.replace("## Applicability: x", "## Applicability")
        self.assertTrue(any("Applicability" in p for p in self.run_check(full=full)))
        d = self.DIGEST.replace("## Statuses", "## Applicability\n\n| Artifact | Status | Reason |\nN/A\n\n## Statuses")
        self.assertEqual(self.run_check(full=full, digest=d), [])


class ApplicabilityRows(unittest.TestCase):
    MODEL = "export const A = [\n  { path: X, label: 'contract', stage: 's' },\n  { path: Y, label: 'delivery', stage: 't' },\n]\n"

    def run_check(self, rows):
        text = "# Specs\n\n## Applicability\n\n| Artifact | Status | Reason |\n|---|---|---|\n" + rows + "\n## Next\n\n| bogus | row |\n"
        root = tree({consistency.PIPELINE_MODEL: self.MODEL, "specs/README.md": text})
        return consistency.check_applicability(root)

    def test_a_valid_row_passes(self):
        self.assertEqual(self.run_check("| `Contract` | N/A | no HTTP API |\n"), [])

    def test_no_section_passes(self):
        root = tree({consistency.PIPELINE_MODEL: self.MODEL, "specs/README.md": "# Specs\n"})
        self.assertEqual(consistency.check_applicability(root), [])

    def test_an_unknown_artifact_fails(self):
        self.assertTrue(any("nonsense" in p for p in self.run_check("| nonsense | N/A | x |\n")))

    def test_another_status_fails(self):
        self.assertTrue(any("N/A" in p for p in self.run_check("| contract | Pending | x |\n")))

    def test_an_empty_reason_fails(self):
        self.assertTrue(any("reason" in p for p in self.run_check("| contract | N/A |  |\n")))
        self.assertTrue(any("reason" in p for p in self.run_check("| contract | N/A |\n")))

    def test_the_repository_agrees(self):
        self.assertEqual(consistency.check_applicability(consistency.ROOT), [])


class StageNames(unittest.TestCase):
    MODEL = ("  {{ path: BRIEF, label: 'brief', stage: '/project:init', after: [] }},\n"
             "  {{ path: 'a.md', label: 'a', stage: '/project:architecture', after: [] }},\n"
             "export const STAGE_NAME: Readonly<Record<string, string>> = {{\n{names}}}\n")

    def problems(self, names):
        body = "".join(f"  '{n}': 'x',\n" for n in names)
        return consistency.check_stage_names(tree({consistency.PIPELINE_MODEL: self.MODEL.format(names=body)}))

    def test_every_stage_named_passes(self):
        self.assertEqual(self.problems(["/project:init", "/project:architecture"]), [])

    def test_a_stage_without_a_name_fails(self):
        problems = self.problems(["/project:init"])
        self.assertEqual(len(problems), 1)
        self.assertIn("/project:architecture", problems[0])

    def test_the_repository_agrees(self):
        self.assertEqual(consistency.check_stage_names(consistency.ROOT), [])


class GuardUx(unittest.TestCase):
    SHARED = ("export const GUARDS = {\n  git: {\n    marker: 'git:workflow guard:',\n    skill: 'git:workflow',\n    entries: [\n"
              "      { match: 'hooks must not be bypassed', say: 'x' },\n    ],\n  },\n}\n")
    GUARD = 'decide deny "hooks must not be bypassed (--no-verify); fix it"\n'

    def files(self, shared=SHARED, guard=GUARD, copy=None):
        files = {consistency.GUARD_UX: shared, "plugins/git/hooks/guard.sh": guard}
        for plugin in consistency.SHARED_CODE[consistency.GUARD_UX]:
            files[f"plugins/{plugin}/hooks/guard-ux.ts"] = shared if copy is None else copy
        return files

    def problems(self, **kw):
        # only the git guard has a guard.sh in these throwaway trees
        return consistency.check_guard_ux(tree(self.files(**kw)))

    def test_identical_copies_and_a_covered_reason_pass(self):
        self.assertEqual(self.problems(), [])

    def test_a_copy_that_differs_fails(self):
        problems = self.problems(copy=self.SHARED + "// edited\n")
        self.assertEqual(len(problems), len(consistency.SHARED_CODE[consistency.GUARD_UX]))
        self.assertIn("--sync-shared-code", problems[0])

    def test_a_band_priority_copy_that_differs_fails(self):
        files = self.files()
        files["shared/band-priority.ts"] = "export const a = 1\n"
        files["plugins/project/hooks/band-priority.ts"] = "export const a = 1\n"
        files["plugins/git/hooks/band-priority.ts"] = "export const a = 2\n"
        files["plugins/repo/hooks/band-priority.ts"] = "export const a = 1\n"
        files["plugins/devcontainer/hooks/band-priority.ts"] = "export const a = 1\n"
        problems = consistency.check_guard_ux(tree(files))
        self.assertEqual(len(problems), 1)
        self.assertIn("plugins/git/hooks/band-priority.ts differs from shared/band-priority.ts", problems[0])

    def test_the_repo_band_priority_copy_is_checked_too(self):
        files = self.files()
        files["shared/band-priority.ts"] = "export const a = 1\n"
        files["plugins/project/hooks/band-priority.ts"] = "export const a = 1\n"
        files["plugins/git/hooks/band-priority.ts"] = "export const a = 1\n"
        files["plugins/repo/hooks/band-priority.ts"] = "export const a = 2\n"
        files["plugins/devcontainer/hooks/band-priority.ts"] = "export const a = 1\n"
        problems = consistency.check_guard_ux(tree(files))
        self.assertEqual(len(problems), 1)
        self.assertIn("plugins/repo/hooks/band-priority.ts differs", problems[0])

    def test_the_devcontainer_band_priority_copy_is_checked_too(self):
        files = self.files()
        files["shared/band-priority.ts"] = "export const a = 1\n"
        for plugin in ("project", "git", "repo"):
            files[f"plugins/{plugin}/hooks/band-priority.ts"] = "export const a = 1\n"
        files["plugins/devcontainer/hooks/band-priority.ts"] = "export const a = 2\n"
        problems = consistency.check_guard_ux(tree(files))
        self.assertEqual(len(problems), 1)
        self.assertIn("plugins/devcontainer/hooks/band-priority.ts differs", problems[0])

    def test_a_reason_with_no_entry_fails(self):
        problems = self.problems(guard='decide ask "a brand new question"\ndecide deny "hooks must not be bypassed"\n')
        self.assertEqual(len(problems), 1)
        self.assertIn("a brand new question", problems[0])

    def test_reasons_come_from_decide_lines_variables_and_printf_arguments(self):
        text = 'creds_reason="that is a creds file"\ndecide deny "$creds_reason"\ndecide ask "plain"\nprintf x \\\n    "from printf"\n'
        self.assertEqual(consistency.guard_reasons(text), ["that is a creds file", "plain", "from printf"])

    def test_the_repository_agrees(self):
        self.assertEqual(consistency.check_guard_ux(consistency.ROOT), [])


class ReleaseSubjects(unittest.TestCase):
    MODEL = ("export const RELEASE_ON_MAIN = /^chore\\(release\\): (\\d+\\.\\d+\\.\\d+)$/\n"
             "export const RELEASE_BACK_INTO_DEV = /^chore\\(release\\): merge (\\d+\\.\\d+\\.\\d+) back into (?:dev|develop)$/\n")
    JOB = 'git commit -m "chore(release): $version"\ngit merge --no-ff -m "chore(release): merge $version back into $integ" "$version"\n'
    DOC = 'git commit -m "chore(release): $version"\ngit merge --no-ff -m "chore(release): merge X.Y.Z back into <integ>" X.Y.Z\n'

    def files(self, job=JOB, doc=DOC, model=MODEL):
        return {consistency.HYGIENE_MODEL: model, ".github/workflows/release.yml": job, "shared/release-platforms.md": doc}

    def test_the_job_and_the_template_match_the_patterns(self):
        self.assertEqual(consistency.check_release_subjects(tree(self.files())), [])

    def test_a_changed_subject_fails(self):
        problems = consistency.check_release_subjects(tree(self.files(job='git commit -m "chore(release): v$version"\ngit merge -m "chore(release): merge $version back into $integ"\n')))
        self.assertEqual(len(problems), 2)

    def test_a_missing_merge_back_fails(self):
        problems = consistency.check_release_subjects(tree(self.files(job='git commit -m "chore(release): $version"\n')))
        self.assertEqual(len(problems), 1)
        self.assertIn("merge back", problems[0])

    def test_the_repository_agrees(self):
        self.assertEqual(consistency.check_release_subjects(consistency.ROOT), [])


class Cards(unittest.TestCase):
    def test_every_card_has_a_projection(self):
        ok = "# Cards\n\n## Card `a`: T001\n\n**Route**: Effort `low` · Tokens: projected ~100k | used —\n"
        self.assertEqual(consistency.check_cards(tree({CARDS: ok})), [])

    def test_card_without_a_projection_fails(self):
        bad = ("# Cards\n\n## Card `a`: T001\n\n**Route**: Tokens: projected ~1k\n\n"
               "## Card `b`: T002\n\n**Route**: Effort `low`\n")
        problems = consistency.check_cards(tree({CARDS: bad}))
        self.assertEqual(len(problems), 1)
        self.assertIn("cards.md:7", problems[0])

    def test_fenced_examples_and_other_headings_are_not_cards(self):
        text = "# Cards\n\n```markdown\n## Card `x`\n```\n\n## Notes\n\nnothing\n"
        self.assertEqual(consistency.check_cards(tree({CARDS: text})), [])

    def test_template_must_show_the_line(self):
        ref = "plugins/agents/skills/orchestrate/references/cards.md"
        self.assertEqual(len(consistency.check_cards(tree({ref: "# Cards\n\n## Template\n"}))), 1)
        self.assertEqual(consistency.check_cards(tree({ref: "# Cards\n\nTokens: projected ~420k\n"})), [])  # no vocab: tags unchecked
        both = {ref: "# Cards\n\nTags: extend, rust · Tokens: projected ~420k\n", "shared/token-tags.json": VOCAB_JSON}
        self.assertEqual(consistency.check_cards(tree(both)), [])
        both[ref] = "# Cards\n\nTokens: projected ~420k\n"
        self.assertEqual(len(consistency.check_cards(tree(both))), 1)

    def test_worktrees_are_not_linted(self):
        self.assertEqual(consistency.check_cards(tree({".worktrees/x/" + CARDS: "## Card `a`\n"})), [])


VOCAB_JSON = json.dumps({"facets": {
    "kind": {"required": True, "multi": False, "values": {"extend": "x", "fix": "x"}},
    "lang": {"required": True, "multi": False, "values": {"rust": "x"}},
    "stack": {"required": False, "multi": True, "values": {"tokio": "rust", "react": "typescript"}, "aliases": {"react-dom": "react"}}}})
VOCAB_DOC = "`extend` `fix` `rust` `tokio` `react`"


class Tags(unittest.TestCase):
    def files(self, line, extra=None):
        card = f"# Cards\n\n## Card `a`: T001\n\n**Route**: {line} · Tokens: projected ~1k\n"
        return {CARDS: card, "shared/token-tags.json": VOCAB_JSON, "shared/token-tags.md": VOCAB_DOC, **(extra or {})}

    def problems(self, line):
        return consistency.check_cards(tree(self.files(line)))

    def test_one_value_per_facet_passes_stack_optional_and_aliased(self):
        self.assertEqual(self.problems("Tags: extend, rust"), [])
        self.assertEqual(self.problems("Tags: fix, rust, tokio, react-dom"), [])

    def test_missing_unknown_and_duplicate_facets_fail(self):
        self.assertIn("no lang tag", self.problems("Tags: extend")[0])
        self.assertIn("unknown tag 'cobol'", self.problems("Tags: extend, rust, cobol")[0])
        self.assertIn("2 kind tags", self.problems("Tags: extend, fix, rust")[0])
        self.assertIn("no `Tags:` line", self.problems("Effort `low`")[0])

    def test_vocabulary_values_must_be_documented_and_unique(self):
        ok = consistency.check_vocab(tree({"shared/token-tags.json": VOCAB_JSON, "shared/token-tags.md": VOCAB_DOC}))
        self.assertEqual(ok, [])
        missing = consistency.check_vocab(tree({"shared/token-tags.json": VOCAB_JSON, "shared/token-tags.md": "`extend`"}))
        self.assertTrue(any("'tokio' (stack) isn't documented" in m for m in missing))
        dup = json.loads(VOCAB_JSON)
        dup["facets"]["lang"]["values"]["fix"] = "x"
        self.assertTrue(any("'fix' is in both" in m for m in consistency.check_vocab(
            tree({"shared/token-tags.json": json.dumps(dup), "shared/token-tags.md": VOCAB_DOC + " `fix`"}))))


SPEC_DM = '## `--json` shape\n\n```json\n{"checked": true, "project": "p", "findings": [\n{"kind": "disk", "free_gb": 1, "floor_gb": 2, "store": "/"},\n{"kind": "dangling", "count": 2}]}\n{"checked": false, "reason": "no-engine"}\n```\n'
SCRIPT = (
    "#!/bin/sh\n# comment with \"ignored\":\n"
    "printf '{\"checked\":false,\"reason\":\"%s\"}' x\n"
    "printf '{\"checked\":true,\"project\":\"%s\",\"findings\":[%s]}' p f\n"
    "add \"{\\\"kind\\\":\\\"disk\\\",\\\"free_gb\\\":$a,\\\"floor_gb\\\":$b,\\\"store\\\":\\\"s\\\"}\"\n"
    "add \"{\\\"kind\\\":\\\"dangling\\\",\\\"count\\\":$n}\"\n"
)


class LeftoversJson(unittest.TestCase):
    files = {consistency.LEFTOVERS: SCRIPT, consistency.LEFTOVERS_SPEC: SPEC_DM}

    def test_matching_passes(self):
        self.assertEqual(consistency.check_leftovers_json(tree(self.files)), [])

    def test_renamed_key_fails(self):
        files = {**self.files, consistency.LEFTOVERS: SCRIPT.replace("floor_gb", "floor")}
        self.assertTrue(consistency.check_leftovers_json(tree(files)))

    def test_unknown_kind_fails(self):
        files = {**self.files, consistency.LEFTOVERS: SCRIPT + 'add "{\\"kind\\":\\"extra\\",\\"count\\":1}"\n'}
        self.assertTrue(consistency.check_leftovers_json(tree(files)))

    def test_spec_key_missing_from_script_fails(self):
        files = {**self.files, consistency.LEFTOVERS_SPEC: SPEC_DM.replace('"store": "/"', '"store": "/", "extra": 1')}
        self.assertTrue(consistency.check_leftovers_json(tree(files)))


class ReleaseJson(unittest.TestCase):
    PY = 'JSON_KEYS = ("version", "bump")\nTARGET_KEYS = ("kind", "label")\n'

    def model(self, keys="'version', 'bump'", target="'kind', 'label'"):
        return f"export const JSON_KEYS = [{keys}] as const\nexport const TARGET_KEYS = [{target}] as const\n"

    def files(self, model):
        return {"scripts/release.py": self.PY, "shared/release-single.py": self.PY, consistency.RELEASE_MODEL: model}

    def test_equal_lists_pass(self):
        self.assertEqual(consistency.check_release_json(tree(self.files(self.model()))), [])

    def test_a_model_key_list_that_differs_fails(self):
        self.assertEqual(len(consistency.check_release_json(tree(self.files(self.model(keys="'version'"))))), 1)
        self.assertEqual(len(consistency.check_release_json(tree(self.files(self.model(target="'kind'"))))), 1)

    def test_a_model_without_the_lists_fails(self):
        self.assertEqual(len(consistency.check_release_json(tree(self.files("export const X = 1\n")))), 1)

    def test_the_repository_agrees(self):
        self.assertEqual(consistency.check_release_json(consistency.ROOT), [])


class Research(unittest.TestCase):
    PTR = "Method: `../../references/research.md`.\n"

    def linked(self, files, link=True, target="shared/research.md"):
        root = tree({**files, "shared/research.md": "# x\n"})
        if link:
            ref = root / "plugins/p/references"
            ref.mkdir(parents=True, exist_ok=True)
            os.symlink("../../../" + target, ref / "research.md")
        return root

    def test_pointer_with_symlink_passes(self):
        root = self.linked({"plugins/p/skills/s/SKILL.md": self.PTR})
        self.assertEqual(consistency.check_research_links(root), [])

    def test_pointer_without_symlink_fails(self):
        root = self.linked({"plugins/p/skills/s/SKILL.md": self.PTR}, link=False)
        self.assertEqual(len(consistency.check_research_links(root)), 1)

    def test_symlink_to_the_wrong_file_fails(self):
        root = self.linked({"plugins/p/skills/s/SKILL.md": self.PTR, "shared/other.md": "x\n"}, target="shared/other.md")
        self.assertEqual(len(consistency.check_research_links(root)), 1)

    def test_a_skill_own_research_reference_is_not_a_pointer(self):
        root = self.linked({"plugins/p/skills/s/SKILL.md": "See `references/research.md`.\n"}, link=False)
        self.assertEqual(consistency.check_research_links(root), [])

    def test_deny_listed_skill_with_pointer_fails(self):
        root = tree({"plugins/security/skills/scope/SKILL.md": self.PTR})
        self.assertEqual(len(consistency.check_research_denied(root)), 1)

    def test_deny_listed_skill_without_pointer_passes(self):
        root = tree({"plugins/security/skills/scope/SKILL.md": "Record the scope.\n"})
        self.assertEqual(consistency.check_research_denied(root), [])

    def test_every_deny_entry_has_a_reason(self):
        self.assertTrue(all(consistency.RESEARCH_DENY.values()))

    def test_labels(self):
        ok = tree({"shared/a.md": "x [RESEARCHED: S3] y [KNOWN] z [ASSUMPTION] [ASSUMPTION: why] [RESEARCHED: S<n>]\n"})
        self.assertEqual(consistency.check_research_labels(ok), [])
        for bad in ("[ASSUMED]", "[VERIFIED]", "[RESEARCHED]", "[RESEARCHED: 3]", "[KNOWN: S1]"):
            with self.subTest(bad=bad):
                self.assertEqual(len(consistency.check_research_labels(tree({"shared/a.md": bad + "\n"}))), 1)

    def test_method_sections(self):
        full = "\n".join(f"## {h}" for h in ("Research, ask or know", "Search recipes", "Source quality", "Citing and labels", "Safety", "Offline and thin results")) + "\nBudgets are soft.\n"
        self.assertEqual(consistency.check_research_method(tree({"shared/research.md": full})), [])
        problems = consistency.check_research_method(tree({"shared/research.md": full.replace("## Safety", "## Other")}))
        self.assertEqual(len(problems), 1)
        self.assertIn("safety", problems[0])

    def test_the_repository_agrees(self):
        for check in (consistency.check_research_links, consistency.check_research_denied, consistency.check_research_labels, consistency.check_research_method):
            self.assertEqual(check(consistency.ROOT), [], check.__name__)


PLUGIN = '{"name": "git", "license": "GPL-3.0-or-later"}'
WS = '[workspace]\nmembers = ["crates/a"]\n[workspace.package]\nlicense = "GPL-3.0-or-later"\n'


class RustWorkspace(unittest.TestCase):
    def files(self, crate: str, extra: dict[str, str] | None = None) -> Path:
        return tree({"plugins/git/.claude-plugin/plugin.json": PLUGIN, "rust/Cargo.toml": WS, "rust/crates/a/Cargo.toml": crate, **(extra or {})})

    def test_no_workspace_is_fine(self):
        self.assertEqual(consistency.check_rust_workspace(tree({"README.md": "x"})), [])

    def test_inherited_and_literal_license_pass(self):
        self.assertEqual(consistency.check_rust_workspace(self.files('[package]\nname = "a"\nlicense.workspace = true\n')), [])
        self.assertEqual(consistency.check_rust_workspace(self.files('[package]\nname = "a"\nlicense = "GPL-3.0-or-later"\n')), [])

    def test_other_license_fails(self):
        problems = consistency.check_rust_workspace(self.files('[package]\nname = "a"\nlicense = "MIT"\n'))
        self.assertEqual(len(problems), 1)
        self.assertIn("license is 'MIT'", problems[0])

    def test_rust_is_not_a_plugin(self):
        root = self.files('[package]\nname = "a"\nlicense.workspace = true\n', {"rust/skills/x/SKILL.md": "no frontmatter"})
        self.assertEqual(len(consistency.check_rust_workspace(root)), 1)
        # and the skill checks never read rust/ (they would fail on this file)
        self.assertEqual(consistency.check_budget(root)[0], [])

    def test_the_repository_agrees(self):
        self.assertEqual(consistency.check_rust_workspace(consistency.ROOT), [])


class Budget(unittest.TestCase):
    def test_ceiling(self):
        root = tree({"skills/a/SKILL.md": "---\nname: a\ndescription: " + "x" * 100 + "\n---\n"})
        os.environ["DESC_TOTAL_CEILING"] = "99"
        try:
            self.assertEqual(len(consistency.check_budget(root)[0]), 1)
            os.environ["DESC_TOTAL_CEILING"] = "100"
            self.assertEqual(consistency.check_budget(root)[0], [])
        finally:
            del os.environ["DESC_TOTAL_CEILING"]


class GuardReasons(unittest.TestCase):
    UX = "marker: 'repo guard:',\n    entries: [\n      { match: 'sign-off' },\n    ],\n"
    GUARD = '#!/bin/sh\ndecide deny "no sign-off here"\n'

    def files(self, text):
        return {
            consistency.GUARD_UX: self.UX,
            "plugins/repo/hooks/guard.sh": self.GUARD,
            consistency.REASONS_JSON: json.dumps([{"id": "repo.attrib", "guard": "repo", "marker": "repo guard:", "decision": "deny", "text": text}]),
        }

    def test_equal_is_clean(self):
        root = tree(self.files("no sign-off here"))
        self.assertEqual(consistency.check_guard_reasons(root, consistency.guard_matches(root)), [])

    def test_a_reason_the_script_does_not_print_fails(self):
        root = tree(self.files("a sign-off other text"))
        problems = consistency.check_guard_reasons(root, consistency.guard_matches(root))
        self.assertEqual(len(problems), 2, problems)

    def test_a_reason_without_a_ux_entry_fails(self):
        root = tree(self.files("no sign-off here"))
        (root / consistency.GUARD_UX).write_text("marker: 'repo guard:',\n    entries: [\n      { match: 'other' },\n    ],\n")
        problems = consistency.check_guard_reasons(root, consistency.guard_matches(root))
        self.assertTrue(any("no entry" in p for p in problems), problems)


if __name__ == "__main__":
    unittest.main()
