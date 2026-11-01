#!/usr/bin/env python3
"""Rebuild docs/product/sbom.cdx.json (CycloneDX 1.5) from docs/site/bun.lock.

The lockfile is the source of truth for the docs site's npm components; syft
and cdxgen need not be installed. Run it after the lockfile changes:

    python3 scripts/sbom-from-bunlock.py

The CI `Docs CI` job still builds its own SBOM with anchore/sbom-action; this
one is the committed inventory that project:retrofit starts from.
"""
import json
import re
import subprocess
import uuid
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import quote

ROOT = Path(__file__).resolve().parent.parent
LOCK = ROOT / "docs/site/bun.lock"
PKG = ROOT / "docs/site/package.json"
OUT = ROOT / "docs/product/sbom.cdx.json"
NOTE = ("Python runners (dbrun, reporun, specsync, wt, passstore) are stdlib-only; "
        "dbrun's drivers are fetched at run time by uv, unpinned")


def packages(text: str) -> dict:
    # bun.lock is JSON with trailing commas
    return json.loads(re.sub(r",(\s*[}\]])", r"\1", text))["packages"]


def split(spec: str):
    name, _, version = spec.rpartition("@")
    return name, version


def main() -> None:
    comps = {}
    for entry in packages(LOCK.read_text()).values():
        name, version = split(entry[0])
        if not name or not version or "/" in version:
            continue
        purl = f"pkg:npm/{quote(name, safe='/')}@{version}"
        comps[purl] = {"type": "library", "bom-ref": purl, "name": name,
                       "version": version, "purl": purl}
    old = json.loads(OUT.read_text()) if OUT.exists() else {}
    pkg = json.loads(PKG.read_text())
    commit = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--short", "HEAD"],
                            capture_output=True, text=True).stdout.strip()
    bom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": old.get("serialNumber", f"urn:uuid:{uuid.uuid4()}"),
        "version": old.get("version", 1) + (1 if old else 0),
        "metadata": {
            "timestamp": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
            "tools": {"components": [{
                "type": "application", "name": "scripts/sbom-from-bunlock.py",
                "description": "Reads docs/site/bun.lock; syft and cdxgen are not required"}]},
            "component": {
                "type": "application", "name": pkg["name"], "version": pkg["version"],
                "properties": [{"name": "ai-gent:commit", "value": commit},
                               {"name": "ai-gent:note", "value": NOTE}]},
        },
        "components": sorted(comps.values(), key=lambda c: (c["name"], c["version"])),
    }
    OUT.write_text(json.dumps(bom, indent=2) + "\n")
    print(f"{OUT.relative_to(ROOT)}: {len(comps)} components, {pkg['name']} {pkg['version']}")


if __name__ == "__main__":
    main()
