# Rollback rehearsal (T051) - owner-run, DRAFT

**Status: prepared, not run.** T051 stays unticked until the owner runs this in a scratch repository and attaches the transcript to the release PR. Base assumed: `origin/dev` at 41dff19.

## What it proves

`contracts/release-gates.md` ("Rollback, project level") says that when 3.0.0 is found defective after publishing, you do not move the tag; you mark the release a pre-release and make the last good one latest:

```sh
gh release edit 3.0.0 --prerelease
gh release edit <last 2.x> --latest
```

and that `install.sh` then resolves `latest` to the last 2.x. The flags are marked `[KNOWN]` there with the instruction to run them once in a scratch repo first (T060 in the gate text; it is T051 in `tasks.md`). Only real GitHub can answer whether the second command changes `releases/latest`, so this is a rehearsal against a throwaway repository.

How `install.sh` resolves latest (read from `install.sh`, `resolve_latest`): it does `curl -fsSL -o /dev/null -w '%{url_effective}' https://github.com/lucasvmigotto/ai-gent/releases/latest` and takes the last path segment of the redirected URL. `install.ps1` does the same with a HEAD request. The repository slug is hardcoded in both installers, so they cannot be pointed at the scratch repo; the script asks the API for the same answer (`gh api repos/<repo>/releases/latest --jq .tag_name`), which also works for a private scratch repo. If the scratch repo is public you can also run the installer's own probe by hand (below).

## Prepare

1. Create a throwaway repository, public or private, whose name contains `scratch` (the script refuses any other name), for example `<you>/ai-gent-rollback-scratch`. It needs at least one commit on its default branch (add a README).
2. `gh auth status` shows you logged in with permission to create and edit releases there.
3. Do not use `lucasvmigotto/ai-gent` or any repository called `ai-gent`; the script refuses both, in any letter case.

## Dry run (default, no network)

```sh
scripts/rehearse-rollback.sh <you>/ai-gent-rollback-scratch
```

It prints six steps with the exact `gh` commands and what `install.sh` should resolve after each. It starts nothing.

| Step | Command | install.sh should resolve |
|---|---|---|
| 1 create-good | `gh release view 9.9.0 -R <repo> \|\| gh release create 9.9.0 -R <repo> --title 9.9.0 --notes ...` | unchanged |
| 2 create-bad | the same for `9.9.1` | unchanged |
| 3 arm | `gh release edit 9.9.1 -R <repo> --prerelease=false --latest` | `9.9.1` |
| 4 prerelease | `gh release edit 9.9.1 -R <repo> --prerelease` | anything but `9.9.1` (the previous latest, `9.9.0`, is expected) |
| 5 latest | `gh release edit 9.9.0 -R <repo> --latest` | `9.9.0` |
| 6 undo | `gh release edit 9.9.1 -R <repo> --prerelease=false --latest` | `9.9.1` |

`--good X.Y.Z` and `--bad X.Y.Z` change the two versions. Steps 3 and 6 are not in T051; they make the rehearsal independent of the order the releases were created in and leave the repo in a known state.

## Run

```sh
scripts/rehearse-rollback.sh --run <you>/ai-gent-rollback-scratch --transcript ./t051-transcript.txt
```

It checks `gh auth status`, confirms the repository exists (and that its canonical name is still not the real project), creates the two releases only when absent, runs steps 3 to 6, and after each step that has an expectation asks the API what `latest` is and writes `[ok]` or `[MISMATCH ...]` to the transcript. Exit 0 means every expectation held; 1 means one did not or a command failed; 2 means a refusal. The transcript is never overwritten (the script stops if the file exists). The script never calls `gh release delete`, never deletes a tag, and never runs without `--run <repo>` naming the repository.

Optional check for a public scratch repo, the installer's own probe:

```sh
curl -fsSL --proto '=https' -o /dev/null -w '%{url_effective}\n' https://github.com/<you>/ai-gent-rollback-scratch/releases/latest
```

Run it after step 4 and after step 5 and paste both lines under the transcript. The tag in the URL (`.../releases/tag/9.9.0`) should match the script's answer.

## What to attach and decide

- The transcript file in the release PR (T051 "Verify: transcript in the PR").
- If step 4 or 5 shows `[MISMATCH]`, the rollback text in `release-gates.md` and in the future README section (T052) is wrong and must change before 3.0.0: record what GitHub actually does (for example, `--latest` on a pre-release, or a prerelease still winning) and fix the instructions.
- The two releases can stay in the scratch repo or be deleted by hand later; the script will not do it.

## Not covered

- The user-level rollback (`ai-gent update --rollback`, `sh install.sh --rollback`) is covered by `scripts/test-migrate.sh --rollback` (T050, ticked).
- The crates step (`cargo yank`) is owner-run through the runner pattern and nothing here touches crates.io.
- The script was tested only against a fake `gh` (`scripts/test-rehearse-rollback.sh`), never against GitHub. Whether `--prerelease=false` and `--latest` combine as written is exactly what the rehearsal tests.
