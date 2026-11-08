# Differences between `dbrun.py` and the Rust port

Each entry is a question for the owner unless it says "decided". Worker A2 (T007, T010) entries; the
intended 3.0.0 changes of T005 are added by that task.

## TLS (T007)

- D-A2-01. Field names and keys are proposals: `tls_mode` (`verify` default, `unverified`), `tls_ca_file`
  (absolute path or `~/...`), `tls_server_name`; in a `.env` file `<PROFILE>_TLS_MODE`,
  `<PROFILE>_TLS_CA_FILE`, `<PROFILE>_TLS_SERVER_NAME`. Keep, or rename (for example `sslmode` style)?
- D-A2-02. `tls_ca_file` must be absolute or start with `~/`; a relative path is refused so the result does
  not depend on the directory the runner starts in. Should a relative path resolve against the profiles
  file's directory instead?
- D-A2-03. `unverified` is refused when the profile is DECLARED production (which includes a profile with no
  class at all) even if the local proof succeeds, because the declaration is the owner's word. A profile
  that names a container but no class needs `CLASS=local` to use `unverified`. Intended?
- D-A2-04. `unverified` plus `tls_ca_file` is refused as contradictory (the file would be ignored).
- D-A2-05. The system root store is `rustls-native-certs`; when it yields no root the error names
  `tls_ca_file`. A CA file holding a private key is refused. The CA file is capped at 1 MiB and 128
  certificates.
- D-A2-06. Unverified connections do a full handshake each time (session resumption off) so the warning is
  logged on every connection.

## Profiles (T010)

- D-A2-10. Container names are checked against the engines' own name rule before they reach `inspect`
  (`dbrun.py` passed any string, so `-v` could be read as an option). A profile whose container name does not
  match is "not proven local". Hardening, no valid name is affected.
- D-A2-11. The published-port detection (FR-020) is new: a proven container whose engine port is published on
  a wildcard or loopback address is reached by the Rust driver on `127.0.0.1:<host port>`; anything else
  (no mapping, a mapping bound to another address, host networking, a podman pod's member) takes the
  container exec path. `dbrun.py` always joined the container's network namespace.
  Question: should a mapping on a specific non-loopback address of this machine count as published?
- D-A2-12. `secrets migrate` (single and `--all`) plans first and changes nothing until the plan is safe:
  it refuses a symbolic link profiles file before any pass entry is written, takes the file's size and
  modification time and its text BEFORE writing entries, builds the rewritten text with the same line
  splitter as the parser (Python `splitlines`, every terminator kept), and verifies it (no old plaintext for
  a target, the reference in its place, every other line equal and in order); any failure refuses the
  whole migration. Only then are the pass entries written and the file replaced through a new file
  created exclusively next to it, after a second size/time comparison against the snapshot.
  `dbrun.py` split with `\n` only on rewrite and wrote a `.tmp` file, then chmod.
- D-A2-13. `profiles --export-env` creates the file exclusively with mode 600 from its creation (Unix; see
  D-A2-23), refuses a value with a line break of any kind Python's `splitlines` knows or a control character
  other than tab, and quotes a value with both whitespace and `"` with single quotes (and
  refuses one with both quote kinds); `dbrun.py` wrote such values unquoted-safe only for the simple case. It
  also exports the three TLS keys.
- D-A2-14. Profile files over 1 MiB, with NUL bytes or not UTF-8 are refused (`dbrun.py` read any size and
  crashed on bad UTF-8). Line splitting follows Python's `splitlines`, `\r\n` counting once.
- D-A2-15. TOML syntax errors report line and column only; `tomllib`'s text and the Rust `toml` crate's both
  can quote the offending line, which can hold a value. Messages for unknown keys and tables are the same.
- D-A2-16. At most 64 database containers are inspected for discovery; a container engine answer over 8 MiB is
  treated as a failure (no local profiles).
- D-A2-17. `DOCKER_HOST` or `CONTAINER_HOST` other than `unix://` makes the engine non-local, as before. On
  Windows Docker Desktop reports an `npipe://` endpoint, so no profile can be proven local there by this rule
  (the same as `dbrun.py`). Question: accept `npipe://` as local on Windows?
- D-A2-18. Command log entries for `secrets migrate` belong to the log module (T009): the library returns the
  report and the log line (`MigrateReport::log_line`) for the caller to log.
- D-A2-19. `oracle_dsn` refuses descriptor characters in `host`, `port`, `service` and `sid` (defence in
  depth; the shared validator of `core::fields` is the contract and replaces it when A1 lands).
- D-A2-20. The command word in messages stays `dbrun` here; T011 and T080 rename it.
- D-A2-21. An explicit `pass:<entry>` is validated with the pass store's entry rule at LOAD: an invalid
  reference (leading `-` or `/`, `..`, whitespace, a line break) refuses loading the profiles, naming the
  profile and never the text. `dbrun.py` validated at read time, so `dbrun profiles` still listed the profile.
  Consequence: one bad reference blocks every command until it is fixed. Question: list it with an error
  marker instead?
- D-A2-22. `get_profile` registers the password, host and user with the process-wide redactor of
  `ai-gent-core` (replacement text `<token>`, values under four characters ignored, substring match).
  `dbrun.py` redacted whole words with the labels `<password>`, `<host>`, `<user>`. The labeled form is
  available through `Profile::redaction_pairs` for the callers that want it (the shared redactor of T004).
- D-A2-23. Mode 600 is a Unix guarantee. On Windows the migrate and export files are created exclusively
  but inherit the directory's ACL: owner-only is NOT enforced there and nothing in the code claims it.
  Question: enforce an owner-only ACL on Windows, or document it for the user?
- D-A2-24. Reads of profile and CA files open first (non-blocking on Unix, so a FIFO swapped in cannot
  hang the open) and then check the opened file is regular and under the cap. The container engine's
  output is read by `ai-gent-core`'s `exec::run` to the end and only then compared with the 8 MiB cap: the
  30 s timeout bounds that read, the cap does not bound memory during it.
- D-A2-25. `Authorized` carries the host it was checked against (`Authorized::host`) and `authorize` repeats
  the `unverified` + `tls_ca_file` refusal, so settings built by hand cannot skip it.

## Owner questions (not decided by the worker)

1. Relative `tls_ca_file`: keep refusing it, or resolve against the profiles file's directory? (D-A2-02)
2. A container profile with no `CLASS` is declared production and cannot use `unverified` even when proven
   local: keep, or let a proof make it eligible? (D-A2-03)
3. A published port bound to a specific non-loopback address of this machine: published, or exec path?
   (D-A2-11)
4. `npipe://` (Docker Desktop on Windows): count as local? (D-A2-17)
5. Podman pod members and host networking show no published port in their own `inspect`, so they take the
   exec path: acceptable, or inspect the pod's infra container? (D-A2-11)
6. A bad `pass:` reference blocks loading all profiles (D-A2-21): keep, or list with an error marker?
7. Windows owner-only files (D-A2-23): enforce an ACL, or document?
