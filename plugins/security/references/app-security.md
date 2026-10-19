# Application security checklist

Shared by `security:red` and `security:blue`. Based on the OWASP Top 10
(2021), the OWASP API Security Top 10, and ASVS's categories, condensed to
what's actually testable without a dedicated scanner subscription.

## Attack surface

- **Authentication** — credential storage (bcrypt/argon2/scrypt, never a
  fast hash), session fixation/expiry, lockout and rate limiting on
  login, MFA bypass paths, password reset token entropy and expiry,
  OAuth2/OIDC flow misuse (`devcontainer:infra` has the flow-shape detail
  for a project that simulates an IdP locally).
- **Authorization** — broken object-level access (IDOR: can user A read
  or write user B's resource by changing an id), broken function-level
  access (a low-privilege role reaching an admin endpoint), multi-tenant
  isolation (one tenant's query touching another's rows), privilege
  escalation through a mutable claim or role field.
- **Injection** — SQL/NoSQL, OS command, LDAP, XPath, template
  (SSTI), XML external entities (XXE), unsafe deserialization. Test with
  the app's real input shapes, not just a textbook payload.
- **Client-side** — reflected/stored/DOM XSS, CSRF (missing token or
  `SameSite`), clickjacking (missing `X-Frame-Options`/`frame-ancestors`),
  CSP gaps, open redirect.
- **Server-side request forgery (SSRF)** — any feature that fetches a
  URL the user influences (webhooks, image/link previews, PDF
  generation) reaching internal services or cloud metadata endpoints.
- **Business logic** — race conditions on balance/inventory/coupon
  checks, price or quantity tampering, workflow step skipping, replay of
  a one-time action.
- **Cryptography** — weak or custom algorithms, hardcoded keys, insecure
  randomness for tokens/ids, missing or weak TLS config, secrets in
  comparison without constant time.
- **File handling** — unrestricted upload (type, size, content
  sniffing), path traversal on filenames, SSRF through PDF/image renderers.
- **API-specific** — BOLA/BFLA (API's shape of IDOR/function-level
  access), mass assignment (extra fields in a request body silently
  updating protected columns), excessive data exposure in a response,
  missing rate limiting, GraphQL introspection/depth/batching abuse.
- **Dependencies** — known CVEs in direct and transitive dependencies;
  hand off to `devsecops:supply-chain` rather than duplicate its scan.
- **Error handling & logging** — stack traces or internal paths leaked
  to the client, missing audit trail for sensitive actions (login,
  permission change, payment), secrets written to logs.

## Baseline (the blue-side half)

Secure framework defaults over custom security code; security headers
(`Content-Security-Policy`, `Strict-Transport-Security`,
`X-Content-Type-Options: nosniff`) on every response; input validation
at the trust boundary, not only client-side; output encoding matched to
the sink (HTML, attribute, URL, JS); dependencies pinned and scanned
(`devsecops:supply-chain`); every authz decision made server-side,
re-checked on every request, never trusted from a client-supplied field.

## Tools, when available

Burp Suite or OWASP ZAP for proxying and active scanning; `sqlmap` only
against an in-scope target; `semgrep`/`CodeQL` for pattern-based static
review backing up manual reading. Treat tool output as leads to confirm,
the same way `devsecops:audit` treats `zizmor`/`poutine`/Scorecard output.
