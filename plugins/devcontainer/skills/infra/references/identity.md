# Identity — OAuth2, OIDC, SAML, LDAP

Part of `devcontainer:infra`. Section numbers match `../SKILL.md`, which
holds the tier procedure (§2), the catalog (§3), bootstrapping (§6),
compose mechanics (§7), credentials (§8), verification (§9) and gap
documentation (§10).

## 4. OAuth2 / OIDC identity

### 4.1 Classify how the app uses identity first

| App's role | What the dev IdP must provide | Lightest fit |
|---|---|---|
| **Resource server only** — the API validates bearer JWTs, never logs anyone in | an issuer, a discovery document, a JWKS, and a way to mint tokens with chosen claims | mock-oauth2-server |
| **Service-to-service** — `client_credentials` calls to another API | a token endpoint accepting any/known client id+secret | mock-oauth2-server |
| **Browser login** — the client (SPA or server-side) runs authorization code + PKCE | a real login page, redirect URI handling, id_token, usually refresh tokens and logout | mock-oauth2-server's interactive login for "type any user + claims"; Keycloak when the flow needs real users, passwords, consent, logout/session behavior |
| **Admin-side identity logic** — the app manages users/roles/groups via the IdP's admin API, or relies on realms, fine-grained roles, identity brokering | the IdP's actual admin API | Keycloak (tier 1 if prod is Keycloak, tier 2 otherwise — say which) |
| **Enterprise SSO** — SAML, LDAP/AD bind or lookup | §4.6 | Keycloak / an LDAP image |

A project with a split API + client (see `devcontainer:setup`) usually
needs **both** the resource-server row (for `api`) and the browser-login
row (for `web`) from the **same** IdP instance, so tokens the client gets
are ones the API accepts.

### 4.2 The tools

- **`ghcr.io/navikt/mock-oauth2-server`** — a real OAuth2/OIDC server with
  no users to manage. Each URL path segment is its own issuer
  (`http://auth:8080/<issuerId>` with its own
  `/.well-known/openid-configuration`, `/jwks`, `/token`, `/authorize`),
  so several audiences/issuers come from one container. Supports
  `authorization_code` (with PKCE), `client_credentials`, `refresh_token`,
  JWT-bearer and token-exchange grants. `interactiveLogin: true` shows a
  form where you type any subject plus a JSON claims blob. Deterministic
  claims per request go in `tokenCallbacks` in its JSON config
  (`JSON_CONFIG` env var or `JSON_CONFIG_PATH` mounted file). Fastest
  option by far; the gap is that it has no user store, no admin API and no
  realistic session/logout.
- **Keycloak (`quay.io/keycloak/keycloak`)** — run `start-dev
  --import-realm` with a realm export committed to the repo, mounted at
  `/opt/keycloak/data/import/`. The realm file declares clients (with the
  dev redirect URIs), roles, groups and test users, so a fresh container
  is ready to log in with no clicking. Bootstrap admin via
  `KC_BOOTSTRAP_ADMIN_USERNAME`/`KC_BOOTSTRAP_ADMIN_PASSWORD` (older
  releases used `KEYCLOAK_ADMIN*` — check the pinned version). Heavier
  (JVM, ~several hundred MB RAM, slower boot) — pick it for the rows in
  §4.1 that need it, not by default.
- **Dex (`ghcr.io/dexidp/dex`)** — small OIDC provider driven by one YAML
  file (`staticClients`, `staticPasswords`, or a connector to LDAP/GitHub).
  A middle ground when you want real login with fixed users but not
  Keycloak's weight.
- **Firebase Auth / Cognito** — Firebase Auth has its own emulator in the
  Firebase suite (tier 1); Cognito's LocalStack emulation is paid-tier —
  otherwise simulate with mock-oauth2-server (tier 2) and document it.

### 4.3 Match production's token shape, not just "a valid JWT"

The app's authorization code (role mapping, tenant checks, user lookup)
must run **unchanged** against dev tokens, so the dev IdP must emit prod's
claim names and structure. Read them from the app's security config and
any decoded prod token in a runbook — don't guess:

- **Entra ID (Azure AD) v2**: `iss` of the form
  `https://login.microsoftonline.com/<tid>/v2.0`, `aud` = the API's app ID
  URI or client id, `tid`, `oid`, `preferred_username`, `name`, app roles
  in `roles`, delegated scopes in `scp` (space-separated string).
- **Keycloak**: realm roles in `realm_access.roles`, client roles in
  `resource_access.<client>.roles`, `preferred_username`, `email`.
- **Okta/Auth0**: custom claims are often namespaced
  (`https://<your-namespace>/roles`) — copy the exact key.

Configure the dev IdP to produce exactly those (mock-oauth2-server
`tokenCallbacks`, Keycloak protocol mappers/realm roles). Put 2–3 named
test users/personas in the config (an admin, a regular user, a user with
no roles) — they're what makes authorization paths testable, not just
authentication.

### 4.4 The issuer-URL trap (browser vs. container network)

The browser runs on the host and reaches the IdP at
`http://localhost:<port>`; the API container reaches it at
`http://auth:<port>`. Tokens carry whichever URL the IdP saw as its own
in `iss`, and most JWT validators reject a token whose `iss` doesn't
match the configured issuer **exactly**. Pick one of:

- **Split issuer from JWKS in the API config** (usually simplest): validate
  `iss` = the browser-facing URL, but fetch keys from the internal alias.
  Spring: set `jwk-set-uri: http://auth:8080/<...>/jwks` and validate the
  issuer claim separately, instead of `issuer-uri` (which also fetches
  discovery from that URL at startup). Other stacks have the same two knobs.
- **Pin the IdP's public URL**: Keycloak `KC_HOSTNAME=http://localhost:8080`
  (plus `KC_HOSTNAME_BACKCHANNEL_DYNAMIC=true` so backchannel calls from
  `api` work over the alias); mock-oauth2-server derives the issuer from
  the request `Host`, so have both sides send the same host.
- **Make one name resolve everywhere**: route the browser through the
  `devcontainer:proxy` simulation so both sides use the same public URL.

Whatever you pick, write it down in the compose file comments — this is
the single most common "works in Postman, 401 in the app" cause.

### 4.5 Redirect URIs, CORS, and the client config

- Register the **exact** dev redirect URIs (the forwarded dev-server URL,
  and the proxy URL if `devcontainer:proxy` is in play) in the realm
  export / mock config; wildcard redirect URIs hide the path bugs the
  proxy simulation exists to catch.
- The SPA calls the IdP's token endpoint from the browser, so the IdP
  must allow the dev origin (Keycloak client "Web origins"; mock-oauth2-server
  allows CORS by default).
- Keep the client ids/scopes/audiences **the same names as prod** in dev
  config wherever possible; only the authority URL should differ between
  environments.

### 4.6 SAML and LDAP/AD

- **SAML SP** (the app consumes SAML assertions): Keycloak can act as a
  SAML IdP from the same realm import — one container for OIDC and SAML
  if both are needed.
- **LDAP bind/lookup** (the app authenticates against or reads from a
  directory, common with on-prem Active Directory): run an LDAP server
  seeded from a committed LDIF with the same base DN, OU layout and
  attribute names (`sAMAccountName`, `memberOf`, `userPrincipalName`) the
  app queries. `lldap/lldap` is light and fine for plain user/group
  lookups; a full OpenLDAP image is needed for custom schema. AD-only
  behavior (nested-group `LDAP_MATCHING_RULE_IN_CHAIN`, `objectGUID`
  binary handling) is a documented tier-2 gap — note it.
- If the app talks to Keycloak which federates LDAP in prod, mirror that
  topology rather than pointing the app at LDAP directly.

### 4.7 Identity credentials are dev fixtures

Test-user passwords, mock client secrets and the dev admin login are
fixtures, not secrets (§8 of `../SKILL.md`) — commit them in the realm export /
`.env.example` and list them in the README so anyone can log in.
