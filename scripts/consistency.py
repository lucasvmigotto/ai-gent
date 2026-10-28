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

Run from check.sh; prints one FAIL line per problem and exits 1, else 0.
Environment: DESC_TOTAL_CEILING, the total characters of all descriptions
(default 19000, room for growth; raise it deliberately with a new plugin,
never to silence a bloated single description).
"""
import json
import os
import re
import sys
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


def main(root: Path = ROOT) -> int:
    if "--sync-secret-data" in sys.argv[1:]:
        (root / SECRET_DATA).write_text(secret_data_text(root), encoding="utf-8")
        print(f"wrote {SECRET_DATA}")
        return 0
    problems = check_sections(root) + check_retired(root) + check_cleanup(root) + check_prune(root) + check_cards(root) + check_vocab(root) + check_secret_data(root) + check_pipeline_paths(root)
    budget, total = check_budget(root)
    problems += budget
    for p in problems:
        print(f"FAIL  {p}")
    if not problems:
        print(f"ok    section references, retired phrasings, container cleanup, card projections, secret data, pipeline paths, descriptions ({total} characters)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
