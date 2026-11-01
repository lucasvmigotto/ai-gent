# Decision template: `dbrun` and Rust (copy to `decision.md`)

- **Date / author / machines used**:
- **Option chosen**: N (native all) | H (hybrid) | C (containers for networked engines) | S (stay a script that needs uv) | "not yet" (S plus a revisit date)
- **Per engine**: postgresql: native/container/stay; mysql: ; mariadb: ; sqlserver: ; oracle: ; sqlite: native
- **Evidence**: E1..E8 results (pass / fail / not run + reason), linked to `results.md`
- **User-visible requirement after the change** (one sentence):
- **Command word**: `ai-gent db` | `dbrun.py` unchanged
- **Release**: (proposed; the owner decides) and the cutover edit list if the answer is not S: references (`git grep 'dbrun\.py'`: 21 lines in 12 files on 2026-11-01), guard patterns (`plugins/db/hooks/guard.sh`, `opencode/guard/rules.mjs`, `rules.test.mjs`, `scripts/test-db-guard.sh`, `scripts/test-secrets-guard.sh`, `shared/guard-ux.ts` and its copies), the `scripts/check.sh` unit-test block, `scripts/test-dbrun-engines.py`, `plugins/db/references/{runner,engines/oracle,engines/postgresql}.md`
- **Must-not-regress checklist** (spec.md): each item ticked with the test that proves it
- **If the answer is "not yet"**: what stays (`plugins/db/scripts/dbrun.py`, `test_dbrun.py`, the `passstore.py` link, guard patterns, the `check.sh` block, the weekly engines run), what is documented (the db plugin requires `uv` or Podman/Docker; `ai-gent doctor` reports it), what is revisited and when
- **Owner sign-off**: name, date, or "pending"
