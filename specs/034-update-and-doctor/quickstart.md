# Quickstart: how update and doctor are proven

```sh
cd rust && cargo test -p ai-gent --test update_check --test update_apply --test doctor
scripts/test-update.sh          # real install.sh + real binary, file:// releases, throwaway HOME
ai-gent doctor --json | jq .summary
ai-gent update --check --json
```

Manual rehearsal on the owner's machine: `AI_GENT_RELEASE_BASE=file://$PWD/dist AI_GENT_HOME=$(mktemp -d)`; install 9.9.0, build 9.9.1 with `scripts/package.py`, run `ai-gent update --yes`, then `--rollback`.

## Inventory: `scripts/test-update-check.sh` to its replacement (T001)

Filled by T001. The groups to cover: version ordering (line 81), local version resolution (88), repo slug resolution (92), stale refresh and detached refresh (132-139), `ai-gent-update.sh` behaviors (164-210, stay until 3.0.0), hook wiring in a clone and in a package (216-251), pinned silence and unpin (248-251), package update re-running the installer with `latest` and an explicit version (253-262).
