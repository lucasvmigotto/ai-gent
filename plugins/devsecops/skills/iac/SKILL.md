---
name: iac
description: Write infrastructure as code for the environments the architecture chose — OpenTofu or Terraform by default, or the project's Pulumi, Bicep or CDK — with modules, per-environment state, OIDC deploy identities, least-privilege IAM, private networking, policy checks and plan-on-PR, apply-on-approval wiring. Use for "write the Terraform", "provision the cloud resources", "infrastructure as code".
---

# Infrastructure as code

Turns the architecture's hosting decisions into reviewable, reproducible
code for the real environments (staging, production, previews). Local
development stays with `devcontainer:infra`; this skill never provisions
anything to make local dev work.

## Before starting

1. Read `../../references/principles.md` and `../../references/pipeline.md`.
2. Read the inputs that exist:
   - `docs/product/architecture.md` and `docs/product/adr/` — hosting,
     regions, topology, data stores, messaging, identity, DR targets
     (RPO/RTO), capacity model (sizes), cost ceiling;
   - `docs/product/delivery.md` — environments, approvals, the CI
     identity and its OIDC trust subjects;
   - `devcontainer:infra`'s compose files — the services the app expects,
     so real resource names and env variables line up with local ones;
   - existing IaC anywhere in the repo — **extend it**, never start a
     parallel stack.
3. Ask what's missing, in one round: cloud accounts/subscriptions/projects
   per environment, who may apply to production, where state lives,
   naming and tagging rules, existing shared resources (network, DNS,
   registries) to reference rather than create.

If the architecture chose a PaaS whose config is the infrastructure
(Vercel, Netlify, Fly.io, Render, Cloudflare Pages), write that platform's
config files and whatever IaC its provider supports, and say that a full
IaC stack would be overkill.

## Tool

Detect before choosing:

| Found | Use |
|---|---|
| `*.tf`, `.terraform.lock.hcl` | Terraform or OpenTofu — keep whichever the project runs (`required_version`, CI images) |
| `Pulumi.yaml` | Pulumi, in the project's language |
| `*.bicep`, `main.bicepparam` | Bicep |
| `cdk.json` | AWS CDK |
| CloudFormation templates | CloudFormation (propose CDK/OpenTofu only if the user wants a move) |
| Helm charts, Kustomize | keep them for Kubernetes workloads; cloud resources still need one of the above |
| nothing | **OpenTofu** (Terraform-compatible, open license); Bicep for an Azure-only system if the team prefers it |

## Layout

```
infra/
  modules/<component>/        one module per architectural component (network, database, app, queue, dns, observability)
  envs/<env>/                 one root per environment: backend config, provider, module calls, tfvars without secrets
  bootstrap/                  state storage + the CI deploy identities, applied once by a human
```

- **State:** remote, encrypted, locked (S3 + DynamoDB or S3 native
  locking, Azure Storage, GCS), one state per environment, access limited
  to the deploy identity and administrators. State can hold secrets:
  treat it as a secret.
- **Pin everything:** tool version, providers (commit the lock file),
  modules by version or commit.
- **Naming and tags:** one convention (`<product>-<env>-<component>`),
  plus tags for environment, owner, cost center and repository on every
  taggable resource.

## Rules

- **Least privilege.** Every workload gets its own identity (IAM role,
  managed identity, service account) with only the permissions it uses.
  No wildcards on actions or resources without a comment saying why.
- **No secrets in code, tfvars or outputs.** Generate them in the cloud's
  secret manager (or reference existing ones) and pass references to
  workloads; mark sensitive outputs.
- **Private by default.** Databases, caches and queues on private
  networks with no public endpoint; ingress only through the load
  balancer or gateway the architecture names; encryption at rest and in
  transit.
- **Protect data.** `prevent_destroy` (or the tool's equivalent) and
  deletion protection on stateful resources; backups and retention that
  meet the architecture's RPO; a tested restore procedure in
  `delivery.md`.
- **Size from the capacity model**, not defaults: instance classes,
  autoscaling bounds, connection limits, storage growth. Note each size's
  source.
- **Observability and cost from day one:** log retention, metrics,
  alerts on the SLOs, and a budget alert at the architecture's ceiling.
- **Environments differ by variables only.** The same modules build
  staging and production.

## Delivery wiring

With `devsecops:pipeline` (it owns the CI files):

- **Pull request:** `fmt -check`, `validate`, `tflint` (or the tool's
  linter), policy checks (Trivy config or Checkov; OPA/Conftest for
  project rules), then `plan` per affected environment with the plan
  shown to reviewers.
- **Main:** apply the reviewed plan to staging; production apply behind
  the protected environment and its approvers, using the saved plan.
- **Scheduled:** drift detection (`plan -detailed-exitcode`) that opens
  an issue or alert on drift.
- CI authenticates with OIDC; the trust is scoped to repository +
  environment (or protected ref). No cloud keys in CI secrets.

## Verify

1. `fmt`, `validate`, the linter and policy checks pass locally.
2. `plan` against a sandbox or staging account **only with the user's
   go-ahead** — it needs real credentials.
3. **Never `apply` or `destroy` on your own.** Applying creates real,
   billable resources, and destroying can lose data: both need an
   explicit order, per environment, and production goes through the
   pipeline.
4. Review the plan for public endpoints, wildcard permissions, missing
   encryption and resources without tags.

## Coverage checklist

- [ ] every component in `architecture.md` maps to a module (or is marked N/A with a reason)
- [ ] one state per environment, remote, encrypted, locked
- [ ] tool, providers and modules pinned; lock file committed
- [ ] per-workload identities, least privilege, no unexplained wildcards
- [ ] no secrets in code, tfvars, outputs or CI variables
- [ ] stateful resources: private, encrypted, deletion-protected, backed up to the RPO
- [ ] sizes traced to the capacity model; budget alert set
- [ ] plan on PR, gated apply, drift detection — wired through `devsecops:pipeline`
- [ ] `bootstrap/` documented as a one-time human step
- [ ] `delivery.md` has the Infrastructure section: environments, state locations, identities, apply procedure, restore runbook

## Handoff

The modules and environments written, what a human must bootstrap (state
storage, deploy identities, DNS delegation), the plan summary if one ran,
estimated monthly cost against the ceiling, and follow-ups. Record gaps in
the architecture as `[UPSTREAM GAP: …]`. Commits follow `git:workflow`.
