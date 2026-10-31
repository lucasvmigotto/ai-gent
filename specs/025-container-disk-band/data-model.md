# Data model — container and disk band

| Entity | Fields | Source |
|---|---|---|
| Finding | kind (`disk` · `dangling` · `volumes` · `buildah`, or any unknown string), count (nullable), free_gb and floor_gb (disk only) | one element of `findings` in `leftovers.sh --json`'s output |
| Containers state | findings[], checked (false when the script reported `checked: false`), reason (nullable: `no-engine` · `not-a-checkout` · `disabled`), readAt, ok (false after a timeout or unparseable output) | the band's one drawn value |

## State atoms

| Key | Plugin | Type | Purpose |
|---|---|---|---|
| `containers.state` | devcontainer | `Shaped<ContainersState \| null>` (tag `containers-1`) | what the band draws |
| `toasted` | devcontainer | existing record | the session-start toast stays once per session (017) |

## `--json` shape (prerequisite change)

```json
{"checked": true, "project": "shop", "findings": [
  {"kind": "disk", "free_gb": 14, "floor_gb": 20, "store": "/var/lib/containers"},
  {"kind": "dangling", "count": 2},
  {"kind": "volumes", "count": 1},
  {"kind": "buildah", "count": 21}]}
{"checked": true, "project": "shop", "findings": []}
{"checked": false, "reason": "no-engine"}
```

Findings come in the script's order (disk, dangling, volumes, buildah); the band keeps it. Counts and numbers only; no sentence travels in the JSON.

## Band wording by kind

| kind | Band wording |
|---|---|
| `disk` | `disk N GB free, floor F` (`free_gb`, `floor_gb`), marked `⚠` |
| `dangling` | `n dangling images` |
| `volumes` | `n volume(s) of a gone worktree` |
| `buildah` | `n buildah (any project)` |
| unknown | `<kind> <count>` |

## Band text

```
containers: 2 dangling images · 1 volume of a gone worktree · 21 buildah (any project) — /containers
containers: ⚠ disk 14 GB free, floor 20 · 3 dangling images — /containers
containers ⚠ 3 — /containers
```
