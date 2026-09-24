# Mail — SMTP, mail APIs, inboxes

Part of `devcontainer:infra`. Section numbers match `../SKILL.md`, which
holds the tier procedure (§2), the catalog (§3), bootstrapping (§6),
compose mechanics (§7), credentials (§8), verification (§9) and gap
documentation (§10).

## 5. Mail — SMTP, mail APIs, inboxes

### 5.1 Classify how the app sends/receives mail

- **SMTP** (Spring Mail, Nodemailer, Django `EmailBackend`, PHPMailer,
  `smtplib`, …) → Mailpit, §5.2.
- **Provider HTTP API** (SES SDK, SendGrid/Postmark/Mailgun SDKs) → §5.3.
- **Reads a mailbox** (IMAP/POP3 polling, bounce processing, ticket
  ingestion) → §5.4.

### 5.2 SMTP capture with Mailpit

`axllent/mailpit`: SMTP on `1025`, web UI + REST API on `8025`. It never
delivers anywhere unless relay is explicitly configured — keep it that way.

- Point the app at `mailpit:1025` by the network alias. Match what the
  app's SMTP client insists on rather than weakening the app's config:
  - app always sends `AUTH` → `MP_SMTP_AUTH_ACCEPT_ANY=true` plus
    `MP_SMTP_AUTH_ALLOW_INSECURE=true` (any username/password accepted);
  - app requires STARTTLS/TLS (common on port 587/465 configs) → give
    Mailpit a self-signed cert via `MP_SMTP_TLS_CERT`/`MP_SMTP_TLS_KEY`
    (optionally `MP_SMTP_REQUIRE_STARTTLS=true`) and trust it / disable
    verification only in the dev profile.
- Keep the SMTP host/port/auth/TLS settings as **env vars the app already
  reads**, so dev and prod exercise the same code path with different
  values.
- Use the REST API for assertions in integration/e2e tests
  (`GET /api/v1/messages`, `GET /api/v1/search?query=to:<addr>`,
  `DELETE /api/v1/messages` to reset between tests) — don't scrape the UI.
- Healthcheck: Mailpit exposes `/livez` and `/readyz` on the HTTP port.
- Useful extras when the team will use them: `MP_MAX_MESSAGES` to cap
  retention, the built-in HTML/link checks for template work, and recent
  versions' chaos option to make SMTP return errors on demand for testing
  retry/bounce handling.
- No persistent volume by default — an empty inbox on restart is the more
  useful state.
- MailHog is the older, unmaintained equivalent; migrate to Mailpit if a
  reference project still uses it.

### 5.3 Transactional-mail HTTP APIs

Mailpit only speaks SMTP, so an app calling a provider's HTTP API needs
something else, in this order:

1. **Vendor sandbox (tier 0)** when online dev is acceptable: SendGrid
   `mail_settings.sandbox_mode.enable=true` (validates, never sends),
   Postmark's `POSTMARK_API_TEST` server token, Mailgun `o:testmode=yes`.
   Beware "sandbox domains" that *do* deliver to allow-listed addresses —
   that's real sending, not a sandbox.
2. **Emulator**: AWS SES via LocalStack (sent messages are retrievable
   from LocalStack's `/_aws/ses` endpoint) — set the SDK endpoint
   override (see Pitfalls).
3. **Stub (tier 3)**: WireMock with the provider's request/response shape
   when the vendor has no sandbox and must be offline.

Don't switch the app to an SMTP transport "just for dev" unless the
transport is already a config choice prod also uses — otherwise dev tests
a code path prod never runs.

### 5.4 Inbound mailboxes

For apps that read mail, `greenmail/standalone` provides SMTP + IMAP +
POP3 (ports `3025`/`3143`/`3110`, SSL variants `3465`/`3993`/`3995`) with
users declared via `GREENMAIL_OPTS` (e.g.
`-Dgreenmail.setup.test.all -Dgreenmail.users=inbox:secret@example.test
-Dgreenmail.hostname=0.0.0.0`). Seed test mail by sending to its SMTP
port. If the app both sends and reads, run Mailpit for outbound and
GreenMail for the mailbox — don't make one do both jobs.
