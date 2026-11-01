# Quickstart: proving the 3.0.0 contract

```sh
python3 scripts/release.py --dry-run --json | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["version"], d["bump"])'   # expect: 3.0.0 major
scripts/test-migrate.sh                 # fixture homes: 2.26 package, 2.28 package, clone, mixed, env, pinned
scripts/test-migrate.sh --rollback      # candidate over 2.28.x, then back
scripts/check.sh                        # full, once, at the end
grep -rEn 'wt\.py|token_report\.py|reporun\.py|specsync\.py|compact\.py' plugins shared README.md AGENTS.md docs/site/src   # SC-004
ai-gent doctor --json | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["summary"])'
```

## How the pieces connect

1. 032's `rust.yml` rehearsal produces the six binaries; `package.py` builds the per-target archives; `install.sh` installs one.
2. 033's fixtures and shadow log produce the guard evidence for G28-1.
3. 034's `update` and `doctor` are both the user-facing migration tool and the check that it worked.
4. `scripts/test-migrate.sh` builds each starting state from real history (the 2.26.0 tag) and upgrades it to the candidate package served over `file://`, so the migration is exercised without publishing anything.
5. The gate tables in `contracts/release-gates.md` are the checklist the owner signs.

## What cannot be proven offline

Real GitHub behavior of `gh release edit --latest` on a published release (T051 rehearses it in a scratch repository); Gatekeeper and SmartScreen on real machines (032 de-risk); Codex hook support (T032).
