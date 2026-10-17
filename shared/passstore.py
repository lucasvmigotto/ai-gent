"""passstore — how ai-gent's runners read and write secrets in pass stores
(shared/secrets.md). Linked into each runner's scripts/ directory; imported
only on the host, never inside a runner's client container.

- **Stores and mounts.** The personal store is PASSWORD_STORE_DIR (default
  ~/.password-store). AI_GENT_PASS_MOUNTS adds more, as `name=path` pairs
  separated by `;` (`team=/srv/team-store;ops=~/ops-store`): an entry whose
  first segment is a mount's name lives in that store (`team/db/shop` is
  `db/shop` in the team store).
- **Recipients** are pass's own: the nearest `.gpg-id` up from the entry's
  folder, one or more fingerprints per file.
- **Sync.** A store that is a git repository with a remote is fetched and
  fast-forwarded before a read when the last fetch is older than
  AI_GENT_PASS_FRESHNESS seconds (default 300; 0 turns it off), and before a
  write; a write is committed and pushed. Nothing is ever merged: a store
  that can't fast-forward is reported, and the local copy is used.
- **Without pass** (native Windows with Gpg4win), entries are decrypted with
  `gpg --decrypt <store>/<entry>.gpg` — the same files, no pass binary.

Values are never printed: callers get them back and redact them.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

ENTRY_RE = re.compile(r"[A-Za-z0-9_@+][A-Za-z0-9._@+-]*(/[A-Za-z0-9_@+][A-Za-z0-9._@+-]*)*")
MOUNT_RE = re.compile(r"[a-z0-9][a-z0-9-]*")


class StoreError(Exception):
    """A read or write that couldn't be done; the message never holds a value."""


def prefix() -> str:
    return (os.environ.get("AI_GENT_PASS_PREFIX") or "ai-gent").strip("/")


def personal_store() -> Path:
    return Path(os.environ.get("PASSWORD_STORE_DIR") or Path.home() / ".password-store").expanduser()


def mounts() -> dict[str, Path]:
    """Extra stores from AI_GENT_PASS_MOUNTS (`name=path;name=path`)."""
    out: dict[str, Path] = {}
    for part in (os.environ.get("AI_GENT_PASS_MOUNTS") or "").split(";"):
        name, sep, path = part.strip().partition("=")
        if not part.strip():
            continue
        if not sep or not MOUNT_RE.fullmatch(name) or not path.strip():
            raise StoreError(f"AI_GENT_PASS_MOUNTS: {name or part!r} isn't name=path (lowercase name)")
        out[name] = Path(path.strip()).expanduser()
    return out


def check(entry: str) -> str:
    if not ENTRY_RE.fullmatch(entry) or ".." in entry.split("/"):
        raise StoreError(f"{entry!r} isn't a pass entry name (no leading - or /, no ..)")
    return entry


def resolve(entry: str) -> tuple[Path, str]:
    """The store holding the entry, and its name inside that store."""
    check(entry)
    first, _, rest = entry.partition("/")
    m = mounts()
    if first in m and rest:
        return m[first], rest
    return personal_store(), entry


def has(entry: str) -> bool:
    """Whether the entry exists: a file check, nothing decrypted, no fetch."""
    store, name = resolve(entry)
    return (store / f"{name}.gpg").is_file()


def recipients(entry: str) -> list[str]:
    """Fingerprints from the nearest .gpg-id up from the entry's folder (pass's rule)."""
    store, name = resolve(entry)
    folder = (store / name).parent
    while True:
        gid = folder / ".gpg-id"
        if gid.is_file():
            return [ln.strip() for ln in gid.read_text().splitlines() if ln.strip() and not ln.startswith("#")]
        if folder == store or store not in folder.parents:
            return []
        folder = folder.parent


# ---------------------------------------------------------------------- sync


def _git(store: Path, *args: str, timeout: float = 30) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(["git", "-C", str(store), *args], capture_output=True, text=True,
                              timeout=timeout, stdin=subprocess.DEVNULL)
    except (OSError, subprocess.TimeoutExpired) as e:
        return subprocess.CompletedProcess(["git"], 124, "", str(e))


def _synced(store: Path) -> bool:
    return (store / ".git").exists() and bool(_git(store, "remote").stdout.strip())


def freshen(store: Path, force: bool = False) -> str:
    """Fetch and fast-forward a synced store when it's stale; '' or a warning."""
    if not _synced(store):
        return ""
    window = float(os.environ.get("AI_GENT_PASS_FRESHNESS", "300") or 0)
    if window <= 0 and not force:
        return ""
    head = store / ".git" / "FETCH_HEAD"
    if not force and head.exists() and time.time() - head.stat().st_mtime < window:
        return ""
    out = _git(store, "pull", "--ff-only", "--quiet")
    if out.returncode != 0:
        return (f"the store at {store.name} couldn't fast-forward (diverged or offline); using the local copy. "
                f"Resolve it yourself — never merge .gpg files: pull, then redo your own change on top")
    return ""


def publish(store: Path, message: str) -> str:
    """Commit (when gpg wrote the file) and push a synced store after a write."""
    if not (store / ".git").exists():
        return ""
    if _git(store, "status", "--porcelain").stdout.strip():
        _git(store, "add", "-A")
        _git(store, "commit", "-q", "-m", message)
    if not _synced(store):
        return ""
    out = _git(store, "push", "--quiet", timeout=60)
    return "" if out.returncode == 0 else f"committed, but the push failed: {out.stderr.strip()[-200:]}"


# ---------------------------------------------------------------- read/write


def _no_gui_wsl() -> bool:
    try:
        wsl = "microsoft" in Path("/proc/version").read_text().lower()
    except OSError:
        return False
    return wsl and not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"))


def read(entry: str, timeout: float = 90) -> tuple[str, list[str]]:
    """The secret (first line) and the entry's other lines. Never printed."""
    store, name = resolve(entry)
    warning = freshen(store)
    if warning:
        print(f"warn: {warning}", file=sys.stderr)
    if _no_gui_wsl():
        timeout = min(timeout, 15)  # no pinentry can show: fail fast
    env = {**os.environ, "PASSWORD_STORE_DIR": str(store)}
    if shutil.which("pass"):
        argv, unlock = ["pass", "show", name], f"pass show {entry} >/dev/null"
    elif shutil.which("gpg"):
        argv = ["gpg", "--quiet", "--decrypt", str(store / f"{name}.gpg")]
        unlock = f"gpg --decrypt \"{store / (name + '.gpg')}\" > {'NUL' if os.name == 'nt' else '/dev/null'}"
    else:
        raise StoreError(f"{entry} is in a pass store, but neither pass nor gpg is installed")
    try:
        out = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=timeout, env=env)
    except subprocess.TimeoutExpired:
        out = subprocess.CompletedProcess(argv, 124, "", "timed out waiting for the passphrase")
    lines = out.stdout.splitlines() if out.returncode == 0 else []
    secret = lines[0].strip() if lines else ""
    if not secret:
        why = "it's empty" if out.returncode == 0 else (out.stderr.strip().splitlines() or ["locked or missing"])[-1][:200]
        raise StoreError(f"couldn't read {entry} ({why}). If the key is locked, unlock it in your own terminal "
                         f"(`{unlock}`) and retry")
    return secret, lines[1:]


def write(entry: str, value: str, message: str = "") -> str:
    """Add a new entry (never overwrites); '' or a warning about the push."""
    store, name = resolve(entry)
    warning = freshen(store, force=True)
    if warning:
        raise StoreError(warning)
    if (store / f"{name}.gpg").exists():
        raise StoreError(f"{entry} already exists")
    env = {**os.environ, "PASSWORD_STORE_DIR": str(store)}
    if shutil.which("pass"):
        out = subprocess.run(["pass", "insert", "--multiline", name], input=value + "\n", capture_output=True,
                             text=True, timeout=60, env=env)
    elif shutil.which("gpg"):
        rcpt = recipients(entry)
        if not rcpt:
            raise StoreError(f"no .gpg-id above {entry}; initialise the store with pass init (or gpg-id) first")
        target = store / f"{name}.gpg"
        target.parent.mkdir(parents=True, exist_ok=True)
        argv = ["gpg", "--quiet", "--batch", "--yes", "--encrypt", "--output", str(target)]
        for r in rcpt:
            argv += ["--recipient", r]
        out = subprocess.run(argv, input=value + "\n", capture_output=True, text=True, timeout=60)
    else:
        raise StoreError("neither pass nor gpg is installed")
    if out.returncode != 0 or not (store / f"{name}.gpg").exists():
        raise StoreError(f"writing {entry} failed: {out.stderr.strip()[-300:]}")
    return publish(store, message or f"Add given password for {name} to store.")
