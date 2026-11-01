# The decision dimensions

Options, defaults and what each must justify, for `project:architecture`'s Decisions step.

1. **Deployment shape** — monolith · modular monolith · frontend +
   API (+ BFF when clients' needs genuinely diverge) · microservices ·
   functions/serverless · hybrid. Default to a modular monolith whose
   modules are the bounded contexts — from `domain-model.md` if it
   exists, otherwise derived from the brief's domain overview and feature
   map (`backend:domain` refines them later and raises an
   `[UPSTREAM GAP]` if they don't fit); justify anything more
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
12. **Stack** — per module: language, runtime and version, framework,
    persistence library, and the frontend stack. Weigh the team's skills,
    the hosting choice's supported runtimes, and the ecosystem the
    integrations need; for web clients, suggest the user's usual stack
    (Bun · React · TypeScript · Vite · Tailwind CSS · Biome) unless a
    driver argues otherwise. `project:spec`'s plans take the stack from
    here. A terminal client (TUI or CLI) is a surface only when the user
    asked for one; its stack follows
    `../../../../frontend/references/terminal-stacks.md` (a client of the API
    may use any language; one embedding the project's code uses its
    language).
