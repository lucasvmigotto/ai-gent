# Jenkins

## Layout

- **Declarative** `Jenkinsfile` in the repo (`pipeline { agent … stages {
  … } post { … } }`); scripted pipelines only where declarative can't
  express it.
- **New pipelines default to `.jenkins/Jenkinsfile`** — set as the job's
  (or Multibranch source's) **Script Path**, since Jenkins only reads a
  root `Jenkinsfile` otherwise. An existing root `Jenkinsfile` is updated
  in place, not moved, unless the user asks — moving it means changing
  that setting too.
- **Multibranch pipelines** / organization folders discover branches and
  PRs from the SCM.
- Reuse with **shared libraries** loaded as `@Library('lib@<tag-or-sha>')`
  — pin them; an unpinned library is code that changes under you.
- Agents: containerized (`agent { docker { image '…@sha256:…' } }`) or
  Kubernetes pod templates; **zero executors on the controller**.

## Skeleton

```groovy
pipeline {
  agent { docker { image '<toolchain-image>@sha256:<digest>' } }
  options { timeout(time: 15, unit: 'MINUTES'); disableConcurrentBuilds(abortPrevious: true) }
  stages {
    stage('check') {
      steps { sh '<lint> && <typecheck> && <test>' }
    }
    stage('deploy staging') {
      when { branch 'main' }
      steps {
        withCredentials([string(credentialsId: 'staging-deploy', variable: 'TOKEN')]) {
          sh 'deploy --token "$TOKEN"'
        }
      }
    }
  }
  post { always { junit '**/test-results/*.xml' } }
}
```

## Secrets and identity

- Jenkins **Credentials**, scoped to folders, bound per step with
  `withCredentials` (single-quoted `sh` strings so Groovy doesn't
  interpolate secrets into the command line).
- Short-lived cloud credentials via a secrets manager/vault plugin or a
  cloud-specific OIDC plugin where available — verify the plugin's
  maintenance status; avoid static cloud keys in credentials.

## Environments and deployment

- `when { branch 'main' }` / `when { tag … }` gates; `input` step for
  manual approval (restrict `submitter`); Lockable Resources (`lock`) for
  deploy serialization; folder-level credentials per environment.

## Supply chain features

- No built-in scanners; run open tools as stages (Semgrep, gitleaks,
  Trivy, osv-scanner, Syft, cosign).
- Keep Jenkins core and **plugins** current — plugins are the largest
  vulnerability surface; remove unused ones; track the Jenkins security
  advisories.

## Attack surface

1. **Untrusted PR `Jenkinsfile`s** — configure the branch source's trust
   setting so PRs from forks/untrusted contributors can't change the
   pipeline that runs with credentials.
2. **Groovy string interpolation of secrets** (`sh "… ${TOKEN}"`) —
   exposes them in process listings and logs.
3. **Builds on the controller** (non-zero executors) — full controller
   compromise from a build.
4. **Script approval / sandbox** bypass — review in-process script
   approvals; don't approve broad signatures.
5. **Replay** and "Build with parameters" letting users run modified
   pipeline code with production credentials — restrict permissions.
6. **Unpinned shared libraries and plugins.**
7. **Persistent agents** reused across trust levels.

## Lint and verify

- Declarative linter (`/pipeline-model-converter/validate` endpoint or the
  CLI), a replay/test on a branch job; no mainstream security linter —
  apply the checklist above manually.

## Observe a run

Used by `devsecops:watch`. Use the REST API with the user's API token
(`curl -u <user>:<token>`); POSTs may also need a CSRF crumb.

| Need | Endpoint (under `<jenkins>/job/<name>/` — nested folders repeat `job/<folder>/`) |
| --- | --- |
| A build's status | `lastBuild/api/json?tree=number,result,building,duration` (`result` is null while it runs) |
| Its console log | `<n>/consoleText` (large: fetch `logText/progressiveText?start=<offset>`) |
| Pipeline stages | `<n>/wfapi/describe` (Pipeline REST API plugin) |
| A stage's log | `<n>/execution/node/<id>/wfapi/log` |
| Credentials | IDs only, in the job configuration or the credentials page; never print one |

- The commit a build used is in its `actions` (Git plugin build data);
  match it to the pushed SHA before trusting "last build".

## Release job

Every project's default release job (Conventional Commits decide the bump,
the pipeline tags) for Jenkins — snippet, push identity, loop guard, tag
pipeline and the one-time grants — is in `../release-platforms.md`.
