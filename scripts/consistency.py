#!/usr/bin/env python3
"""Consistency checks for the prose that scripts/check.sh can't see.

  1. `plugin:skill` followed by an *italic section* names a heading (or a bold
     lead-in) in that skill or its references.
  2. Phrasings the toolkit retired don't come back.
  3. The skill descriptions every session loads stay under a ceiling.
  4. A skill that tells an agent to create container resources (volumes,
     tools images, compose stacks, worktrees) also says how they're removed
     (`containers.md` §9/§9a, `wt.py remove`, the `clean` recipe).
  5. No prune is taught as a command: a line naming an engine and a
     `prune` must also say "never" (shared/containers.md §9).
  6. Every card in a cards.md (outside code fences) carries a
     `Tokens: projected` line and a `Tags:` line with one valid value per
     facet (lang required, stack optional), and the card template shows both;
     every tag value is documented (shared/token-tags.md, token-tracking.md).
  7. The secrets mod's tests can read only code files, so they import
     `plugins/secrets/hooks/secret-data.ts`, a generated copy of
     shared/secret-patterns.json and shared/secret-cases.json. It matches
     them exactly (`--sync-secret-data` rewrites it).
  8. Every artifact path the project plugin's pipeline pane lists
     (hooks/pipeline-model.ts) is in shared/pipeline.md's artifact table.
     Also shared/pipeline-digest.md (the short form skills read first) lists
     every artifact and owner of that table and nothing it lacks, names every
     feature status, and each `§"heading" #rule` pointer resolves.
  9. Shared code a plugin's hooks module imports (shared/guard-ux.ts,
     shared/hook-lines.ts) is COPIED into the plugins that use it, because the
     engine refuses an import through a symlink that leaves the plugin. The
     copies are identical (`--sync-shared-code` rewrites them), and every
     reason a guard.sh can print has an entry in guard-ux's table.
 10. The git plugin's repo-hygiene band recognizes a release by the subjects
     .github/workflows/release.yml and shared/release-platforms.md write
     (`chore(release): X.Y.Z`, `chore(release): merge X.Y.Z back into dev`).
 11. The JSON keys and finding kinds `leftovers.sh --json` prints are the ones
     specs/025-container-disk-band/data-model.md documents (the band reads them).

 12. Research (shared/research.md): a plugin whose skills point at the shared
     method has the symlink; the offline deny-list skills don't point at it;
     the labels in prose are only [RESEARCHED: S<n>], [KNOWN], [ASSUMPTION];
     the method file keeps its required sections.

 13. The Rust workspace (rust/, spec 032) is not a plugin (no .claude-plugin,
     no skills/) and every crate's license is the one the plugins declare.

Run from check.sh; prints one FAIL line per problem and exits 1, else 0.
Environment: DESC_TOTAL_CEILING, the total characters of all descriptions
(default 19000, room for growth; raise it deliberately with a new plugin,
never to silence a bloated single description).
"""
import json
import os
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROSE_DIRS = ("plugins", "shared")
PROSE_FILES = ("README.md", "AGENTS.md")

# A skill reference, then (optionally "→", ",", "says (") an *italic section*.
SECTION_REF = re.compile(r"`([a-z]+:[a-z-]+)`(?:\s*(?:→|,|says\s*\()\s*|\s+)\*([A-Za-z][^*\n]{1,60})\*")

# Retired phrasings: (pattern, what to say instead).
RETIRED = (
    (r"\b3[–-]5 questions\b", "ask first has no fixed count (pipeline rule 2)"),
    (r"\bone question at a time\b", "ask up to four per prompt (pipeline rule 2)"),
    (r"\bat most 5 questions\b", "ask up to four per prompt (pipeline rule 2)"),
    (r"\b5 questions (tops|at most|max)\b", "ask up to four per prompt (pipeline rule 2)"),
    (r"\b\d+ (skills|plugins)\b", "no hardcoded counts in copy; they drift"),
)


def prose_files(root: Path):
    for d in PROSE_DIRS:
        for p in sorted((root / d).rglob("*.md")):
            if "/evals/" not in p.as_posix():
                yield p
    for name in PROSE_FILES:
        if (root / name).is_file():
            yield root / name


def headings(skill_dir: Path) -> list[str]:
    """Headings and bold lead-ins of a skill's SKILL.md and references/*.md."""
    found: list[str] = []
    files = [skill_dir / "SKILL.md", *sorted((skill_dir / "references").rglob("*.md"))]
    for f in files:
        if not f.is_file():
            continue
        for line in f.read_text().splitlines():
            m = re.match(r"#+\s+(.*)", line) or re.match(r"\s*(?:[-*]\s+)?\*\*(.+?)\*\*", line)
            if m:
                found.append(m.group(1).strip().lower())
    return found


def check_sections(root: Path) -> list[str]:
    problems = []
    cache: dict[str, list[str]] = {}
    for f in prose_files(root):
        text = f.read_text()
        for m in SECTION_REF.finditer(text):
            ref, section = m.group(1), m.group(2).strip().lower().rstrip(".,")
            plugin, skill = ref.split(":")
            skill_dir = root / "plugins" / plugin / "skills" / skill
            if not skill_dir.is_dir():
                continue  # a missing skill is the cross-reference check's finding
            heads = cache.setdefault(ref, headings(skill_dir))
            if not any(section in h for h in heads):
                line = text.count("\n", 0, m.start()) + 1
                problems.append(f"{f.relative_to(root)}:{line}: {ref} has no section '{m.group(2).strip()}'")
    return problems


def check_retired(root: Path) -> list[str]:
    problems = []
    for f in prose_files(root):
        for n, line in enumerate(f.read_text().splitlines(), 1):
            for pattern, instead in RETIRED:
                if re.search(pattern, line, re.IGNORECASE):
                    problems.append(f"{f.relative_to(root)}:{n}: retired phrasing /{pattern}/ — {instead}")
    return problems


# 4. What creates container resources, and what says how they go.
CREATES = re.compile(r"volume create|compose[^`\n]{0,40}\bup\b|\bbuild (--target|-t)\b|named (cache )?volumes?|wt\.py add", re.I)
CLEANS = re.compile(r"containers\.md`? §9a?|wt\.py remove|`clean`", re.I)
# 5. A prune written as a command.
PRUNE = re.compile(r"(podman|docker|nerdctl|buildah|CONTAINER_ENGINE)[^\n]*\b(system|image|volume|container|builder|network) prune\b")


def check_cleanup(root: Path) -> list[str]:
    problems = []
    for skill in sorted(root.glob("plugins/*/skills/*")):
        files = [skill / "SKILL.md", *sorted((skill / "references").rglob("*.md"))]
        text = "\n".join(f.read_text() for f in files if f.is_file() and not f.is_symlink())
        m = CREATES.search(text)
        if m and not CLEANS.search(text):
            problems.append(f"{skill.relative_to(root)}: creates container resources ('{m.group(0)}') "
                            f"but never says how they're removed (containers.md §9a, wt.py remove, `clean`)")
    return problems


def check_prune(root: Path) -> list[str]:
    problems = []
    for f in prose_files(root):
        for n, line in enumerate(f.read_text().splitlines(), 1):
            if PRUNE.search(line) and not re.search(r"\bnever\b", line, re.I):
                problems.append(f"{f.relative_to(root)}:{n}: a prune taught as a command — say never, "
                                f"or remove by ID or label (containers.md §9)")
    return problems


VOCAB = "shared/token-tags.json"
TAGS_LINE = re.compile(r"Tags:\s*([^·|`\n]*)")


def load_vocab(root: Path) -> dict:
    import json
    return json.loads((root / VOCAB).read_text())["facets"] if (root / VOCAB).is_file() else {}


def tag_problems(text: str, vocab: dict) -> list[str]:
    """Why a `Tags:` list is not one valid value per facet (stack: any number, aliases allowed)."""
    flat = {v: f for f, d in vocab.items() for v in d["values"]}
    flat.update({a: "stack" for a in vocab.get("stack", {}).get("aliases", {})})
    seen: dict[str, list[str]] = {}
    problems = []
    for tag in [t.strip() for t in text.split(",") if t.strip()]:
        facet = flat.get(tag)
        if facet is None:
            problems.append(f"unknown tag '{tag}' (shared/token-tags.json)")
        else:
            seen.setdefault(facet, []).append(tag)
    for facet, d in vocab.items():
        n = len(seen.get(facet, []))
        if d["required"] and n == 0:
            problems.append(f"no {facet} tag")
        if not d["multi"] and n > 1:
            problems.append(f"{n} {facet} tags ({', '.join(seen[facet])}), want one")
    return problems


def check_vocab(root: Path) -> list[str]:
    """Every tag value is documented in shared/token-tags.md and belongs to one facet."""
    vocab = load_vocab(root)
    if not vocab:
        return []
    doc = (root / "shared/token-tags.md").read_text() if (root / "shared/token-tags.md").is_file() else ""
    problems, owner = [], {}
    for facet, d in vocab.items():
        for v in d["values"]:
            if v in owner:
                problems.append(f"{VOCAB}: '{v}' is in both {owner[v]} and {facet}")
            owner[v] = facet
            if f"`{v}`" not in doc:
                problems.append(f"shared/token-tags.md: '{v}' ({facet}) isn't documented")
        for a, target in d.get("aliases", {}).items():
            if target not in d["values"]:
                problems.append(f"{VOCAB}: alias '{a}' points to unknown '{target}'")
    return problems


CARD_HEAD = re.compile(r"^## Card\b.*$")
SKIP_DIRS = {".git", ".worktrees", "node_modules", "dist"}


def card_files(root: Path):
    for p in sorted(root.rglob("cards.md")):
        if not SKIP_DIRS & set(p.relative_to(root).parts):
            yield p


def check_cards(root: Path) -> list[str]:
    """Each '## Card' section of a cards.md names its projection; the template does too."""
    problems = []
    for f in card_files(root):
        rel = f.relative_to(root)
        sections: list[tuple[int, str, list[str]]] = []
        fenced = False
        for n, line in enumerate(f.read_text().splitlines(), 1):
            if line.lstrip().startswith("```"):
                fenced = not fenced
            elif not fenced and line.startswith("## "):
                sections.append((n, line, []) if CARD_HEAD.match(line) else (n, "", []))
            if sections:
                sections[-1][2].append(line)
        vocab = load_vocab(root)
        for n, head, body in sections:
            if not head:
                continue
            text = "\n".join(body)
            if "Tokens: projected" not in text:
                problems.append(f"{rel}:{n}: {head[:60]} has no `Tokens: projected` line "
                                f"(estimate it: token_report.py estimate)")
            m = TAGS_LINE.search(text)
            if vocab and not m:
                problems.append(f"{rel}:{n}: {head[:60]} has no `Tags:` line (shared/token-tags.md)")
            elif vocab:
                for why in tag_problems(m.group(1), vocab):
                    problems.append(f"{rel}:{n}: {head[:60]} Tags: {why}")
        if "/references/" in rel.as_posix():
            body = f.read_text()
            if "Tokens: projected" not in body or (vocab and "Tags:" not in body):
                problems.append(f"{rel}: the card template doesn't show `Tags:` and `Tokens: projected` on the route line")
    return problems


def description_total(root: Path) -> int:
    total = 0
    for f in sorted(root.glob("skills/*/SKILL.md")) + sorted(root.glob("plugins/*/skills/*/SKILL.md")):
        for line in f.read_text().splitlines():
            if line.startswith("description: "):
                total += len(line) - len("description: ")
                break
    return total


def check_budget(root: Path) -> tuple[list[str], int]:
    ceiling = int(os.environ.get("DESC_TOTAL_CEILING", "19000"))
    total = description_total(root)
    problems = []
    if total > ceiling:
        problems.append(f"skill descriptions total {total} characters, over the {ceiling} ceiling (every session loads them)")
    return problems, total


SECRET_DATA = "plugins/secrets/hooks/secret-data.ts"
SECRET_SOURCES = (("patterns", "shared/secret-patterns.json"), ("cases", "shared/secret-cases.json"))


def secret_data_text(root: Path) -> str:
    """The generated twin of the shared secret files, as the module tests import it."""
    out = [
        "// Generated by `python3 scripts/consistency.py --sync-secret-data` from",
        "// shared/secret-patterns.json and shared/secret-cases.json: a test can import only",
        "// code files, so it gets the shared data here. scripts/consistency.py fails if",
        "// this file differs from them. Edit the JSON, not this file.",
        "",
    ]
    for name, rel in SECRET_SOURCES:
        data = json.loads((root / rel).read_text(encoding="utf-8"))
        out.append(f"export const {name} = {json.dumps(data, indent=2, ensure_ascii=False)}\n")
    return "\n".join(out)


def check_secret_data(root: Path) -> list[str]:
    twin = root / SECRET_DATA
    if not all((root / rel).is_file() for _, rel in SECRET_SOURCES):
        return []
    if not twin.is_file() or twin.read_text(encoding="utf-8") != secret_data_text(root):
        return [f"{SECRET_DATA} differs from shared/secret-patterns.json and shared/secret-cases.json (run python3 scripts/consistency.py --sync-secret-data)"]
    return []


PIPELINE_MODEL = "plugins/project/hooks/pipeline-model.ts"


def check_pipeline_paths(root: Path) -> list[str]:
    """The pane's artifact list is a subset of the pipeline contract's table."""
    model, table = root / PIPELINE_MODEL, root / "shared/pipeline.md"
    if not model.is_file() or not table.is_file():
        return []
    paths = re.findall(r"^\s*\{ path: ([A-Z_]+|'[^']+')", model.read_text(encoding="utf-8"), re.M)
    consts = dict(re.findall(r"^export const ([A-Z_]+) = '([^']+)'", model.read_text(encoding="utf-8"), re.M))
    rows = {m for line in table.read_text(encoding="utf-8").splitlines() if line.startswith("|")
            for m in re.findall(r"`([^`]+)`", line.split("|")[1] if line.count("|") > 1 else "")}
    problems = []
    for raw in paths:
        path = consts.get(raw, raw.strip("'"))
        if path not in rows:
            problems.append(f"{PIPELINE_MODEL}: '{path}' is not in the artifact table of shared/pipeline.md")
    if not paths:
        problems.append(f"{PIPELINE_MODEL}: no artifact paths found (the check reads `{{ path: '...' }}` lines)")
    return problems


def check_applicability(root: Path) -> list[str]:
    """Every row of specs/README.md's `## Applicability` table names an artifact the pane lists, says N/A and gives a reason."""
    model, readme = root / PIPELINE_MODEL, root / "specs/README.md"
    if not model.is_file() or not readme.is_file():
        return []
    known = set(re.findall(r"label: '([^']+)'", model.read_text(encoding="utf-8")))
    section = re.search(r"^## Applicability[ \t]*\n(.*?)(?=^#{1,6} |\Z)", readme.read_text(encoding="utf-8"), re.M | re.S)
    problems = []
    for line in (section.group(1) if section else "").splitlines():
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        name = cells[0].replace("`", "").strip().lower()
        if name in ("", "artifact") or not name.strip("-: "):
            continue
        if name not in known:
            problems.append(f"specs/README.md: Applicability row '{name}' is not an artifact the pane lists ({', '.join(sorted(known))})")
        elif len(cells) < 2 or cells[1].replace("`", "").strip().lower() != "n/a":
            problems.append(f"specs/README.md: Applicability row '{name}' must have the status N/A")
        elif not " | ".join(cells[2:]).strip():
            problems.append(f"specs/README.md: Applicability row '{name}' needs a reason")
    return problems


PIPELINE_DIGEST = "shared/pipeline-digest.md"


def _artifact_rows(text: str) -> dict[str, set[str]]:
    """First backticked path of each table row -> (all col-1 tokens, owner skills); rows keyed by that path."""
    rows: dict[str, set[str]] = {}
    for line in text.splitlines():
        cells = line.split("|")
        if not line.startswith("|") or len(cells) < 3:
            continue
        paths = re.findall(r"`([^`]+)`", cells[1])
        if not paths:
            continue
        rows[paths[0]] = set(paths) | {f"owner:{o}" for o in re.findall(r"`([a-z]+:[a-z-]+)`", cells[2])}
    return rows


def check_pipeline_digest(root: Path) -> list[str]:
    """shared/pipeline-digest.md is derivable from shared/pipeline.md: same artifacts and owners,
    every feature status named, every `§"heading" #rule` pointer resolves."""
    full_p, dig_p = root / "shared/pipeline.md", root / PIPELINE_DIGEST
    if not full_p.is_file() or not dig_p.is_file():
        return []
    full, dig = full_p.read_text(encoding="utf-8"), dig_p.read_text(encoding="utf-8")
    problems = []
    contract, short = _artifact_rows(full), _artifact_rows(dig)
    for key, toks in contract.items():
        mine = short.get(key)
        if mine is None:
            problems.append(f"{PIPELINE_DIGEST}: artifact '{key}' of shared/pipeline.md is missing from the digest table")
            continue
        for tok in sorted(toks - mine):
            problems.append(f"{PIPELINE_DIGEST}: '{key}' lacks {'owner ' + tok[6:] if tok.startswith('owner:') else 'path ' + tok}")
    for key, toks in short.items():
        for tok in sorted(toks):
            name = tok[6:] if tok.startswith("owner:") else tok
            if name not in full:
                problems.append(f"{PIPELINE_DIGEST}: '{name}' (row '{key}') is not in shared/pipeline.md")
    statuses = re.findall(r"^\s+- \*([A-Za-z ]+)\* —", full, re.M)
    section = re.search(r"^## Statuses\n(.*?)(?=^## |\Z)", dig, re.M | re.S)
    body = section.group(1) if section else ""
    for status in statuses:
        if status not in body:
            problems.append(f"{PIPELINE_DIGEST}: status '{status}' of shared/pipeline.md is missing from its Statuses section")
    if not statuses:
        problems.append("shared/pipeline.md: no feature statuses found (the check reads `- *Status* —` lines)")
    headings = set(re.findall(r"^## (.+)$", full, re.M))
    rules = re.search(r"^## Rules every stage follows\n(.*?)(?=^## |\Z)", full, re.M | re.S)
    count = len(re.findall(r"^\d+\. ", rules.group(1), re.M)) if rules else 0
    for heading, num in re.findall(r'§"([^"]+)"(?: #(\d+))?', dig):
        if heading not in headings:
            problems.append(f"{PIPELINE_DIGEST}: pointer §\"{heading}\" is not a heading of shared/pipeline.md")
        elif num and not 1 <= int(num) <= count:
            problems.append(f"{PIPELINE_DIGEST}: pointer §\"{heading}\" #{num} is not a rule (pipeline.md has {count})")
    if "## Applicability" in full:
        app = re.search(r"^## Applicability\n(.*?)(?=^## |\Z)", dig, re.M | re.S)
        if not app or "| Artifact | Status | Reason |" not in app.group(1) or "N/A" not in app.group(1):
            problems.append(f"{PIPELINE_DIGEST}: needs an Applicability section with the `| Artifact | Status | Reason |` table and N/A, as shared/pipeline.md has")
    if "`pipeline.md`" not in dig:
        problems.append(f"{PIPELINE_DIGEST}: must tell the reader to read `pipeline.md` on a conflict")
    return problems


def check_stage_names(root: Path) -> list[str]:
    """Every artifact `stage` in the pane's model has a display name in STAGE_NAME (spec 029)."""
    model = root / PIPELINE_MODEL
    if not model.is_file():
        return []
    text = model.read_text(encoding="utf-8")
    stages = set(re.findall(r"^\s*\{ path: .*?\bstage: '([^']+)'", text, re.M))
    block = re.search(r"STAGE_NAME[^=]*=\s*\{(.*?)^\}", text, re.M | re.S)
    names = set(re.findall(r"^\s*'([^']+)':", block.group(1), re.M)) if block else set()
    problems = [f"{PIPELINE_MODEL}: stage '{s}' has no entry in STAGE_NAME" for s in sorted(stages - names)]
    if not stages:
        problems.append(f"{PIPELINE_MODEL}: no artifact stages found (the check reads `stage: '...'` on `{{ path: ... }}` lines)")
    return problems


GUARD_UX = "shared/guard-ux.ts"
# Shared code a plugin's hooks module imports is COPIED into the plugins that use it
# (the engine refuses an import through a symlink that leaves the plugin).
SHARED_CODE = {
    "shared/guard-ux.ts": ("git", "db", "secrets", "repo", "devcontainer"),
    "shared/hook-lines.ts": ("git", "devcontainer"),
    "shared/band-priority.ts": ("project", "git", "repo", "devcontainer"),
}
GUARD_PLUGINS = {"git": "git:workflow guard:", "db": "db guard:", "secrets": "secrets guard:", "repo": "repo guard:",
                 "devcontainer": "containers guard:"}


def guard_reasons(text: str) -> list[str]:
    """Every reason a guard.sh can print: `decide deny|ask "…"`, its named reason variables, and repo's printf arguments."""
    named = dict(re.findall(r'^(\w+_reason)="((?:[^"\\]|\\.)*)"', text, re.M))
    out = []
    for kind, arg in re.findall(r'decide\s+(deny|ask)\s+("(?:[^"\\]|\\.)*"|\$\w+)', text):
        body = arg.strip('"')
        out.append(named.get(body[1:], "") if re.fullmatch(r"\$\w+", body) else body)
    out += re.findall(r'^\s{4,}"((?:[^"\\]|\\.)*)"\s*$', text, re.M)  # repo guard: the printf's reason argument
    return list(dict.fromkeys(r for r in out if r))


def guard_matches(root: Path) -> dict[str, list[str]]:
    """The `match` fragments of each guard's entries in shared/guard-ux.ts, by marker."""
    src = (root / GUARD_UX).read_text(encoding="utf-8")
    out: dict[str, list[str]] = {}
    for block in re.finditer(r"marker: '([^']+)',.*?entries: \[(.*?)\n    \],", src, re.S):
        out[block[1]] = [m.replace("\\'", "'") for m in re.findall(r"match: '((?:[^'\\]|\\.)*)'", block[2])]
    return out


def check_guard_ux(root: Path) -> list[str]:
    source = root / GUARD_UX
    if not source.is_file():
        return []
    problems = []
    for shared, plugins in SHARED_CODE.items():
        if not (root / shared).is_file():
            continue
        for plugin in plugins:
            copy = root / "plugins" / plugin / "hooks" / Path(shared).name
            if not copy.is_file() or copy.read_bytes() != (root / shared).read_bytes():
                problems.append(f"plugins/{plugin}/hooks/{Path(shared).name} differs from {shared} (run python3 scripts/consistency.py --sync-shared-code)")
    matches = guard_matches(root)
    for plugin, marker in GUARD_PLUGINS.items():
        guard = root / "plugins" / plugin / "hooks" / "guard.sh"
        if not guard.is_file():
            continue
        reasons = guard_reasons(guard.read_text(encoding="utf-8"))
        if not reasons:
            problems.append(f"plugins/{plugin}/hooks/guard.sh: no reasons found (the check reads `decide deny|ask \"…\"`)")
        for reason in reasons:
            if not any(m in reason for m in matches.get(marker, [])):
                problems.append(f"plugins/{plugin}/hooks/guard.sh: '{reason[:70]}…' has no entry in {GUARD_UX}")
    return problems


HYGIENE_MODEL = "plugins/git/hooks/hygiene-model.ts"
RELEASE_SOURCES = (".github/workflows/release.yml", "shared/release-platforms.md")


def check_release_subjects(root: Path) -> list[str]:
    """The band's two subject patterns still match every release subject the release job and its template write."""
    model = root / HYGIENE_MODEL
    if not model.is_file():
        return []
    patterns = dict(re.findall(r"^export const (RELEASE_\w+) = /(.*)/$", model.read_text(encoding="utf-8"), re.M))
    if set(patterns) != {"RELEASE_ON_MAIN", "RELEASE_BACK_INTO_DEV"}:
        return [f"{HYGIENE_MODEL}: RELEASE_ON_MAIN and RELEASE_BACK_INTO_DEV not found (the check reads `export const NAME = /…/`)"]
    main, back = re.compile(patterns["RELEASE_ON_MAIN"]), re.compile(patterns["RELEASE_BACK_INTO_DEV"])
    problems = []
    for rel in RELEASE_SOURCES:
        path = root / rel
        if not path.is_file():
            continue
        samples = [s.replace("$version", "1.2.3").replace("X.Y.Z", "1.2.3").replace("$integ", "dev").replace("<integ>", "dev")
                   for s in re.findall(r'"(chore\(release\): [^"]*)"', path.read_text(encoding="utf-8"))]
        for sample in samples:
            if not (main.match(sample) or back.match(sample)):
                problems.append(f"{rel}: the release subject '{sample}' matches neither pattern in {HYGIENE_MODEL}")
        if not any(main.match(s) for s in samples) or not any(back.match(s) for s in samples):
            problems.append(f"{rel}: no release subject for both the release and its merge back into dev (the band's patterns in {HYGIENE_MODEL} would never see one)")
    return problems


RELEASE_SCRIPTS = ("scripts/release.py", "shared/release-single.py")
RELEASE_MODEL = "plugins/git/hooks/release-model.ts"


def check_release_json(root: Path) -> list[str]:
    """Both release scripts print the same keys for `--dry-run --json`."""
    found: dict[str, tuple] = {}
    problems = []
    for rel in RELEASE_SCRIPTS:
        path = root / rel
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        lists = []
        for name in ("JSON_KEYS", "TARGET_KEYS"):
            m = re.search(rf"^{name} = \(([^)]*)\)", text, re.M)
            lists.append(tuple(re.findall(r'"([^"]+)"', m.group(1))) if m else None)
        if None in lists:
            problems.append(f"{rel}: JSON_KEYS and TARGET_KEYS not found (the check reads `NAME = (\"a\", ...)`)")
        else:
            found[rel] = tuple(lists)
    if len(set(found.values())) > 1:
        problems.append(f"{' and '.join(found)}: the --json key lists differ ({found})")
    model = root / RELEASE_MODEL
    if found and model.is_file():
        text = model.read_text(encoding="utf-8")
        got = []
        for name in ("JSON_KEYS", "TARGET_KEYS"):
            m = re.search(rf"^export const {name} = \[([^\]]*)\]", text, re.M)
            got.append(tuple(re.findall(r"'([^']+)'", m.group(1))) if m else None)
        if None in got:
            problems.append(f"{RELEASE_MODEL}: JSON_KEYS and TARGET_KEYS not found (the check reads `export const NAME = ['a', ...]`)")
        elif tuple(got) != next(iter(found.values())):
            problems.append(f"{RELEASE_MODEL}: its key lists {tuple(got)} differ from the scripts' {next(iter(found.values()))}")
    return problems


LEFTOVERS = "plugins/devcontainer/hooks/leftovers.sh"
LEFTOVERS_SPEC = "specs/025-container-disk-band/data-model.md"


def check_leftovers_json(root: Path) -> list[str]:
    """leftovers.sh --json prints exactly the keys and kinds the spec's data model shows."""
    script, spec = root / LEFTOVERS, root / LEFTOVERS_SPEC
    if not script.is_file() or not spec.is_file():
        return []
    code = script.read_text(encoding="utf-8")
    block = re.search(r"## `--json` shape.*?```json\n(.*?)```", spec.read_text(encoding="utf-8"), re.S)
    if not block:
        return [f"{LEFTOVERS_SPEC}: no `--json` shape block"]
    want_keys = set(re.findall(r'"(\w+)":', block.group(1)))
    want_kinds = set(re.findall(r'"kind": "(\w+)"', block.group(1)))
    emitted = "\n".join(line for line in code.splitlines() if not line.lstrip().startswith("#"))
    got_keys = set(re.findall(r'\\?"(\w+)\\?":', emitted))
    got_kinds = set(re.findall(r'\\"kind\\":\\"(\w+)\\"', emitted))
    problems = []
    for what, got, want in (("keys", got_keys, want_keys), ("kinds", got_kinds, want_kinds)):
        if got != want:
            problems.append(f"{LEFTOVERS}: --json {what} {sorted(got)} differ from {LEFTOVERS_SPEC} {sorted(want)}")
    return problems


# 12. Research. The plugin-level pointer is `../../references/research.md` (or
# three levels up from a skill's references/); a skill's own
# `references/research.md` is a different file and doesn't count.
RESEARCH_POINTER = re.compile(r"(?:\.\./){2,3}references/research\.md")
RESEARCH_SHARED = "shared/research.md"
# Skills that must not point at the research method, with the reason.
RESEARCH_DENY = {
    "security:scope": "records authorization; no research before a scope record exists",
    "security:purple": "retests recorded findings against detections; no new facts",
    "db:connect": "handles connection profiles and credentials; queries would leak names",
    "db:inspect": "reads a live schema; nothing public to look up",
    "db:investigate": "evidence is the data itself; queries would leak it",
    "db:review": "reviews the project's own code and schema; internal facts",
    "secrets:pass": "handles credentials; never sends anything outward",
    "git:workflow": "the user's own rules, not facts that go stale",
    "repo:issues": "tracker operations on private data",
    "repo:pr": "tracker operations on private data",
    "repo:sync": "tracker operations on private data",
    "project:checkpoint": "a hand-off file of the user's own words",
    "project:recap": "read-only report of the local session",
    "project:status": "read-only report of repository artifacts",
    "compact:code": "reads local source; the web adds nothing",
    "devcontainer:workflow": "runs the project's own containers",
    "devsecops:watch": "reads CI logs, which may hold private details",
    "agents:parallel": "git and worktree mechanics",
}
RESEARCH_LABEL = re.compile(r"\[(RESEARCHED|KNOWN|ASSUMPTION|ASSUMED|VERIFIED|UNVERIFIED|SOURCED|CITED|ESTIMATED)\b([^\]\n]*)\]")
RESEARCH_SECTIONS = (
    ("decision table", r"^#+\s+research, ask or know"),
    ("search recipes", r"^#+\s+search recipes"),
    ("source ladder", r"^#+\s+source quality"),
    ("labels", r"^#+\s+citing and labels"),
    ("safety", r"^#+\s+safety"),
    ("offline fallback", r"^#+\s+offline"),
    ("budgets", r"budgets? (are|is) soft"),
)


def skill_texts(skill_dir: Path):
    for f in [skill_dir / "SKILL.md", *sorted((skill_dir / "references").rglob("*.md"))]:
        if f.is_file() and not f.is_symlink():
            yield f, f.read_text(encoding="utf-8")


def check_research_links(root: Path) -> list[str]:
    problems = []
    for plugin in sorted((root / "plugins").glob("*")):
        users = [f for skill in sorted((plugin / "skills").glob("*")) for f, t in skill_texts(skill) if RESEARCH_POINTER.search(t)]
        if not users:
            continue
        link = plugin / "references" / "research.md"
        want = (root / RESEARCH_SHARED).resolve()
        if not link.is_symlink() or link.resolve() != want:
            problems.append(f"{link.relative_to(root)}: must be a symlink to {RESEARCH_SHARED} (pointed at by {users[0].relative_to(root)})")
    return problems


def check_research_denied(root: Path) -> list[str]:
    problems = []
    for ref, reason in RESEARCH_DENY.items():
        plugin, skill = ref.split(":")
        for f, text in skill_texts(root / "plugins" / plugin / "skills" / skill):
            if RESEARCH_POINTER.search(text):
                problems.append(f"{f.relative_to(root)}: {ref} must not point at the research method ({reason})")
    return problems


def check_research_labels(root: Path) -> list[str]:
    problems = []
    for f in prose_files(root):
        for n, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
            for m in RESEARCH_LABEL.finditer(line):
                word, rest = m.group(1), m.group(2)
                ok = (word == "RESEARCHED" and re.fullmatch(r": S(\d+|<n>)", rest)) or (word == "KNOWN" and rest == "") or (word == "ASSUMPTION" and (rest == "" or rest.startswith(": ")))
                if not ok:
                    problems.append(f"{f.relative_to(root)}:{n}: label {m.group(0)} is not [RESEARCHED: S<n>], [KNOWN] or [ASSUMPTION]")
    return problems


def check_research_method(root: Path) -> list[str]:
    path = root / RESEARCH_SHARED
    if not path.is_file():
        return []
    text = path.read_text(encoding="utf-8")
    return [f"{RESEARCH_SHARED}: lost its {name} section" for name, pat in RESEARCH_SECTIONS if not re.search(pat, text, re.I | re.M)]


def check_rust_workspace(root: Path) -> list[str]:
    """rust/ is not a plugin, and its crates carry the repository's license."""
    ws = root / "rust" / "Cargo.toml"
    if not ws.is_file():
        return []
    problems = []
    for stray in (".claude-plugin", "skills", "plugins"):
        if (root / "rust" / stray).exists():
            problems.append(f"rust/{stray}: rust/ is a Cargo workspace, not a plugin; plugins live in plugins/<name>/")
    licenses = set()
    for manifest in sorted(root.glob("plugins/*/.claude-plugin/plugin.json")):
        lic = json.loads(manifest.read_text(encoding="utf-8")).get("license")
        if lic:
            licenses.add(lic)
    if len(licenses) != 1:
        return problems + [f"plugin licenses are not one value ({sorted(licenses)}); cannot compare the crates to them"]
    (expected,) = licenses
    workspace = tomllib.loads(ws.read_text(encoding="utf-8"))
    ws_license = workspace.get("workspace", {}).get("package", {}).get("license")
    for member in workspace.get("workspace", {}).get("members", []):
        manifest = root / "rust" / member / "Cargo.toml"
        if not manifest.is_file():
            problems.append(f"rust/{member}/Cargo.toml: workspace member is missing")
            continue
        pkg = tomllib.loads(manifest.read_text(encoding="utf-8")).get("package", {})
        lic = pkg.get("license")
        if isinstance(lic, dict) and lic.get("workspace") is True:
            lic = ws_license
        if lic != expected:
            problems.append(f"rust/{member}/Cargo.toml: license is {lic!r}, the plugins declare {expected!r}")
    return problems


def main(root: Path = ROOT) -> int:
    if "--sync-shared-code" in sys.argv[1:] or "--sync-guard-ux" in sys.argv[1:]:
        for shared, plugins in SHARED_CODE.items():
            for plugin in plugins:
                (root / "plugins" / plugin / "hooks" / Path(shared).name).write_bytes((root / shared).read_bytes())
        print(f"copied {len(SHARED_CODE)} shared files into the plugins that import them")
        return 0
    if "--sync-secret-data" in sys.argv[1:]:
        (root / SECRET_DATA).write_text(secret_data_text(root), encoding="utf-8")
        print(f"wrote {SECRET_DATA}")
        return 0
    problems = check_sections(root) + check_retired(root) + check_cleanup(root) + check_prune(root) + check_cards(root) + check_vocab(root) + check_secret_data(root) + check_pipeline_paths(root) + check_pipeline_digest(root) + check_applicability(root) + check_stage_names(root) + check_guard_ux(root) + check_release_subjects(root) + check_release_json(root) + check_leftovers_json(root) + check_research_links(root) + check_research_denied(root) + check_research_labels(root) + check_research_method(root) + check_rust_workspace(root)
    budget, total = check_budget(root)
    problems += budget
    for p in problems:
        print(f"FAIL  {p}")
    if not problems:
        print(f"ok    section references, retired phrasings, container cleanup, card projections, secret data, pipeline paths and digest, guard reasons, release subjects, release --json keys, leftovers JSON, research method, rust workspace license, descriptions ({total} characters)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
# t016 throwaway
