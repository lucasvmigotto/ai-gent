---
name: supply-chain
description: Harden the software supply chain and add security scanning to CI — pinning, Renovate or Dependabot, SCA, SAST, secret, IaC and container scanning, SBOM, signing, SLSA provenance, license policy — with severity gates, GitHub Actions by default. Use for "add security scanning", "generate an SBOM", "sign our images".
---

# Software supply-chain security

## Before starting

1. Read `../../references/principles.md`,
   `../../references/platforms/concepts.md` and the platform's file.
2. Read `../../references/pipeline.md`, the constitution (security
   baseline, e.g. ASVS level), `docs/product/architecture.md` (what gets
   built and shipped: images, packages, static sites, functions), and the
   existing CI and dependency manifests.
3. Inventory what's shipped and from what: languages and package
   managers, lockfiles present or not, base images, third-party CI steps,
   IaC, registries/feeds.

## Controls — pick what applies, in this order

1. **Pin and lock**
   - Lockfiles committed and enforced (`--frozen-lockfile`, `npm ci`,
     `poetry install --no-update`, `cargo --locked`…).
   - Third-party CI steps pinned by SHA/digest; base images by digest in
     release builds.
   - Internal package names protected from dependency confusion (scoped
     names, registry configuration without public fallback for internal
     scopes).
2. **Update continuously** — Renovate (default: works on every platform
   listed) or Dependabot on GitHub; grouped minor/patch updates,
   automerge only for low-risk updates with passing CI, pins included.
3. **Scan code and dependencies**
   - SCA / vulnerability scanning: osv-scanner, Trivy or Grype on
     lockfiles; dependency review on PRs where the platform has it.
   - SAST: CodeQL (GitHub) or Semgrep (any platform) with a curated
     ruleset; fast rules on PRs, full scan scheduled.
   - Secret scanning: platform push protection where available, plus
     gitleaks on PRs and history once.
   - IaC scanning: Trivy config / Checkov on Terraform, Bicep, Kubernetes
     manifests, Dockerfiles.
4. **Harden images** — minimal/distroless or slim bases, non-root user,
   no build tools or secrets in the final stage (multi-stage builds),
   read-only filesystem where possible; scan the built image (Trivy/Grype).
5. **SBOM** — Syft (or the build tool's plugin) producing CycloneDX or
   SPDX for every release artifact; attach it to the release and the
   registry (as an attestation).
6. **Sign and attest** — cosign keyless signing with the CI's OIDC
   identity (Sigstore) or Notation with a cloud KMS; build provenance
   (GitHub artifact attestations or the SLSA generator; equivalent
   attestation steps elsewhere). State the SLSA build level reached.
7. **Verify at deploy** — the deploy job (or an admission controller in
   Kubernetes) verifies signature and provenance against the expected
   identity before rolling out.
8. **License policy** — allow/deny list checked on PRs (e.g. from the SBOM).
9. **DAST** — OWASP ZAP baseline scan against staging on the main
   pipeline or on a schedule; full scan scheduled.
10. **Posture** — OpenSSF Scorecard (GitHub) or the platform's settings
    checklist: branch protection, required reviews, token defaults.

## Gates and exceptions

- **Block**: secrets detected; critical/high vulnerabilities with a fix
  available in shipped (not dev-only) dependencies or the final image;
  denied licenses; unsigned or unverifiable artifacts at deploy; SAST
  findings of high confidence and high severity.
- **Warn and track**: everything else, surfaced in the PR and a tracked
  issue.
- **Exceptions** in a committed file (e.g. `.trivyignore`, Semgrep
  `nosemgrep` with reason, a `security-exceptions.md`) — each with the
  finding id, justification, owner and expiry date; expired exceptions
  fail the build.

## Output

- CI steps/jobs on the right pipelines (PR: fast; main/release: full;
  scheduled: deep) — coordinate with `devsecops:pipeline`.
- Tool configuration files committed (Renovate/Dependabot, Semgrep rules,
  scanner ignore files with reasons).
- A "Supply chain" section in `docs/product/delivery.md`: controls in
  place, SLSA level, gates, how to verify an artifact, exception process.

## Coverage checklist

- [ ] every ecosystem has a committed lockfile enforced in CI
- [ ] every third-party CI step and base image pinned, with an update bot moving pins
- [ ] SCA, SAST and secret scanning on PRs; deep scans scheduled
- [ ] IaC and container images scanned
- [ ] SBOM, signature and provenance produced for every release artifact
- [ ] deploy verifies signature/provenance
- [ ] gates configured by severity; exceptions file with owners and expiries

## Handoff

Controls added, gates, the SLSA level reached, anything that needs
platform settings or licenses the user must enable, and follow-ups.
Commits follow `git:workflow`.
