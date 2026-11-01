# Data Model: Release readiness band

## `release.py --dry-run --json` (prior change, both scripts)

One JSON object on stdout, exit 0; exit 3 (nothing releasable) and 1/2 (error) print nothing on stdout.

| Key | Type | Meaning |
|---|---|---|
| `version` | string | the next version (`X.Y.Z`) |
| `bump` | `"major" \| "minor" \| "patch"` | the largest bump of the commits |
| `since` | string or null | the base tag; null when there is none (first release) |
| `targets` | array | what the release changes |
| `targets[].kind` | `"plugin" \| "manifest" \| "source"` | `plugin`: `release.py`'s per-plugin bumps; `manifest`: its `release.json` files; `source`: `release-single.py`'s single version file |
| `targets[].label` | string | the plugin name or file |
| `targets[].from`, `.to` | string | versions |
| `targets[].bump` | string | the target's own bump (`release-single.py`: same as `bump`) |
| `changelog` | string | the `CHANGELOG.md` line's text (`how`), for display only |

The two scripts print the same keys. `scripts/consistency.py` compares the key lists in both scripts and in `release-model.ts`.

## Atoms (`plugins/git/types/index.d.ts`, plugin `git`)

| Atom | Value | Notes |
|---|---|---|
| `releasePlan` | `{ version, bump, since, plugins } \| { failed: string } \| null` | what the band draws; `plugins` is the count of `kind: "plugin"` targets; shape-tagged (`release-1`) |
| `releasePlanFor` | `string \| null` | the key it was computed for: HEAD sha, a newline, the merged tag list |
| `releaseBandHidden` | `string \| null` | the key at which the person pressed Hide; the band returns when the key changes |

## Rules

- Band text, wide: `next release <version> (<bump>, since <since>)` plus ` · <n> plugin(s) bump` when `n > 0`, plus ` — /release-preview for the plan`; `since` null reads `first release`. Narrow (below `bodyColumns` 60): `next <version> (<bump>)`.
- Priority: `info`.
