#!/usr/bin/env python3
"""Build the ai-gent release package (stdlib only).

  scripts/package.py --version 2.26.0 [--out dist]   build dist/ai-gent-<ver>.tar.gz,
                                                     dist/install.sh, dist/install.ps1, dist/SHA256SUMS and dist/LATEST
  scripts/package.py --version 2.27.0 --target <triple> --binary <path> [--out dist]
                                                     build dist/ai-gent-<ver>-<triple>.tar.gz (.zip for
                                                     *-windows-msvc): the package plus bin/ai-gent[.exe];
                                                     the line is added to dist/SHA256SUMS
  scripts/package.py --version 2.27.0 --content-only  the default, spelled out: no binary, as in 2.26
  scripts/package.py --check                         build into a temp dir and verify it (also a stub
                                                     per-triple package); publishes nothing (check.sh)
  scripts/package.py --check --with-binaries <dir>   also package and verify the <dir>/bin-<triple>/
                                                     binaries a rust-build run uploaded (rust.yml)

The package is an ALLOWLIST of the repository, keeping its relative layout so
the relative symlinks (plugins/<n>/references/* -> ../../../shared/*) still
resolve: plugins/ without dev-only files, the shared/ files those symlinks
reach, opencode/guard, setup.sh, install.sh, the update scripts, LICENSE,
README.md, VERSION and MANIFEST (plus setup.ps1 and install.ps1 in the Windows zips only) (sha256 of every file, `sha256sum -c` format).

It refuses to build when a symlink would dangle, be absolute or leave the
package, and when a Rust source, Cargo file, target/ directory or fixture
corpus would be packaged (the binary is the only Rust artifact shipped).
Windows zips hold copies, not symlinks (spec 032 FR-021). The output is deterministic: sorted entries, fixed mtime/uid/gid/modes,
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
import zipfile

ROOT = os.path.realpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
EPOCH = 0  # fixed mtime for every entry
ZIP_EPOCH = (1980, 1, 1, 0, 0, 0)  # the earliest time a zip can store
TARGETS = (
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
)

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
# Only in the *-windows-msvc archives: install.ps1 runs the package's setup.ps1, and `ai-gent update` and
# the rollback run the installed release's install.ps1 (spec 032 D24).
# Keeping it out of the others leaves the content-only and Unix archives byte-identical.
WINDOWS_ONLY_FILES = ["setup.ps1", "install.ps1"]

# Dev-only: never shipped. Directory names are matched anywhere in a tree.
DEV_DIRS = {"evals", "results", "tests", "test", "__pycache__", "node_modules", ".git", ".ai-gent", ".worktrees"}
DEV_FILE_RE = re.compile(
    r"(\.test\.(ts|tsx|js|mjs)$|^test_.*\.py$|\.pyc$|^\.DS_Store$|~$|\.bak(-.*)?$)"
)
# Laid by Claude Code for a loaded hooks module, gitignored.
DEV_PATHS = ("/.claude-plugin/types/",)


class PackageError(Exception):
    pass


# Never in a package, whatever the allowlist reaches (spec 032 contracts/release-assets.md).
RUST_FILE_RE = re.compile(r"(\.rs$|^Cargo\.(toml|lock)$|^deny\.toml$|^rust-toolchain(\.toml)?$)")


def is_rust_artifact(rel):
    parts = rel.split("/")
    if parts[0] in ("rust", "target") or "target" in parts[:-1]:
        return True
    if rel.startswith("shared/guard-fixtures/"):
        return True
    return bool(RUST_FILE_RE.search(parts[-1]))


def is_windows(target):
    return bool(target) and target.endswith("-windows-msvc")


def archive_name(version, target):
    base = f"ai-gent-{version}" + (f"-{target}" if target else "")
    return base + (".zip" if is_windows(target) else ".tar.gz")


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


def write_sums(out_dir, name, digest, installer, installer_ps1, version):
    """Add `name` to out_dir/SHA256SUMS: archives sorted, then install.sh and install.ps1.

    One directory collects the content-only archive and every per-triple one
    (a release runs this script once for each), so existing lines are kept.
    """
    path = os.path.join(out_dir, "SHA256SUMS")
    sums = {}
    if os.path.exists(path):
        with open(path) as fh:
            for line in fh:
                d, _, n = line.rstrip("\n").partition("  ")
                if n and n not in ("install.sh", "install.ps1") and re.match(rf"ai-gent-{re.escape(version)}(-|\.tar\.gz$)", n):
                    sums[n] = d  # lines of another version are stale
    sums[name] = digest
    with open(path, "w") as fh:
        for n in sorted(sums):
            fh.write(f"{sums[n]}  {n}\n")
        fh.write(f"{installer}  install.sh\n")
        fh.write(f"{installer_ps1}  install.ps1\n")


def build(version, out_dir, target=None, binary=None):
    """Write the package, SHA256SUMS, install.sh and LATEST into out_dir.

    Without target/binary this is the content-only package, ai-gent-<version>.tar.gz
    (what 2.26 shipped). With both, ai-gent-<version>-<target>.tar.gz (.zip for
    Windows) holds the same content plus bin/ai-gent[.exe].
    """
    if not VERSION_RE.match(version):
        raise PackageError(f"version must look like 1.2.3, got {version!r}")
    if (target is None) != (binary is None):
        raise PackageError("--target and --binary go together (or neither, for the content-only package)")
    if target is not None and target not in TARGETS:
        raise PackageError(f"unknown target {target!r}; supported: {', '.join(TARGETS)}")
    members = collect(version)
    if is_windows(target):
        for rel in WINDOWS_ONLY_FILES:
            if not os.path.isfile(os.path.join(ROOT, rel)):
                raise PackageError(f"{rel}: missing")
            members[rel] = ("file", os.path.join(ROOT, rel))
    for rel in members:
        if is_rust_artifact(rel):
            raise PackageError(f"{rel}: Rust sources, Cargo files and target/ are never packaged")
    if binary is not None:
        if not os.path.isfile(binary) or os.path.getsize(binary) == 0:
            raise PackageError(f"{binary}: the binary is missing or empty")
        bin_rel = "bin/ai-gent" + (".exe" if is_windows(target) else "")
        if bin_rel in members:
            raise PackageError(f"{bin_rel}: already in the package")
        members[bin_rel] = ("file", os.path.realpath(binary))
    windows = is_windows(target)

    # Contents of regular files (symlinks are hashed through their target).
    contents = {}
    for rel in sorted(members):
        real = resolve_in_package(rel, members)
        with open(members[real][1], "rb") as fh:
            contents[rel] = fh.read()
    manifest = "".join(f"{sha256_of(contents[r])}  {r}\n" for r in sorted(contents))
    version_txt = version + "\n"

    prefix = f"ai-gent-{version}" + (f"-{target}" if target else "")
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

    def file_mode(rel, path):
        # upload-artifact drops the executable bit, so the binary's mode is set here
        return 0o755 if rel.startswith("bin/") or os.stat(path).st_mode & stat.S_IXUSR else 0o644

    dirs = {""}
    for rel in members:
        d = posixpath.dirname(rel)
        while d:
            dirs.add(d)
            d = posixpath.dirname(d)
    for d in dirs:
        entries[d + "/"] = (info(d, tarfile.DIRTYPE, 0o755), None)
    for rel, (kind, val) in members.items():
        if kind == "link" and windows:  # a Windows archive holds copies (FR-021)
            real = resolve_in_package(rel, members)
            entries[rel] = (info(rel, tarfile.REGTYPE, file_mode(real, members[real][1]), len(contents[rel])), contents[rel])
        elif kind == "link":
            entries[rel] = (info(rel, tarfile.SYMTYPE, 0o777, link=val), None)
        else:
            entries[rel] = (info(rel, tarfile.REGTYPE, file_mode(rel, val), len(contents[rel])), contents[rel])
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
    name = archive_name(version, target)
    final = os.path.join(out_dir, name)
    tmp = final + ".part"
    if windows:
        with zipfile.ZipFile(tmp, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
            for path in sorted(entries):
                ti, payload = entries[path]
                if ti.isdir():
                    continue
                zi = zipfile.ZipInfo(ti.name, ZIP_EPOCH)
                zi.create_system = 3  # Unix: the same bytes whichever OS builds it
                zi.external_attr = (stat.S_IFREG | ti.mode) << 16
                zi.compress_type = zipfile.ZIP_DEFLATED
                zf.writestr(zi, payload, compresslevel=9)
    else:
        with open(tmp, "wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, compresslevel=9, mtime=0) as gz:
                with tarfile.open(fileobj=gz, mode="w", format=tarfile.GNU_FORMAT) as tar:
                    for path in sorted(entries):
                        ti, payload = entries[path]
                        tar.addfile(ti, io.BytesIO(payload) if payload is not None else None)
    os.replace(tmp, final)
    with open(final, "rb") as fh:
        digest = sha256_of(fh.read())
    # install.sh and install.ps1 are released beside the archives (curl | sh, irm | iex); their checksums are listed too.
    sums = []
    for script in ("install.sh", "install.ps1"):
        if not os.path.isfile(os.path.join(ROOT, script)):
            raise PackageError(f"{script}: missing")
        shutil.copyfile(os.path.join(ROOT, script), os.path.join(out_dir, script))
        with open(os.path.join(ROOT, script), "rb") as fh:
            sums.append(sha256_of(fh.read()))
    write_sums(out_dir, name, digest, sums[0], sums[1], version)
    with open(os.path.join(out_dir, "LATEST"), "w") as fh:  # for mirrors (AI_GENT_RELEASE_BASE)
        fh.write(version_txt)
    return final, len(entries)


class _Reader:
    """The members of a built archive, tar.gz or zip, behind one interface."""

    def __init__(self, path):
        self.zip = path.endswith(".zip")
        self.items = {}  # name -> dict(kind, mode, link, uid, gid, mtime_ok, read)
        if self.zip:
            self.zf = zipfile.ZipFile(path)
            for zi in self.zf.infolist():
                m = (zi.external_attr >> 16) & 0o177777
                kind = "dir" if zi.is_dir() else "link" if stat.S_ISLNK(m) else "file"
                self.items[zi.filename] = dict(kind=kind, mode=m & 0o777, link="", norm=zi.date_time == ZIP_EPOCH,
                                               read=lambda zi=zi: self.zf.read(zi))
        else:
            self.tar = tarfile.open(path, "r:gz")
            for t in self.tar.getmembers():
                kind = "dir" if t.isdir() else "link" if t.issym() else "file"
                self.items[t.name] = dict(kind=kind, mode=t.mode, link=t.linkname,
                                          norm=not (t.uid or t.gid or t.mtime != EPOCH),
                                          read=(lambda t=t: self.tar.extractfile(t).read()) if kind == "file" else None)

    def close(self):
        (self.zf if self.zip else self.tar).close()


def verify(archive, target=None):
    """Re-read a built package and check its contents; raise PackageError."""
    problems = []
    rd = _Reader(archive)
    try:
        top = None
        names = {}
        for full, it in rd.items.items():
            first = full.split("/")[0]
            top = top or first
            if first != top or full.startswith("/") or ".." in full.split("/"):
                problems.append(f"unsafe or foreign member {full}")
            names[full[len(top) + 1:]] = it
        want_top = os.path.basename(archive)[: -len(".zip" if rd.zip else ".tar.gz")]
        if top != want_top:
            problems.append(f"top directory {top!r} should be {want_top!r}")
        manifest = names["MANIFEST"]["read"]().decode()
        version = names["VERSION"]["read"]().decode().strip()
        listed = {}
        for line in manifest.splitlines():
            digest, _, path = line.partition("  ")
            listed[path] = digest
        for path, it in names.items():
            if it["kind"] == "dir" or path == "MANIFEST":
                continue
            if path not in listed:
                problems.append(f"{path}: not in MANIFEST")
            if it["kind"] == "link":
                if rd.zip:
                    problems.append(f"{path}: a symlink in a zip (Windows archives hold copies)")
                    continue
                tgt = posixpath.normpath(posixpath.join(posixpath.dirname(path), it["link"]))
                if tgt.startswith("..") or tgt not in names:
                    problems.append(f"{path}: dangling or escaping symlink -> {it['link']}")
                continue
            if sha256_of(it["read"]()) != listed.get(path):
                problems.append(f"{path}: sha256 differs from MANIFEST")
            if is_dev(path) and path not in ("VERSION", "MANIFEST"):
                problems.append(f"{path}: dev-only file in the package")
            if is_rust_artifact(path):
                problems.append(f"{path}: Rust source, Cargo file or target/ in the package")
            if not it["norm"]:
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
        for path in WINDOWS_ONLY_FILES:
            if is_windows(target) and path not in names:
                problems.append(f"{path}: missing from the Windows archive")
            elif not is_windows(target) and path in names:
                problems.append(f"{path}: only the Windows archives carry it")
        bins = sorted(p for p in names if p.startswith("bin/"))
        if target:
            want = "bin/ai-gent" + (".exe" if is_windows(target) else "")
            if bins != [want]:
                problems.append(f"bin/ should hold exactly {want}, found {bins}")
            elif names[want]["mode"] & 0o777 != 0o755:
                problems.append(f"{want}: mode {names[want]['mode'] & 0o777:o}, should be 755")
            elif want not in listed:
                problems.append(f"{want}: not in MANIFEST")
        elif bins:
            problems.append(f"the content-only package has {bins}")
    finally:
        rd.close()
    if problems:
        raise PackageError("package check failed:\n  " + "\n  ".join(problems))
    return version, len(names)


def stub_binary(directory, target):
    """A stand-in binary (a shell script) for --check: the archive code does not care what it holds."""
    path = os.path.join(directory, "ai-gent" + (".exe" if is_windows(target) else ""))
    with open(path, "w") as fh:
        fh.write("#!/bin/sh\necho stub\n")
    return path


def check_one(version, tmp, label, target=None, binary=None):
    """Build twice, require identical bytes, verify; return (archive path, entries)."""
    a, _ = build(version, os.path.join(tmp, label + "-a"), target, binary)
    b, count = build(version, os.path.join(tmp, label + "-b"), target, binary)
    with open(a, "rb") as fa, open(b, "rb") as fb:
        if fa.read() != fb.read():
            raise PackageError(f"two builds of {os.path.basename(a)} differ (not deterministic)")
    verify(a, target)
    return a, count


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--version", help="release version, e.g. 2.26.0 (the tag, without a v)")
    ap.add_argument("--out", default=os.path.join(ROOT, "dist"), help="output directory (default dist/)")
    ap.add_argument("--target", help="target triple of --binary: builds the per-triple package")
    ap.add_argument("--binary", help="the ai-gent[.exe] built for --target")
    ap.add_argument("--content-only", action="store_true",
                    help="the package without a binary (the default; refuses --target and --binary)")
    ap.add_argument("--check", action="store_true", help="build twice into a temp dir, verify, publish nothing")
    ap.add_argument("--with-binaries", metavar="DIR",
                    help="with --check: also package the DIR/bin-<triple>/ai-gent[.exe] binaries a rust-build run uploaded")
    args = ap.parse_args(argv)
    if args.content_only and (args.target or args.binary):
        ap.error("--content-only excludes --target and --binary")
    if bool(args.target) != bool(args.binary):
        ap.error("--target and --binary go together")
    if args.with_binaries and not args.check:
        ap.error("--with-binaries needs --check")
    try:
        if args.check:
            version = args.version or "0.0.0"
            tmp = tempfile.mkdtemp(prefix="ai-gent-package-")
            try:
                a, _ = check_one(version, tmp, "content")
                ver, n = verify(a)
                print(f"package ok: ai-gent-{ver}.tar.gz, {n} entries, {os.path.getsize(a)} bytes, deterministic")
                stubs = os.path.join(tmp, "stubs")
                os.makedirs(stubs)
                done = []
                for target in TARGETS:
                    stub_dir = os.path.join(stubs, target)
                    os.makedirs(stub_dir)
                    path, _ = check_one(version, tmp, target, target, stub_binary(stub_dir, target))
                    done.append(os.path.basename(path))
                print(f"per-triple packages ok (stub binaries): {len(done)} archives, deterministic")
                if args.with_binaries:
                    found = []
                    for target in TARGETS:
                        exe = "ai-gent" + (".exe" if is_windows(target) else "")
                        bpath = os.path.join(args.with_binaries, f"bin-{target}", exe)
                        if not os.path.isfile(bpath):
                            print(f"no binary for {target} in {args.with_binaries} (skipped)")
                            continue
                        path, _ = check_one(version, tmp, "real-" + target, target, bpath)
                        found.append(target)
                        print(f"{os.path.basename(path)}: {os.path.getsize(path)} bytes")
                    if not found:
                        raise PackageError(f"--with-binaries: no bin-<triple>/ai-gent in {args.with_binaries}")
            finally:
                shutil.rmtree(tmp, ignore_errors=True)
            return 0
        if not args.version:
            ap.error("--version is required (or use --check)")
        path, count = build(args.version, args.out, args.target, args.binary)
        verify(path, args.target)
        print(f"built {path} ({count} entries); SHA256SUMS and LATEST beside it")
        return 0
    except PackageError as exc:
        print(f"package.py: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
