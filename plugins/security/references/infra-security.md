# Infrastructure security checklist

Shared by `security:red` and `security:blue`. Cloud, network, container
and IaC posture — where `devsecops:iac`/`devsecops:supply-chain` write
and harden infrastructure, this skill tests whether it actually holds.

## Attack surface

- **Identity and access** — over-privileged roles/policies (wildcard
  actions or resources), long-lived static keys where OIDC federation
  would do, cross-account or cross-project trust relationships, unused
  credentials and roles, public resource policies (a bucket, a
  queue, a repository meant to be private).
- **Network** — security groups/firewall rules open wider than the
  traffic actually needs (`0.0.0.0/0` on a management port), missing
  segmentation between trust tiers, exposed databases or admin
  interfaces, DNS takeover risk from a dangling record.
- **Compute and containers** — privileged containers, host
  filesystem/socket mounts, images run as root without need, exposed
  container-engine sockets, unpatched base images; see
  `../../../shared/containers.md` for the hardening and pinning rules
  those images should already follow.
- **Kubernetes**, when in use — default service account token mounted
  everywhere, missing network policies, permissive RBAC bindings,
  workloads without resource limits, secrets as plain env vars instead
  of a mounted, access-controlled source.
- **IaC posture** — unpinned module/provider versions, state files with
  world-readable storage or no encryption, drift between the IaC and
  what's actually deployed, a plan step that isn't gated before apply;
  `devsecops:iac`'s own skill covers writing this correctly — read its
  output here rather than re-deriving the rules.
- **Secrets management** — static long-lived keys where a short-lived
  OIDC token would do, secrets baked into an AMI/image/userdata, a
  secrets manager with overly broad read policies.
- **Logging and monitoring** — audit logging disabled or not centrally
  shipped (CloudTrail, Activity Log, Cloud Audit Logs), no alerting on
  IAM changes, security-group changes, or root/owner account use.
- **Supply chain of infra** — unpinned Terraform/OpenTofu modules and
  providers, unverified base images, a registry mirror that isn't the
  canonical source.

## Baseline (the blue-side half)

Least-privilege IAM as the default, not an afterthought; private
networking with explicit, narrow allow rules; encryption at rest and in
transit everywhere; images pinned by digest (`containers.md` §3);
infrastructure changes only through IaC with a reviewed plan, never a
console click that then drifts from the code.

## Tools, when available

Cloud posture: Prowler, ScoutSuite, or the cloud's own Security Hub /
Defender for Cloud / Security Command Center. IaC static analysis:
`checkov`, `tfsec` (`devsecops:supply-chain` may already run one of
these in CI — read its findings instead of re-scanning). Containers:
`trivy` for image vulnerabilities, `docker bench security` /
`kube-bench` for runtime posture. Network: `nmap` only against an
in-scope target, never a network you don't control.
