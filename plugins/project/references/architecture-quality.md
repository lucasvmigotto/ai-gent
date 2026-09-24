# Architecture quality reference

Used by `project:architecture`. Generated architectures cluster around
recognizable defaults the same way generated UIs and backends do. Each is
right for *some* system — none is right by default. A choice from this list
needs a driver or a number from the capacity model behind it.

## AI-default architecture choices

1. **Microservices for a small team or a new product.** Distributed
   transactions, network failures, N deploy pipelines and N on-call
   surfaces, for a team that could own one modular monolith.
2. **Kubernetes for a handful of containers.** A PaaS or managed
   container service runs 3 services with less to operate; Kubernetes
   pays off with many services, a platform team, or portability needs.
3. **Serverless for steady, high-throughput or long-running work.**
   Cold starts, execution limits and per-invocation cost at scale; great
   for spiky, event-driven, low-duty-cycle work.
4. **GraphQL with one client.** Adds schema stitching, N+1 resolvers,
   caching and authorization complexity without the multi-client payoff.
5. **gRPC to the browser.** Needs gRPC-Web and a proxy; REST/JSON is the
   browser's native language.
6. **Polyglot persistence on day one** — Postgres + Mongo + Redis +
   Elasticsearch before any access pattern needs them. Each store is
   another backup, upgrade, security and consistency story.
7. **Kafka for a few events per minute.** A managed queue or RabbitMQ
   does it; Kafka pays off with high throughput, replay and stream
   processing.
8. **Multicloud "to avoid lock-in"** without a regulatory or contractual
   driver — doubles the platform work and forfeits each cloud's managed
   services. Portability through containers and IaC is usually enough.
9. **Event sourcing / CQRS everywhere** instead of where auditability or
   temporal queries need it.
10. **A cache in front of everything** before measuring what's slow —
    adds invalidation bugs for latency nobody had.
11. **Ignoring what the organization already runs** (its SSO, database
    licenses, cloud contracts, on-prem data center) in favor of a
    greenfield stack.
12. **No numbers** — "scalable", "highly available", "fast" without
    RPS, SLOs, RPO/RTO or cost.
13. **Single region with a 99.99 % promise**, or multi-region
    active-active for an internal tool with a 99.5 % need.
14. **One shared database across services** that are supposed to be
    independent — a distributed monolith.

## Decision checklists

**Deployment shape**
- Which bounded contexts need independent scaling, deployment or ownership
  *today*, with evidence?
- Can module boundaries be enforced in-process (package rules,
  architecture tests) so extraction stays cheap later?

**Hosting**
- Which region satisfies residency and latency for the users?
- Which managed services remove the most operational work for this team?
- What does the organization already pay for or mandate?

**API style**
- Who are the clients (browser, mobile, partners, internal services), and
  what does each need?
- Is caching (HTTP) important? Streaming? Strict cross-language contracts?

**Data**
- What are the top 5 access patterns, their volumes and latency needs?
- Where are transactions across entities required?
- What must be searchable, and how (exact, prefix, full-text, fuzzy, geo)?

**Resilience**
- What does the availability target allow per month (99.9 % ≈ 43 min)?
- RPO/RTO → backup frequency, PITR, replicas, restore drills.

**Cost**
- Cost per environment per month, and cost per active user at 1× and 10×.

## Self-critique questions

- Could a 3-person team run this at 3 a.m.?
- What is the first thing that breaks at 10× load, and what's the plan?
- Which decision is the most expensive to reverse, and is it justified now?
- If the budget were halved, what would change — and why isn't that the
  design already?
- Does every component exist because of a driver, or because it's common?
