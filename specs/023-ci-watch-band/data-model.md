# Data Model: CI watch band

Atoms in `plugins/repo/types/index.d.ts` (created by this feature; plugin `repo`), shape-tagged (`ci-1`).

| Atom | Value | Notes |
|---|---|---|
| `ciRuns` | `{ sha, branch, runs: { id, workflow, name, status, conclusion }[], startedAt } \| null` | the pushed commit and the runs last read (fields from `gh run list --json databaseId,name,status,conclusion,workflowName,headSha`) |
| `ciToasted` | `number[]` | failing run ids already toasted |
| `ciHidden` | `string \| null` | the sha the person hid the band for |
| `ciTimer` | `{ phase: number, failures: number, stopAt: number } \| null` | the back-off phase (30 s, 60 s, 120 s), consecutive failed reads (stop at 3), the hard stop |
| `ciDetect` | `{ platform: string, tokenInPass: boolean, ghSignedIn: boolean } \| null` | `reporun detect --json`'s answer, refreshed at session start or after a failed read |

Marks: `✓` success, `✗` failure/timed_out/startup_failure, `⏳` queued/in progress, `–` cancelled/skipped (neutral, no toast).
Priority for the band-priority rule: `failure` while a run is `✗`, `normal` otherwise.
