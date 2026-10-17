"""passstore — how ai-gent's runners read and write secrets in pass
(shared/secrets.md). Linked into each runner's scripts/ directory; imported
only on the host, never inside a runner's client container.

The store is PASSWORD_STORE_DIR (default ~/.password-store). Values are
never printed: callers get them back and redact them.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

ENTRY_RE = re.compile(r"[A-Za-z0-9_@+][A-Za-z0-9._@+-]*(/[A-Za-z0-9_@+][A-Za-z0-9._@+-]*)*")


class StoreError(Exception):
    """A read or write that couldn't be done; the message never holds a value."""


def prefix() -> str:
    return (os.environ.get("AI_GENT_PASS_PREFIX") or "ai-gent").strip("/")


def store() -> Path:
    return Path(os.environ.get("PASSWORD_STORE_DIR") or Path.home() / ".password-store").expanduser()


def check(entry: str) -> str:
    if not ENTRY_RE.fullmatch(entry) or ".." in entry.split("/"):
        raise StoreError(f"{entry!r} isn't a pass entry name (no leading - or /, no ..)")
    return entry


def has(entry: str) -> bool:
    """Whether the entry exists: a file check, nothing decrypted."""
    return (store() / f"{check(entry)}.gpg").is_file()


def read(entry: str, timeout: float = 90) -> tuple[str, list[str]]:
    """The secret (first line) and the entry's other lines. Never printed."""
    check(entry)
    if not shutil.which("pass"):
        raise StoreError(f"{entry} is meant to be in pass, but pass isn't installed")
    try:
        out = subprocess.run(["pass", "show", entry], stdin=subprocess.DEVNULL, capture_output=True,
                             text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        out = subprocess.CompletedProcess([], 124, "", "timed out waiting for the passphrase")
    lines = out.stdout.splitlines() if out.returncode == 0 else []
    secret = lines[0].strip() if lines else ""
    if not secret:
        why = "it's empty" if out.returncode == 0 else (out.stderr.strip().splitlines() or ["locked or missing"])[-1][:200]
        raise StoreError(f"couldn't read {entry} from pass ({why}). If the store is locked, unlock it in your own "
                         f"terminal (`pass show {entry} >/dev/null`) and retry")
    return secret, lines[1:]


def write(entry: str, value: str) -> None:
    """Add a new entry with `pass insert` (never overwrites)."""
    check(entry)
    if not shutil.which("pass"):
        raise StoreError("pass isn't installed; see secrets:pass")
    if has(entry):
        raise StoreError(f"{entry} already exists")
    out = subprocess.run(["pass", "insert", "--multiline", entry], input=value + "\n", capture_output=True,
                         text=True, timeout=60)
    if out.returncode != 0 or not has(entry):
        raise StoreError(f"pass insert {entry} failed: {out.stderr.strip()[-300:]}")
