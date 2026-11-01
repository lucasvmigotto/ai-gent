#!/usr/bin/env python3
"""Build the ai-gent release package (stdlib only).

  scripts/package.py --version 2.26.0 [--out dist]   build dist/ai-gent-<ver>.tar.gz,
                                                     dist/SHA256SUMS and dist/LATEST
  scripts/package.py --check                         build into a temp dir and verify it;
                                                     publishes nothing (wired into check.sh)

The package is an ALLOWLIST of the repository, keeping its relative layout so
the relative symlinks (plugins/<n>/references/* -> ../../../shared/*) still
resolve: plugins/ without dev-only files, the shared/ files those symlinks
reach, opencode/guard, setup.sh, install.sh, the update scripts, LICENSE,
README.md, VERSION and MANIFEST (sha256 of every file, `sha256sum -c` format).

It refuses to build when a symlink would dangle, be absolute or leave the
package. The output is deterministic: sorted entries, fixed mtime/uid/gid/modes,
gzip without a timestamp, so two builds of one tree are byte-identical (given
the same zlib). Release flow: shared/releasing.md.
"""

import argparse
import gzip
import hashlib
import io
import os
import posixpath
import re
import shutil
import stat
import sys
import tarfile
import tempfile

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
EPOCH = 0  # fixed mtime for every entry

# Whole files and trees copied as they are (relative to the repository root).
ROOT_FILES = [
    "setup.sh",
    "install.sh",
    "LICENSE",
    "README.md",
    "scripts/update-check.sh",
    "scripts/ai-gent-update.sh",
    "scripts/opencode-config.py",  # setup.sh edits opencode.json with it
]
TREES = ["plugins", "opencode/guard"]

# Dev-only: never shipped. Directory names are matched anywhere in a tree.
DEV_DIRS = {"evals", "results", "tests", "test", "__pycache__", "node_modules", ".git", ".ai-gent", ".worktrees"}
DEV_FILE_RE = re.compile(
    r"(\.test\.(ts|tsx|js|mjs)$|^test_.*\.py$|\.pyc$|^\.DS_Store$|~$|\.bak(-.*)?$)"
)
# Laid by Claude Code for a loaded hooks module, gitignored.
DEV_PATHS = ("/.claude-plugin/types/",)


class PackageError(Exception):
    pass


def is_dev(rel):
    parts = rel.split("/")
    if any(p in DEV_DIRS for p in parts[:-1]):
        return True
    if any(d in "/" + rel for d in DEV_PATHS):
        return True
    return bool(DEV_FILE_RE.search(parts[-1]))


def walk(top):
    """Yield repo-relative paths (files and symlinks) under a tree, sorted."""
    base = os.path.join(ROOT, top)
    for dirpath, dirs, files in os.walk(base):
        rel_dir = os.path.relpath(dirpath, ROOT).replace(os.sep, "/")
        dirs[:] = sorted(d for d in dirs if not is_dev(rel_dir + "/" + d + "/x"))
        # os.walk lists symlinks to directories in dirs; none are allowed
        for d in list(dirs):
            if os.path.islink(os.path.join(dirpath, d)):
                raise PackageError(f"{rel_dir}/{d}: symlinks to directories are not supported")
        for name in sorted(files):
            rel = rel_dir + "/" + name
            if not is_dev(rel):
                yield rel


def resolve_in_package(rel, members):
    """Follow package symlinks from rel; return the final regular-file path.

    Raises PackageError for an absolute, escaping, dangling or looping link.
    """
    seen = set()
    cur = rel
    while True:
        if cur in seen:
            raise PackageError(f"{rel}: symlink loop")
        seen.add(cur)
        kind, target = members.get(cur, (None, None))
        if kind is None:
            raise PackageError(f"{rel}: symlink dangles (no {cur} in the package)")
        if kind == "file":
            return cur
        if os.path.isabs(target) or target.startswith("/"):
            raise PackageError(f"{cur}: absolute symlink target {target}")
        nxt = posixpath.normpath(posixpath.join(posixpath.dirname(cur), target))
        if nxt.startswith("../") or nxt == "..":
            raise PackageError(f"{cur}: symlink points outside the package ({target})")
        cur = nxt


def collect(version):
    """Return {relpath: ("file", abspath) | ("link", target)} plus generated files."""
    members = {}

    def add(rel):
        path = os.path.join(ROOT, rel)
        if os.path.islink(path):
            members[rel] = ("link", os.readlink(path))
        elif os.path.isfile(path):
            members[rel] = ("file", path)
        else:
            raise PackageError(f"{rel}: missing")

    for rel in ROOT_FILES:
        add(rel)
    for top in TREES:
        if not os.path.isdir(os.path.join(ROOT, top)):
            raise PackageError(f"{top}: missing")
        for rel in walk(top):
            add(rel)

    # Pull in what the symlinks reach (the shared/ files), transitively.
    pending = [r for r, (k, _) in members.items() if k == "link"]
    while pending:
        rel = pending.pop()
        target = members[rel][1]
        if os.path.isabs(target):
            raise PackageError(f"{rel}: absolute symlink target {target}")
        nxt = posixpath.normpath(posixpath.join(posixpath.dirname(rel), target))
        if nxt.startswith("../") or nxt == "..":
            raise PackageError(f"{rel}: symlink points outside the package ({target})")
        if nxt in members:
            continue
        if not nxt.startswith("shared/"):
            raise PackageError(f"{rel}: links to {nxt}, which is not a packaged path or under shared/")
        path = os.path.join(ROOT, nxt)
        if not os.path.lexists(path):
            raise PackageError(f"{rel}: symlink dangles ({nxt} does not exist)")
        add(nxt)
        if members[nxt][0] == "link":
            pending.append(nxt)

    # Every link must now end at a regular file in the package.
    for rel in sorted(members):
        if members[rel][0] == "link":
            resolve_in_package(rel, members)
    return members


def sha256_of(data):
    return hashlib.sha256(data).hexdigest()


def build(version, out_dir):
    """Write ai-gent-<version>.tar.gz, SHA256SUMS and LATEST into out_dir."""
    if not VERSION_RE.match(version):
        raise PackageError(f"version must look like 1.2.3, got {version!r}")
    members = collect(version)

    # Contents of regular files (symlinks are hashed through their target).
    contents = {}
    for rel in sorted(members):
        real = resolve_in_package(rel, members)
        with open(members[real][1], "rb") as fh:
            contents[rel] = fh.read()
    manifest = "".join(f"{sha256_of(contents[r])}  {r}\n" for r in sorted(contents))
    version_txt = version + "\n"

    prefix = f"ai-gent-{version}"
    entries = {}  # path -> (TarInfo, data or None)

    def info(path, kind, mode, size=0, link=""):
        ti = tarfile.TarInfo(f"{prefix}/{path}" if path else prefix)
        ti.type = kind
        ti.mode = mode
        ti.size = size
        ti.mtime = EPOCH
        ti.uid = ti.gid = 0
        ti.uname = ti.gname = ""
        ti.linkname = link
        return ti

    dirs = {""}
    for rel in members:
        d = posixpath.dirname(rel)
        while d:
            dirs.add(d)
            d = posixpath.dirname(d)
    for d in dirs:
        entries[d + "/"] = (info(d, tarfile.DIRTYPE, 0o755), None)
    for rel, (kind, val) in members.items():
        if kind == "link":
            entries[rel] = (info(rel, tarfile.SYMTYPE, 0o777, link=val), None)
        else:
            mode = 0o755 if os.stat(val).st_mode & stat.S_IXUSR else 0o644
            entries[rel] = (info(rel, tarfile.REGTYPE, mode, len(contents[rel])), contents[rel])
    for rel, data, mode in (("VERSION", version_txt.encode(), 0o644), ("MANIFEST", manifest.encode(), 0o644)):
        entries[rel] = (info(rel, tarfile.REGTYPE, mode, len(data)), data)
    # MANIFEST lists VERSION too; recompute with VERSION included
    manifest = "".join(
        f"{sha256_of(contents[r] if r in contents else version_txt.encode())}  {r}\n"
        for r in sorted(set(contents) | {"VERSION"})
    )
    data = manifest.encode()
    entries["MANIFEST"] = (info("MANIFEST", tarfile.REGTYPE, 0o644, len(data)), data)

    os.makedirs(out_dir, exist_ok=True)
    name = f"{prefix}.tar.gz"
    final = os.path.join(out_dir, name)
    tmp = final + ".part"
    with open(tmp, "wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, compresslevel=9, mtime=0) as gz:
            with tarfile.open(fileobj=gz, mode="w", format=tarfile.GNU_FORMAT) as tar:
                for path in sorted(entries):
                    ti, payload = entries[path]
                    tar.addfile(ti, io.BytesIO(payload) if payload is not None else None)
    os.replace(tmp, final)
    with open(final, "rb") as fh:
        digest = sha256_of(fh.read())
    with open(os.path.join(out_dir, "SHA256SUMS"), "w") as fh:
        fh.write(f"{digest}  {name}\n")
    with open(os.path.join(out_dir, "LATEST"), "w") as fh:  # for mirrors (AI_GENT_RELEASE_BASE)
        fh.write(version_txt)
    return final, len(entries)


def verify(tarball):
    """Re-read a built package and check its contents; raise PackageError."""
    problems = []
    names = {}
    with tarfile.open(tarball, "r:gz") as tar:
        top = None
        for m in tar.getmembers():
            first = m.name.split("/")[0]
            top = top or first
            if first != top or m.name.startswith("/") or ".." in m.name.split("/"):
                problems.append(f"unsafe or foreign member {m.name}")
            names[m.name[len(top) + 1:]] = m
        manifest = tar.extractfile(names["MANIFEST"]).read().decode()
        version = tar.extractfile(names["VERSION"]).read().decode().strip()
        listed = {}
        for line in manifest.splitlines():
            digest, _, path = line.partition("  ")
            listed[path] = digest
        for path, m in names.items():
            if m.isdir() or path == "MANIFEST":
                continue
            if path not in listed:
                problems.append(f"{path}: not in MANIFEST")
            if m.issym():
                tgt = posixpath.normpath(posixpath.join(posixpath.dirname(path), m.linkname))
                if tgt.startswith("..") or tgt not in names:
                    problems.append(f"{path}: dangling or escaping symlink -> {m.linkname}")
                continue
            if sha256_of(tar.extractfile(m).read()) != listed.get(path):
                problems.append(f"{path}: sha256 differs from MANIFEST")
            if is_dev(path) and path not in ("VERSION", "MANIFEST"):
                problems.append(f"{path}: dev-only file in the package")
            if m.uid or m.gid or m.mtime != EPOCH:
                problems.append(f"{path}: not normalized (uid/gid/mtime)")
        for path in listed:
            if path not in names:
                problems.append(f"{path}: in MANIFEST but not in the package")
        for need in ("setup.sh", "install.sh", "scripts/update-check.sh", "scripts/ai-gent-update.sh",
                     "plugins/git/hooks/hooks.json", "plugins/git/.claude-plugin/plugin.json", "LICENSE"):
            if need not in names:
                problems.append(f"{need}: missing")
        for path in names:
            if re.match(r"(scripts/(release|consistency|run-evals|check)\.|\.github/|docs/|specs/)", path):
                problems.append(f"{path}: repo-maintenance file in the package")
    if problems:
        raise PackageError("package check failed:\n  " + "\n  ".join(problems))
    return version, len(names)


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--version", help="release version, e.g. 2.26.0 (the tag, without a v)")
    ap.add_argument("--out", default=os.path.join(ROOT, "dist"), help="output directory (default dist/)")
    ap.add_argument("--check", action="store_true", help="build twice into a temp dir, verify, publish nothing")
    args = ap.parse_args(argv)
    try:
        if args.check:
            version = args.version or "0.0.0"
            tmp = tempfile.mkdtemp(prefix="ai-gent-package-")
            try:
                a, _ = build(version, os.path.join(tmp, "a"))
                b, count = build(version, os.path.join(tmp, "b"))
                with open(a, "rb") as fa, open(b, "rb") as fb:
                    if fa.read() != fb.read():
                        raise PackageError("two builds of the same tree differ (not deterministic)")
                ver, n = verify(a)
                print(f"package ok: ai-gent-{ver}.tar.gz, {n} entries, {os.path.getsize(a)} bytes, deterministic")
            finally:
                shutil.rmtree(tmp, ignore_errors=True)
            return 0
        if not args.version:
            ap.error("--version is required (or use --check)")
        path, count = build(args.version, args.out)
        verify(path)
        print(f"built {path} ({count} entries); SHA256SUMS and LATEST beside it")
        return 0
    except PackageError as exc:
        print(f"package.py: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
