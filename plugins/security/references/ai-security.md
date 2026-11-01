# AI / LLM security checklist

Shared by `security:red` and `security:blue`, for any application with an
LLM, agent or retrieval component. Based on the OWASP Top 10 for LLM
Applications and MITRE ATLAS, plus the agent-specific risks this
toolkit's own pattern (tools, "ask first," sandboxing) exists to manage.

## Attack surface

- **Prompt injection** — direct (the user types an override), indirect
  (an injected instruction arrives through a fetched web page, a
  retrieved document, a tool's output, or another agent's message) and
  chained (one injection sets up a second). Test both: what a direct
  jailbreak attempt gets through, and what a poisoned document or tool
  result can make the system do.
- **Sensitive information disclosure** — system prompt leakage, training
  or fine-tuning data surfacing in outputs, PII appearing in a response
  that shouldn't have access to it, cross-tenant leakage through a
  shared context, cache or vector store.
- **Excessive agency** — tool permissions wider than the task needs, a
  consequential action (a write, a send, a payment, a deletion) taken
  without the human confirmation this toolkit's own skills require, no
  per-action rate limit or cost cap, a tool whose output is trusted as
  an instruction rather than data.
- **Supply chain** — an unpinned or unverified model, a third-party
  plugin/tool/MCP server with broader reach than declared, a dataset or
  fine-tune of unknown provenance.
- **Data and model poisoning** — a RAG source or fine-tuning set an
  attacker can write to (a public wiki, a scraped feed, user-submitted
  content indexed without review).
- **Improper output handling** — a model's output used unsanitized
  downstream (rendered as HTML, run as a shell command, interpolated
  into a SQL query, passed straight to another tool) — this is a normal
  injection vulnerability wearing an AI costume; the fix is the same as
  `app-security.md`'s input-validation rules, applied to model output.
- **Vector/embedding weaknesses** — a shared vector store leaking across
  tenants, embeddings reversible to source text, retrieval poisoning
  through crafted documents ranked to outrank the real answer.
- **Unbounded consumption** — a prompt crafted to maximize token/compute
  cost or loop length; this is a resource-exhaustion attack, so testing
  it stays capped and local (never run an uncapped loop against a real
  billed endpoint, in or out of scope).

## Agent-specific (this toolkit is one)

- **Confused deputy via tool output** — a tool's result (a file's
  contents, a web page, an issue body) treated as an instruction instead
  of data; this is how indirect prompt injection actually lands.
- **Insufficient sandboxing** of a code-execution or shell tool — can the
  model reach outside its intended scope (the filesystem, the network,
  other sessions)?
- **Secrets exposure through a tool result** — a tool call that echoes a
  credential into the model's context, where it can then be repeated,
  logged or leaked in a later response.
- **Cross-session context or memory poisoning** — persisted memory, a
  shared cache, or a long-lived session state an attacker can write to
  and have replayed into a later, unrelated session.

## Baseline (the blue-side half)

Isolate the system prompt from user and retrieved content; treat every
tool result, fetched page and retrieved document as **untrusted data**,
never an instruction; allowlist tool actions explicitly rather than
exposing everything; require human confirmation for consequential
actions (the same "ask first" pattern this toolkit's own skills follow);
rate-limit and budget-cap agent loops and tool calls; log every tool call
with its input and the model's reasoning for audit; run periodic canary
prompts to catch a regression in the guardrails.

## Testing approach

Build a small, labelled test set per risk (direct jailbreak attempts,
injected instructions via a fetched document, via a tool's output, via
retrieved context) and run it against the system's actual guardrails —
not a bare model. Never run a prompt-injection or resource-consumption
test against a third party's production LLM endpoint without that
vendor's own authorization; that is exactly the kind of remote target
`security:scope` gates.
