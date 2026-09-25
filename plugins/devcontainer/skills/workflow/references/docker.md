# When the devcontainer itself needs Docker (Testcontainers, Docker-based builds/tests)

Part of `devcontainer:workflow` (`../SKILL.md`), which has the exec
template, the file-vs-environment split and the rebuild procedure.

Trigger: the project's test suite or build needs a Docker daemon
*inside* the devcontainer itself — not just a language toolchain — most
commonly Testcontainers-based integration tests, or any build step that
shells out to `docker`.

Two devcontainer features give a container access to Docker:

- **`docker-outside-of-docker`** — reuses the **host's** daemon. It
  auto-mounts the host socket to `/var/run/docker-host.sock`, creates a
  `/var/run/docker.sock` symlink to it, and wires the container's
  default user into a `docker` group for non-root access. Prefer this
  by default: it's simpler, avoids duplicating image/layer storage
  inside the container, and containers started by tests are visible
  as normal siblings on the host (easier to debug, and Testcontainers'
  Ryuk reaper cleans them up the same way it would outside a container).
- **`docker-in-docker`** — starts a separate, nested `dockerd` inside
  the container, fully isolated from the host's Docker state. Only
  reach for this when isolation from the host daemon is an actual
  requirement (e.g. the tests build/tear down Docker itself, or must
  not see host-side images/containers/networks) — it costs extra disk
  for a redundant image cache and an extra daemon process for no
  benefit in the common case.

**If a project's `devcontainer.json` has both** — a `docker-in-docker`
feature *and* a manual `mounts` bind of `/var/run/docker.sock` — the
bind mount wins in practice: whatever talks to `/var/run/docker.sock`
reaches the host daemon, and the nested `dockerd` the feature provisions
sits unused. Confirm which daemon is actually in play with
`$CONTAINER_ENGINE ps -a` inside the container: if it shows the devcontainer's own
container as a sibling, you're on the host daemon, and the
`docker-in-docker` feature is dead weight that should either be dropped
in favor of `docker-outside-of-docker`, or the redundant manual mount
should go and the setup made intentional one way or the other.

When switching to `docker-outside-of-docker`, remove any manual socket
`mounts` entry — the feature handles that mount itself, and a manual one
alongside it is redundant (harmless if it happens to agree, but
confusing to read and a landmine if the feature's internal path ever
changes).


## Verification checklist after a rebuild that changes Docker access mode

- `devcontainer exec --workspace-folder <path> docker info` (fallback:
  `$CONTAINER_ENGINE exec ... docker info`) — confirm the daemon identity /
  server version matches the host's, not a freshly-provisioned nested
  one.
- Confirm non-root access **explicitly**, not just as root: check
  `id`/`groups` for the container's actual default user (plain
  `devcontainer exec` already runs as that user — don't use
  `$CONTAINER_ENGINE exec -u root`, not with `sudo`), and run a plain `$CONTAINER_ENGINE ps`
  as that user. Root can talk to the socket regardless of group
  permissions, so testing only as root can hide a permissions
  regression that would bite a normal dev session.
- Run the full test suite end-to-end (unit tests *and* the
  Testcontainers/Docker-backed integration tests), not just a Docker
  smoke command — the goal is confirming the actual test workload
  still works, not just that `$CONTAINER_ENGINE ps` succeeds.

## Testcontainers cleanup latency isn't an orphan leak

Right after a test run finishes, `$CONTAINER_ENGINE ps -a` can still show the
spun-up test container(s) and the Ryuk reaper container as `Up` for a
short while (observed ~15s). This is normal reaper latency, not a
leaked/orphaned container. Wait and re-check before concluding cleanup
is broken.
