#!/usr/bin/env python3
"""Plan and prepare a release from Conventional Commits.

Reads the commits since the last plain SemVer tag (X.Y.Z, no "v") and
decides whether they warrant a release:

  feat                          -> minor
  fix, perf, refactor           -> patch
  "type!:" or a BREAKING CHANGE -> major
  anything else (docs, chore, ci, test, style, build, revert, ...) -> none

Merge-commit subjects and "chore(release): ..." commits are ignored (the
commits a merge brings in are counted on their own).

It then bumps, in place:
  - "version" in each manifest listed in release.json
    ({"manifests": [{"file": "docs/site/package.json"}]}) whose directory
    (or extra "paths") changed since the tag, under the same rules;
  - "version" in each plugins/<name>/.claude-plugin/plugin.json whose files
    (or files its symlinks point to, such as shared/pipeline.md) changed
    since the tag, by the bump of the commits touching those paths; a plugin
    new since the tag, or whose version was already changed by hand since
    the tag, is left alone;
  - the rust/ workspace, when rust/crates exists (spec 032, D23): the binary
    crate (ai-gent) takes the release version; every other crate there is a
    library with its own SemVer, bumped by the commits touching its directory
    (below 1.0.0 a breaking commit is a minor bump, anything else releasable a
    patch; a crate whose version was changed by hand since the tag is left
    alone); the binary's version requirement on each library is kept equal to
    that library's version, and a difference stops the release (exit 1) before
    anything is written; rust/Cargo.lock follows. Their targets have kind "rust";
  - CHANGELOG.md: a "## Unreleased" section is renamed to
    "## X.Y.Z — YYYY-MM-DD" (hand-written notes win, nothing is added);
    otherwise a section is generated from the commits.

With no previous tag, the release is --initial (default 0.1.0) if there is
at least one releasable commit.

Usage (run from anywhere inside the repository):
  release.py [--dry-run [--json]] [--date YYYY-MM-DD] [--initial X.Y.Z]
  release.py --notes [X.Y.Z]

Output and exit codes:
  default    writes the files; prints the new version alone on stdout,
             the plan on stderr; exit 0
  --dry-run  prints the plan (version, plugin bumps, changelog section) on
             stdout, writes nothing; exit 0
  --dry-run --json
             prints the plan as one JSON object (version, bump, since,
             targets, changelog) on stdout; exit 3 and errors print nothing
  --notes    prints one release's CHANGELOG section body (default: the
             newest version heading) for the GitHub Release; exit 0
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
JSON_KEYS = ("version", "bump", "since", "targets", "changelog")
TARGET_KEYS = ("kind", "label", "from", "to", "bump")
RANK = {"none": 0, "patch": 1, "minor": 2, "major": 3}
TYPE_BUMP = {"feat": "minor", "fix": "patch", "perf": "patch", "refactor": "patch"}
HEADER_RE = re.compile(r"^(?P<type>[a-z]+)(?:\((?P<scope>[^)]*)\))?(?P<bang>!)?: (?P<desc>.+)$")
BREAKING_RE = re.compile(r"^BREAKING[ -]CHANGE: *(?P<text>.+)$", re.M)
TAG_RE = re.compile(r"^\d+\.\d+\.\d+$")
SEP_FIELD, SEP_RECORD = "\x1f", "\x1e"


class ReleaseError(Exception):
    pass


def git(*args):
    result = subprocess.run(["git", *args], capture_output=True, text=True)
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


def commits(since, paths=()):
    """Releasable-or-not commits in since..HEAD (all history if since is None)."""
    rev = f"{since}..HEAD" if since else "HEAD"
    fmt = SEP_FIELD.join(["%H", "%P", "%s", "%b"]) + SEP_RECORD
    out = git("log", "--no-color", f"--format={fmt}", rev, "--", *paths) if paths else git(
        "log", "--no-color", f"--format={fmt}", rev)
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


def plugin_paths(root, plugin_dir):
    """The plugin's directory plus the repo files its symlinks point to."""
    paths = {os.path.relpath(plugin_dir, root)}
    for dirpath, dirnames, filenames in os.walk(plugin_dir):
        for name in dirnames + filenames:
            full = os.path.join(dirpath, name)
            if os.path.islink(full):
                target = os.path.realpath(full)
                if target.startswith(root + os.sep) and not target.startswith(plugin_dir + os.sep):
                    paths.add(os.path.relpath(target, root))
    return sorted(paths)


VERSION_RE = re.compile(r'("version"\s*:\s*")([^"]+)(")')
MANIFEST_CONFIG = "release.json"


def version_in(text):
    match = re.search(r'"version"\s*:\s*"([^"]+)"', text)
    if not match:
        raise ReleaseError('no "version" field')
    return match.group(1)


def load_manifest_config(root):
    """Manifests listed in release.json, bumped alongside any plugins/; None without it."""
    path = os.path.join(root, MANIFEST_CONFIG)
    if not os.path.isfile(path):
        return None
    try:
        data = json.loads(open(path).read())
    except ValueError as e:
        raise ReleaseError(f"{MANIFEST_CONFIG}: invalid JSON ({e})")
    manifests = data.get("manifests")
    if not isinstance(manifests, list) or not manifests:
        raise ReleaseError(f"{MANIFEST_CONFIG}: 'manifests' must be a non-empty list")
    return manifests


def plan_manifests(root, tag, config):
    """Version bumps for the manifests in release.json.

    Each entry is {file: path, paths: [extra paths]} — the version at `file`
    is compared with its value at the last tag, and bumped by the strongest
    commit touching `file`'s directory (plus any extra `paths`).
    """
    plans = []
    for entry in config:
        rel = entry.get("file")
        if not rel:
            raise ReleaseError(f"{MANIFEST_CONFIG}: a manifest entry has no 'file'")
        full = os.path.join(root, rel)
        if not os.path.isfile(full):
            raise ReleaseError(f"{rel}: manifest not found")
        text = open(full).read()
        current = version_in(text)
        if tag is None:
            continue
        try:
            tagged = version_in(git("show", f"{tag}:{rel}"))
        except ReleaseError:
            continue  # new since the tag: its version is whatever it was created with
        if tagged != current:
            continue  # already bumped by hand since the tag
        paths = entry.get("paths") or [os.path.dirname(rel) or "."]
        bump = max_bump(commits(tag, paths))
        if bump != "none":
            plans.append({"kind": "manifest", "name": rel, "label": f"manifest {rel}", "manifest": full, "text": text,
                          "from": current, "to": bump_version(current, bump), "bump": bump})
    return plans


def plan_plugins(root, tag):
    plans = []
    plugins_dir = os.path.join(root, "plugins")
    if not os.path.isdir(plugins_dir):
        return plans
    for name in sorted(os.listdir(plugins_dir)):
        plugin_dir = os.path.join(plugins_dir, name)
        manifest = os.path.join(plugin_dir, ".claude-plugin", "plugin.json")
        if not os.path.isfile(manifest):
            continue
        rel_manifest = os.path.relpath(manifest, root)
        with open(manifest) as f:
            text = f.read()
        try:
            current = json.loads(text)["version"]
        except (ValueError, KeyError) as e:
            raise ReleaseError(f"{rel_manifest}: can't read version ({e})")
        if tag is None:
            continue
        try:
            tagged = json.loads(git("show", f"{tag}:{rel_manifest}"))["version"]
        except ReleaseError:
            continue  # new since the tag: its version is whatever it was created with
        if tagged != current:
            continue  # already bumped by hand since the tag
        bump = max_bump(commits(tag, plugin_paths(root, plugin_dir)))
        if bump != "none":
            plans.append({"kind": "plugin", "name": name, "label": f"plugin {name}", "manifest": manifest, "text": text,
                          "from": current, "to": bump_version(current, bump), "bump": bump})
    return plans


# ---------------------------------------------------------------- rust crates
#
# The rust/ workspace (spec 032, D23, D28). Versions live in each crate's Cargo.toml
# and in rust/Cargo.lock; neither is edited by hand:
#   - the binary crate (BINARY_CRATE) is in lockstep with the release tag;
#   - every other crate under rust/crates/ is a library with an independent SemVer,
#     bumped from the conventional commits that touch rust/crates/<dir> (below 1.0.0
#     a breaking commit is a minor bump, anything else releasable a patch);
#   - the binary's `version = "x.y.z"` requirement on each library equals that
#     library's version, and a difference stops the release before anything is written.

RUST_DIR = "rust"
RUST_CRATES = "rust/crates"
BINARY_CRATE = "ai-gent"
PACKAGE_NAME_RE = re.compile(r'(?m)^\[package\]\s*\n(?:(?!\[).*\n)*?name\s*=\s*"([^"]+)"')
PACKAGE_VERSION_RE = re.compile(r'(?m)(^\[package\]\s*\n(?:(?!\[).*\n)*?version\s*=\s*")([^"]+)(")')


def lib_bump(version, bump):
    """Bump a library crate: below 1.0.0 breaking is a minor bump, the rest a patch."""
    if parse_version(version)[0] == 0 and bump != "none":
        bump = "minor" if bump == "major" else "patch"
    return bump_version(version, bump), bump


def read_crates(root):
    """Crates under rust/crates/: [{dir, name, file, text, version}], sorted by name."""
    crates = []
    base = os.path.join(root, RUST_CRATES)
    if not os.path.isdir(base):
        return crates
    for d in sorted(os.listdir(base)):
        file = os.path.join(base, d, "Cargo.toml")
        if not os.path.isfile(file):
            continue
        rel = os.path.relpath(file, root)
        text = open(file).read()
        name, version = PACKAGE_NAME_RE.search(text), PACKAGE_VERSION_RE.search(text)
        if not name or not version:
            raise ReleaseError(f"{rel}: no [package] name and version")
        crates.append({"dir": d, "name": name.group(1), "file": file, "rel": rel, "text": text,
                       "version": version.group(2)})
    return sorted(crates, key=lambda c: c["name"])


def requirement_re(name):
    return re.compile(r'(?m)^(%s\s*=\s*\{[^}\n]*\bversion\s*=\s*")([^"]+)(")' % re.escape(name))


def plan_rust(root, tag, version, bump):
    """Version writes for the rust crates and rust/Cargo.lock; nothing without rust/crates."""
    crates = read_crates(root)
    if not crates:
        return []
    by_name = {c["name"]: c for c in crates}
    binary = by_name.get(BINARY_CRATE)
    if binary is None:
        raise ReleaseError(f"{RUST_CRATES}: no crate named {BINARY_CRATE}")
    libs = [c for c in crates if c is not binary]

    # The binary's requirement on each library equals the library's current version, or we stop.
    for lib in libs:
        match = requirement_re(lib["name"]).search(binary["text"])
        if match and match.group(2) != lib["version"]:
            raise ReleaseError(
                f"{binary['rel']}: requires {lib['name']} {match.group(2)} but {lib['rel']} is {lib['version']}; "
                "make them equal (nothing was written)")
    if parse_version(binary["version"]) > parse_version(version):
        raise ReleaseError(f"{binary['rel']}: version {binary['version']} is ahead of the release {version}")

    plans = []
    for lib in libs:
        if tag is None:
            continue
        try:
            tagged = PACKAGE_VERSION_RE.search(git("show", f"{tag}:{lib['rel']}")).group(2)
        except (ReleaseError, AttributeError):
            tagged = None  # new since the tag: bumped like any other (a placeholder 0.1.0 may be published)
        if tagged is not None and tagged != lib["version"]:
            continue  # already bumped by hand since the tag
        change = max_bump(commits(tag, [os.path.relpath(os.path.dirname(lib["file"]), root)]))
        if change == "none":
            continue
        new, applied = lib_bump(lib["version"], change)
        plans.append({"kind": "rust", "name": lib["name"], "label": f"crate {lib['name']}", "crate": lib,
                      "from": lib["version"], "to": new, "bump": applied})
    if binary["version"] != version:
        plans.append({"kind": "rust", "name": binary["name"], "label": f"crate {binary['name']}", "crate": binary,
                      "from": binary["version"], "to": version, "bump": bump})
    lock = os.path.join(root, RUST_DIR, "Cargo.lock")
    if plans:
        if not os.path.isfile(lock):
            raise ReleaseError(f"{RUST_DIR}/Cargo.lock: missing (commit it; a binary workspace locks)")
        text = open(lock).read()
        for p in plans:
            if not re.search(r'\[\[package\]\]\nname = "%s"\nversion = "%s"\n' % (re.escape(p["name"]), re.escape(p["from"])), text):
                raise ReleaseError(f"{RUST_DIR}/Cargo.lock: no entry for {p['name']} {p['from']} "
                                   "(out of date with its Cargo.toml; nothing was written)")
    return plans


def write_rust(root, plans):
    """Write the crate versions, the binary's requirements and the Cargo.lock entries."""
    if not plans:
        return
    new = {p["name"]: p["to"] for p in plans}
    old = {p["name"]: p["from"] for p in plans}
    crates = read_crates(root)
    binary = next(c for c in crates if c["name"] == BINARY_CRATE)
    for c in crates:
        text = c["text"]
        if c["name"] in new:
            text, n = PACKAGE_VERSION_RE.subn(lambda m: m[1] + new[c["name"]] + m[3], text, count=1)
            if n != 1:
                raise ReleaseError(f"{c['rel']}: no version to bump")
        if c is binary:
            for name, to in new.items():
                if name != BINARY_CRATE:
                    text = requirement_re(name).sub(lambda m: m[1] + to + m[3], text)
        if text != c["text"]:
            with open(c["file"], "w") as f:
                f.write(text)
    lock = os.path.join(root, RUST_DIR, "Cargo.lock")
    if not os.path.isfile(lock):
        raise ReleaseError(f"{RUST_DIR}/Cargo.lock: missing (commit it; a binary workspace locks)")
    text = open(lock).read()
    for name, to in new.items():
        pattern = re.compile(r'(\[\[package\]\]\nname = "%s"\nversion = ")%s(")' % (re.escape(name), re.escape(old[name])))
        text, n = pattern.subn(lambda m: m[1] + to + m[2], text)
        if n != 1:
            raise ReleaseError(f"{RUST_DIR}/Cargo.lock: no entry for {name} {old[name]} (out of date with its Cargo.toml)")
    with open(lock, "w") as f:
        f.write(text)


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
    parser = argparse.ArgumentParser(description="Plan and prepare a release from Conventional Commits.")
    parser.add_argument("--dry-run", action="store_true", help="print the plan, write nothing")
    parser.add_argument("--json", action="store_true", help="with --dry-run, print the plan as one JSON object")
    parser.add_argument("--notes", nargs="?", const="", metavar="X.Y.Z",
                        help="print a release's CHANGELOG section (default: the newest)")
    parser.add_argument("--date", help="release date, YYYY-MM-DD (default: today, UTC)")
    parser.add_argument("--initial", default="0.1.0", help="version when no tag exists (default 0.1.0)")
    args = parser.parse_args(argv)
    if not TAG_RE.match(args.initial):
        parser.error("--initial must be X.Y.Z")
    if args.date and not re.fullmatch(r"\d{4}-\d{2}-\d{2}", args.date):
        parser.error("--date must be YYYY-MM-DD")
    if args.json and not args.dry_run:
        parser.error("--json needs --dry-run")

    root = repo_root()
    changelog = os.path.join(root, "CHANGELOG.md")

    if args.notes is not None:
        with open(changelog) as f:
            sys.stdout.write(section_of(f.read(), args.notes or None))
        return 0

    tag = last_tag()
    items = commits(tag)
    bump = max_bump(items)
    if bump == "none":
        print(f"no release needed: no feat, fix, perf, refactor or breaking commits since {tag or 'the start'}",
              file=sys.stderr)
        return NO_RELEASE
    version = bump_version(tag, bump) if tag else args.initial
    date = args.date or datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    config = load_manifest_config(root)
    plans = plan_plugins(root, tag) + (plan_manifests(root, tag, config) if config is not None else [])
    plans += plan_rust(root, tag, version, bump)

    text = open(changelog).read() if os.path.exists(changelog) else ""
    new_changelog, how = updated_changelog(text, version, date, items)

    plan = [f"release {version} ({bump}, since {tag or 'the start'})",
            *(f"{p['label']}: {p['from']} -> {p['to']} ({p['bump']})" for p in plans),
            f"CHANGELOG.md: {how}"]
    if args.dry_run and args.json:
        targets = [dict(zip(TARGET_KEYS, (p["kind"], p["name"], p["from"], p["to"], p["bump"]))) for p in plans]
        print(json.dumps(dict(zip(JSON_KEYS, (version, bump, tag, targets, how))), ensure_ascii=False))
        return 0
    if args.dry_run:
        print("\n".join(plan))
        print()
        print(f"## {version} — {date}")
        print()
        print(section_of(new_changelog, version), end="")
        return 0

    for p in plans:
        if p["kind"] == "rust":
            continue
        new_text, count = VERSION_RE.subn(lambda m: m[1] + p["to"] + m[3], p["text"], count=1)
        if count != 1:
            raise ReleaseError(f"{p['manifest']}: no version field to bump")
        with open(p["manifest"], "w") as f:
            f.write(new_text)
    write_rust(root, [p for p in plans if p["kind"] == "rust"])
    with open(changelog, "w") as f:
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
