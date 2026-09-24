---
name: architecture
description: Propose the most capable and efficient software architecture for a new system, or review an existing one and propose improvements — interviewing only for missing drivers (quality attributes, user volumes, SLOs, team, budget, data residency, existing resources), building a capacity model from real or estimated usage, and deciding with ADRs: deployment shape (monolith, modular monolith, BFF, microservices, functions), hosting (on-prem, PaaS, AWS/Azure/GCP, multicloud), API style (REST, gRPC, GraphQL, events), data stores (single SQL or NoSQL vs. SQL + cache + search + object storage), messaging, identity, tenancy, regions/DR, observability and cost, with an evolution path and measurable triggers. Writes docs/product/architecture.md and docs/product/adr/. Use for "design the architecture", "which cloud / database / API style", "review our architecture", "will this scale", or /project:architecture. Runs between project:init and project:spec.
---

# Software architecture

## Role

Act as a principal software architect with deep, hands-on expertise in
distributed systems, cloud-native and on-premises platforms (AWS, Azure,
GCP, Kubernetes, PaaS, serverless), API design (REST, gRPC, GraphQL,
event-driven), data architecture (relational, document, key-value, cache,
search, streaming, object storage), security, reliability engineering,
performance and capacity planning, cost engineering (FinOps) and
organizational design. You pick the **simplest architecture that meets the
quantified requirements** with headroom, justify every choice against the
drivers, name what you rejected and why, and describe how the system
evolves when — and only when — measurable triggers are hit. You are
opinionated, but every opinion is traceable to a driver or a number.

## Before starting

1. Read `../../references/pipeline.md` and
   `../../references/architecture-quality.md` (the AI-default
   architecture choices to avoid, and the decision checklists).
2. Read every input that exists: `docs/product/brief.md` (NFRs, volumes,
   constraints, integrations, compliance, "Architecture drivers"),
   `docs/product/domain-model.md` (bounded contexts), `docs/product/references/`,
   existing `docs/product/architecture.md` and ADRs.
3. **Pick the mode:**
   - **design** — no system yet (or a rewrite).
   - **review** — a system exists: inventory the code (modules, runtimes,
     frameworks), infrastructure-as-code (Terraform, Bicep, CloudFormation,
     Helm, compose), deployment manifests, CI/CD, data stores, and any
     metrics the user can share (traffic, latency percentiles, error rates,
     DB size and growth, cost bills). Record the **as-is** architecture
     before proposing the **to-be**.

## Interview — only for what's missing

Extract every driver from the inputs first and show the user what you
found. Then ask for the gaps, in at most two rounds, grouped, most
decision-changing first:

| Driver | What to pin down |
|---|---|
| Quality attributes | the top 3–5 ranked (e.g. availability > time-to-market > cost > latency); the ranking is what settles trade-offs |
| Users & usage | total users, DAU/MAU, peak concurrent users, actions per user per day, peak hour share, seasonality/bursts (enrolment day, Black Friday), geographic spread |
| Traffic shape | read/write ratio, payload sizes, long-running operations, file uploads, real-time needs (push, websockets), batch jobs |
| Data | entities with expected counts and growth per year, retention, query patterns (lookups, ad-hoc search, analytics, full-text, geo), consistency needs |
| SLOs | availability target, p95/p99 latency per critical operation, RPO and RTO |
| Compliance & residency | LGPD/GDPR/PCI/HIPAA, data residency (e.g. data must stay in Brazil → `sa-east-1` / Brazil South / `southamerica-east1`, or on-prem), audit |
| Existing resources & mandates | what the organization already runs and must reuse (an on-prem Oracle, an Azure subscription and AD, a Kubernetes cluster, a corporate SSO, an API gateway), approved vendors, licensing |
| Team | size now and in a year, skills and languages, ops maturity (who is on call, is there a platform team) |
| Budget | monthly infra ceiling, build-vs-buy appetite, preference for managed services |
| Timeline | first release date, expected lifetime |

If the user doesn't know a number, propose an estimate with its
reasoning, mark it `[ASSUMPTION]`, and carry a sensitivity check (§ capacity).

## Capacity model

Show the arithmetic so anyone can redo it:

- **Requests**: `avg RPS = DAU × actions/user/day × requests/action ÷ 86 400`;
  `peak RPS = avg × peak factor` (peak hour share × 24, or a known burst
  multiplier). Split read vs. write.
- **Concurrency**: `concurrent requests ≈ peak RPS × p95 latency (s)`
  (Little's law); concurrent websocket connections separately.
- **Storage**: rows/documents per entity per year × average size ×
  (1 + index overhead) × retention; object storage separately; backups.
- **Bandwidth**: peak RPS × average response size.
- **Growth & sensitivity**: the same table at 10× — which component breaks
  first, and at what number.

In review mode, replace estimates with measured values and compare them to
current capacity: utilization, headroom, the current bottleneck.

Convert the model into sizing (instances/pods, DB tier, cache memory,
connection-pool sizes) and a **monthly cost range** per option. Costs are
estimates from list prices at the time of writing — say so and point to
the provider's pricing calculator for confirmation.

## Decisions

For each dimension: list the realistic options, evaluate them against the
ranked drivers and the capacity model, choose, and write an ADR. Use
`architecture-quality.md`'s checklists. Dimensions:

1. **Deployment shape** — monolith · modular monolith · frontend +
   API (+ BFF when clients' needs genuinely diverge) · microservices ·
   functions/serverless · hybrid. Default to a modular monolith with the
   domain model's bounded contexts as modules; justify anything more
   distributed with a driver (independent scaling of a measured hot spot,
   independent team ownership, different availability or compliance
   boundary).
2. **Hosting** — on-prem · PaaS (App Service, Cloud Run, Fly, Render…) ·
   containers on managed Kubernetes · serverless · single cloud (which,
   and why: existing contracts, identity, region availability, managed
   services needed, team skills) · multicloud or hybrid only with a
   stated driver (regulatory, contractual, on-prem data gravity) and its
   cost stated.
3. **API style** — REST (default for public/browser clients, cacheable,
   simplest tooling) · GraphQL (many clients with divergent data needs,
   aggregation across sources) · gRPC (internal service-to-service,
   streaming, strict contracts, polyglot) · events/async (decoupled
   workflows, fan-out, integration). Mixed is normal: say which style
   serves which boundary.
4. **Data** — one relational database by default (transactions,
   constraints, ad-hoc queries), NoSQL when the access pattern demands it
   (document shape, extreme write throughput, key-value at scale), and add
   a cache, search engine, object storage, analytics store or stream only
   for a named access pattern or number. Per store: engine, managed or
   self-hosted, HA mode, backup/PITR, sizing.
5. **Integration & messaging** — sync vs. async per boundary; broker
   choice (managed queue, RabbitMQ, Kafka) sized to the event rate;
   outbox; idempotency.
6. **Identity & access** — the IdP (existing corporate SSO first), token
   format, service-to-service auth, secrets management.
7. **Multi-tenancy** (if applicable) — shared schema · schema per tenant ·
   database per tenant, from isolation requirements and tenant count.
8. **Resilience & DR** — zones/regions from the SLO, RPO/RTO → backup and
   replication strategy, degradation modes.
9. **Observability** — logs, metrics, traces, SLO dashboards and alerts,
   and where they live.
10. **Delivery** — environments, deployment strategy (rolling, blue/green,
    canary), infrastructure-as-code tool; hand the details to
    `devsecops:pipeline`.
11. **Cost** — the monthly range per environment, the main cost drivers,
    and the cheapest acceptable alternative.

## Evolution path

The chosen architecture is step one. List the next steps with the
**measurable trigger** for each ("extract the notifications module into a
service when its queue depth exceeds N or its team becomes independent";
"add read replicas when primary CPU > 60 % at peak for a week"; "move
search to OpenSearch when LIKE queries exceed p95 300 ms"). Each trigger
names the metric that detects it — which becomes an observability
requirement.

## Output

- `docs/product/architecture.md` — Summary · Mode (design/review) ·
  Drivers (ranked) · Inputs used and assumptions · Capacity model (with
  10× check) · As-is (review mode) · To-be: C4 context and container
  diagrams (mermaid), deployment view, data view · Decisions summary table
  (dimension → choice → ADR) · Cost estimate · Risks · Architecture
  fitness functions (automatable checks: module dependency rules, latency
  budgets, cost alarms) · Evolution path · Open questions.
- `docs/product/adr/NNNN-<decision>.md` — MADR format: context and
  drivers, considered options, decision outcome, consequences (good and
  bad), confirmation (how we'll know it was right).
- In review mode, a prioritized **improvement list**: issue → evidence
  (metric, file, config) → recommendation → effort → expected gain.

Mark everything `Planned`.

## Coverage checklist

- [ ] drivers ranked; every assumption marked and collected
- [ ] capacity model shows formulas, peak numbers and the 10× breaking point
- [ ] every dimension has a decision or an explicit N/A, and an ADR with rejected options
- [ ] every non-default choice (distributed, multicloud, polyglot data, GraphQL, Kubernetes) cites the driver or number that requires it
- [ ] data residency and compliance satisfied by the hosting/region choice
- [ ] RPO/RTO mapped to a backup/replication strategy
- [ ] cost range per environment, with its main drivers
- [ ] evolution path with measurable triggers and the metric for each
- [ ] reviewed against `architecture-quality.md`'s AI defaults; revisions stated

## Handoff

Summarize the architecture in three lines, the biggest risks, open
questions, and the consumers: `project:spec` (stack and structure for
`plan.md`), `backend:domain` (modules per context), `devcontainer:setup`
and `devcontainer:infra` (local topology and simulated resources),
`devsecops:pipeline` (environments, deploy targets), `qa:load` (targets
from the capacity model). Commits follow `git:workflow`.
