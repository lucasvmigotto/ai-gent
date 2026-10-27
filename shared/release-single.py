#!/usr/bin/env python3
"""Plan and prepare a release from Conventional Commits (single-version template).

Reads the commits since the last plain SemVer tag (X.Y.Z, no "v") and
decides whether they warrant a release:

  feat                          -> minor
  fix, perf, refactor           -> patch
  "type!:" or a BREAKING CHANGE -> major
  anything else (docs, chore, ci, test, style, build, revert, ...) -> none

Merge-commit subjects and "chore(release): ..." commits are ignored (the
commits a merge brings in are counted on their own).

It then rewrites, in place:
  - the single version source (see below);
  - CHANGELOG.md: a "## Unreleased" section is renamed to
    "## X.Y.Z — YYYY-MM-DD" (hand-written notes win, nothing is added);
    otherwise a section is generated from the commits.

Single-version template: one product, one version, so the tag, the version
source and the CHANGELOG section are the whole release. The version source
is release.json's "version_file" when present, else the first of
pyproject.toml, package.json, Cargo.toml, VERSION that exists:

  {"version_file": "src/app/__init__.py"}

A source holds `version = "X.Y.Z"`, `__version__ = "X.Y.Z"`, `"version":
"X.Y.Z"`, or (a file named VERSION) the bare number. Lock files that record
the version (uv.lock, package-lock.json, Cargo.lock) are the pipeline's job:
re-lock after this script and commit them with it. A repository that
versions several packages uses ai-gent's scripts/release.py instead.

With no previous tag, the release is --initial (default 0.1.0) if there is
at least one releasable commit.

Usage (run from anywhere inside the repository):
  release.py [--dry-run] [--date YYYY-MM-DD] [--initial X.Y.Z]
  release.py --notes [X.Y.Z]

Output and exit codes:
  default    writes the files; prints the new version alone on stdout,
             the plan on stderr; exit 0
  --dry-run  prints the plan (version, changelog section) on
             stdout, writes nothing; exit 0
  --notes    prints one release's CHANGELOG section body (default: the
             newest version heading) for the release notes; exit 0
  exit 3     no release needed (nothing releasable since the last tag)
  exit 1     error (not a git repository, malformed files, ...)
  exit 2     bad arguments
"""

import argparse
import datetime
import json
import os
import re
import subprocess
import sys

NO_RELEASE = 3
RANK = {"none": 0, "patch": 1, "minor": 2, "major": 3}
TYPE_BUMP = {"feat": "minor", "fix": "patch", "perf": "patch", "refactor": "patch"}
HEADER_RE = re.compile(r"^(?P<type>[a-z]+)(?:\((?P<scope>[^)]*)\))?(?P<bang>!)?: (?P<desc>.+)$")
BREAKING_RE = re.compile(r"^BREAKING[ -]CHANGE: *(?P<text>.+)$", re.M)
TAG_RE = re.compile(r"^\d+\.\d+\.\d+$")
VERSION_RE = re.compile(r'^(\s*"?(?:__version__|version)"?\s*[=:]\s*["\'])(\d+\.\d+\.\d+)(["\'])', re.M)
BARE_RE = re.compile(r"^(\s*)(\d+\.\d+\.\d+)(\s*)$")
CANDIDATES = ("pyproject.toml", "package.json", "Cargo.toml", "VERSION")
SEP_FIELD, SEP_RECORD = "\x1f", "\x1e"


class ReleaseError(Exception):
    pass


def git(*args):
    result = subprocess.run(["git", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        raise ReleaseError(f"git {' '.join(args)}: {result.stderr.strip()}")
    return result.stdout


def parse_version(text):
    return tuple(int(part) for part in text.split("."))


def bump_version(version, bump):
    major, minor, patch = parse_version(version)
    if bump == "major":
        return f"{major + 1}.0.0"
    if bump == "minor":
        return f"{major}.{minor + 1}.0"
    if bump == "patch":
        return f"{major}.{minor}.{patch + 1}"
    return version


def last_tag():
    tags = [t for t in git("tag", "--list", "--merged", "HEAD").split() if TAG_RE.match(t)]
    return max(tags, key=parse_version) if tags else None


def commits(since):
    """Releasable-or-not commits in since..HEAD (all history if since is None)."""
    rev = f"{since}..HEAD" if since else "HEAD"
    fmt = SEP_FIELD.join(["%H", "%P", "%s", "%b"]) + SEP_RECORD
    out = git("log", "--no-color", f"--format={fmt}", rev)
    result = []
    for record in out.split(SEP_RECORD):
        record = record.strip("\n")
        if not record:
            continue
        sha, parents, subject, body = (record.split(SEP_FIELD) + [""] * 4)[:4]
        if len(parents.split()) > 1:  # merge commit: its subject doesn't count
            continue
        if subject.startswith("chore(release):"):
            continue
        result.append(classify(sha, subject, body))
    return result


def classify(sha, subject, body):
    match = HEADER_RE.match(subject)
    commit = {"sha": sha, "subject": subject, "type": None, "scope": None, "desc": subject,
              "breaking": None, "bump": "none"}
    if not match:
        return commit
    commit.update(type=match["type"], scope=match["scope"], desc=match["desc"])
    footer = BREAKING_RE.search(body)
    if match["bang"] or footer:
        commit["breaking"] = footer["text"].strip() if footer else match["desc"]
        commit["bump"] = "major"
    else:
        commit["bump"] = TYPE_BUMP.get(match["type"], "none")
    return commit


def max_bump(items):
    return max((c["bump"] for c in items), key=RANK.get, default="none")


def repo_root():
    return os.path.realpath(git("rev-parse", "--show-toplevel").strip())


def version_file(root):
    config = os.path.join(root, "release.json")
    if os.path.exists(config):
        with open(config, encoding="utf-8") as f:
            name = json.load(f).get("version_file")
        if name:
            return name
    for name in CANDIDATES:
        if os.path.exists(os.path.join(root, name)):
            return name
    raise ReleaseError("no version source: add one of " + ", ".join(CANDIDATES)
                       + ' or {"version_file": "path"} in release.json')


def _pattern(name):
    return BARE_RE if os.path.basename(name) == "VERSION" else VERSION_RE


def read_version(name, text):
    match = _pattern(name).search(text)
    if not match:
        raise ReleaseError(f'{name}: no version field "X.Y.Z"')
    return match.group(2)


def write_version(name, text, version):
    new_text, count = _pattern(name).subn(lambda m: m[1] + version + m[3], text, count=1)
    if count != 1:
        raise ReleaseError(f"{name}: no version field to bump")
    return new_text


def generated_section(version, date, items):
    groups = [("Breaking", [c for c in items if c["breaking"]]),
              ("Added", [c for c in items if not c["breaking"] and c["type"] == "feat"]),
              ("Fixed", [c for c in items if not c["breaking"] and c["type"] == "fix"]),
              ("Changed", [c for c in items if not c["breaking"] and c["type"] in ("perf", "refactor")])]
    lines = [f"## {version} — {date}", ""]
    for title, group in groups:
        if not group:
            continue
        lines += [f"### {title}", ""]
        for c in group:
            text = c["breaking"] if title == "Breaking" else c["desc"]
            lines.append(f"- **{c['scope']}:** {text}" if c["scope"] else f"- {text}")
        lines.append("")
    return "\n".join(lines)


def updated_changelog(text, version, date, items):
    heading = f"## {version} — {date}"
    lines = text.split("\n")
    for i, line in enumerate(lines):
        if line.strip() == "## Unreleased":
            lines[i] = heading
            return "\n".join(lines), "renamed ## Unreleased"
    section = generated_section(version, date, items)
    for i, line in enumerate(lines):
        if line.startswith("## "):
            return "\n".join(lines[:i] + section.split("\n") + lines[i:]), "generated from commits"
    body = text.rstrip("\n")
    return (body + "\n\n" if body else "# Changelog\n\n") + section, "generated from commits"


def section_of(text, version=None):
    lines = text.split("\n")
    start = None
    for i, line in enumerate(lines):
        match = re.match(r"^## (\d+\.\d+\.\d+)\b", line)
        if match and (version is None or match[1] == version):
            start = i
            break
    if start is None:
        raise ReleaseError(f"CHANGELOG.md has no section for {version or 'any version'}")
    end = next((j for j in range(start + 1, len(lines)) if lines[j].startswith("## ")), len(lines))
    return "\n".join(lines[start + 1:end]).strip("\n") + "\n"


def main(argv):
    for stream in (sys.stdout, sys.stderr):  # the plan has an em dash; Windows pipes default to cp1252
        try:
            stream.reconfigure(encoding="utf-8")
        except (AttributeError, ValueError):
            pass
    parser = argparse.ArgumentParser(description="Plan and prepare a release from Conventional Commits (single-version template).")
    parser.add_argument("--dry-run", action="store_true", help="print the plan, write nothing")
    parser.add_argument("--notes", nargs="?", const="", metavar="X.Y.Z",
                        help="print a release's CHANGELOG section (default: the newest)")
    parser.add_argument("--date", help="release date, YYYY-MM-DD (default: today, UTC)")
    parser.add_argument("--initial", default="0.1.0", help="version when no tag exists (default 0.1.0)")
    args = parser.parse_args(argv)
    if not TAG_RE.match(args.initial):
        parser.error("--initial must be X.Y.Z")
    if args.date and not re.fullmatch(r"\d{4}-\d{2}-\d{2}", args.date):
        parser.error("--date must be YYYY-MM-DD")

    root = repo_root()
    changelog = os.path.join(root, "CHANGELOG.md")
    source = version_file(root)

    if args.notes is not None:
        with open(changelog, encoding="utf-8") as f:
            sys.stdout.write(section_of(f.read(), args.notes or None))
        return 0

    tag = last_tag()
    items = commits(tag)
    bump = max_bump(items)
    if bump == "none":
        print(f"no release needed: no feat, fix, perf, refactor or breaking commits since {tag or 'the start'}",
              file=sys.stderr)
        return NO_RELEASE

    if tag is None:
        version = args.initial
    else:
        with open(os.path.join(root, source), encoding="utf-8") as f:
            current = read_version(source, f.read())
        if current != tag:
            raise ReleaseError(f"{source} is at {current} but the last tag is {tag}: "
                               "bump it by hand or move the tag before releasing")
        version = bump_version(tag, bump)
    date = args.date or datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")

    text = open(changelog, encoding="utf-8").read() if os.path.exists(changelog) else ""
    new_changelog, how = updated_changelog(text, version, date, items)

    plan = [f"release {version} ({bump}, since {tag or 'the start'})",
            f"{source}: {tag or args.initial} -> {version}",
            f"CHANGELOG.md: {how}"]
    if args.dry_run:
        print("\n".join(plan))
        print()
        print(f"## {version} — {date}")
        print()
        print(section_of(new_changelog, version), end="")
        return 0

    path = os.path.join(root, source)
    with open(path, encoding="utf-8") as f:
        new_source = write_version(source, f.read(), version)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(new_source)
    with open(changelog, "w", encoding="utf-8", newline="\n") as f:
        f.write(new_changelog)
    print("\n".join(plan), file=sys.stderr)
    print(version)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except ReleaseError as e:
        print(f"release.py: error: {e}", file=sys.stderr)
        sys.exit(1)
