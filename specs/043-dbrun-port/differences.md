# Differences between `dbrun.py` and the Rust runner

Every behavior of `plugins/db/scripts/dbrun.py` that the port changes, or may change, is a row here. A change nobody listed is a bug. Two kinds of row: **Intended** (decided by the owner or by the spec, pinned by a test) and **Question** (found while porting, kept as the Python behavior until the owner answers). Fixtures name their row in a `difference` field and `rust/crates/ai-gent-dbrun/tests/contract.rs` fails if a fixture names a row that is not here. Task owners append rows; the SC-002 mapping of the 51 Python test runs to Rust tests is added to this file as groups land.

## Intended (3.x release that ships the port)

| ID | Item | Python | Rust | Pinned by |
|---|---|---|---|---|
| D-001 | DDL row count (FR-022, plan decision 9) | `cursor.rowcount`, `-1` for DDL and PRAGMA (sqlite3, psycopg, pymssql) | `0` | fixture `13-ddl-after-dml-rowcount` (`expected_port`), `request::rowcount` tests, `SqliteConnector` |
| D-002 | Oracle `NUMBER` (FR-022, plan decision 9) | python-oracledb type (an integer-valued `NUMBER(10,2)` probably prints `5.0`) | typed from its text (`5`; `1E-130` stays a string) | Oracle fixtures of T061 (no fixture yet: SQLite has no `NUMBER`) |
| D-003 | Redaction (FR-008) | host, user, password as whole words, values under 2 characters skipped | password everywhere in the text and in its quoted, escaped, doubled-quote and percent-encoded forms; host and user as whole words, any length, also percent-encoded; one pass, overlapping hits merge (FR-008 requires the redaction of host, user and password; the extra forms are the builder's, see the proposals table) | `core::redact` tests |
| D-004 | Timeout and size limits (FR-011) | none: a timeout of `0` cancels at once on SQLite and means "no limit" in PostgreSQL's `statement_timeout`; a string timeout raises in the middle of the connect; no cap on `max_rows`; no cap on the request | a `connect`-stage error `invalid timeout` unless the timeout is a number from 0.001 to 3600 s (a missing one is 30 s); `invalid max_rows` above 1,000,000; a request line above 16 MiB is a `request`-stage error | fixture `18-bad-timeouts` (port only), `connector::request` tests |
| D-005 | Connection fields of an engine that builds a connect string (FR-009) | `oracle_dsn()` interpolates host, port, service, sid unchecked | an allowlist per field (`core::fields`), refusal names the field, never the value; applied to Oracle (`request::STRING_BUILDING_ENGINES`), other engines pass fields as driver arguments | `core::fields` tests, `request` tests |
| D-006 | A malformed request line (FR-011) | Python has no request-stage error: a traceback | `{"ok":false,"stage":"request","error":...}`, nothing run | `connector::parse_request` tests |
| D-007 | Row counts come from the connection's change counter (FR-010) | `cursor.rowcount`: `-1` for a `WITH ... DELETE` or any statement the driver does not count, `0` for `UPDATE ... RETURNING` (read before the rows are fetched) | the server's own count: a statement that changed rows reports its changes (3 for the CTE delete, 2 for the `RETURNING` update), a read-only statement that returns rows (a SELECT) reports `-1`, DDL, no-change DML and a `RETURNING` that matched nothing report `0` (as Python). Trigger rows are not counted (`changes()` is the statement's own rows; checked in review: an `INSTEAD OF` trigger statement reports `0`, not a stale count); a `changes()` above the total-changes difference is refused. A `PRAGMA` that returns rows but is not read-only reports `0` where Python reports `-1`: open point for T020 | fixtures `21-cte-dml-count-from-the-server`, `14-dml-returning-rowcount` (`expected_port`) |
| D-008 | A plan recorded by the Python runner with `expect -1` for a DDL statement | commits (`-1` equals `-1`) | refused at `verify` with `statement N: expected -1, got 0`, rolled back, nothing created; the user plans again. Refusing is deliberate: accepting `-1` for DDL would also accept `-1` from a statement whose count is unknown | fixture `22-ddl-expect-minus-one` |
| D-009 | A request without `timeout` (FR-011) | `req["timeout"]` raises `KeyError` (a traceback); the CLI always sends one (default 30) | 30 s | `request` tests |
| D-010 | A request without `max_rows` | `req["max_rows"]` raises `KeyError`; the CLI sends 100 (`query`, default `--max-rows 100`), 1000 (`apply`), 1 (identity probe) | 200 (the crate's existing default; Q-008 asks whether it should be the CLI's 100) | `Request::new` |

Added in the second review round (2026-11-14), part of D-MY-01: the lexer refuses any non-ASCII space or control character outside quoted text and counts only ASCII blanks after `--` (the server and Rust disagree on Unicode spaces); read mode refuses, before connecting, any statement that does not start with SELECT, WITH, SHOW, DESCRIBE, DESC, EXPLAIN, TABLE or VALUES (and `EXPLAIN ANALYZE`); after session setup `@@session.sql_mode` is read back and a session whose mode turns on `ANSI_QUOTES` or `NO_BACKSLASH_ESCAPES`, directly or through ANSI, ORACLE, MSSQL, DB2, POSTGRESQL or MAXDB, is refused. The live test `read_only_is_checked_after_every_statement` now checks the allowlist refusals plus the server's own 1792 for a function that deletes (`SELECT wr()`); the connector's "no longer read only" recheck has no live test, because no allowed statement can switch the session to read write, and stays as a backstop.

## Questions for the owner (the Python behavior is what runs today)

| ID | Item | Python | Rust today | Why it may matter |
|---|---|---|---|---|
| Q-001 | `expect` longer than `statements` | the extra entries are ignored and the write commits | same | a plan whose expected counts outnumber its statements passes verification silently; failing closed would be a `verify` error. Pinned as Python by fixture `19-verify-edges` and `request::verify_counts` |
| Q-002 | An empty connection field | `c.get("service") or c.get("database")` treats `""` as unset | `core::fields` refuses an empty value; the profile loaders (T010) should turn a blank field into "absent" | a profile with `SERVICE=` would otherwise be refused at run time |
| Q-003 | Limits chosen here (a builder proposal, see below) | n/a | timeout 0.001 to 3600 s, `max_rows` at most 1,000,000, request line at most 16 MiB (FR-011 says "capped" and gives no number) | the numbers are mine; the owner may want others |
| Q-004 | A timeout that is out of the range of a JSON number (`1e999`) | Python reads it as `inf` and fails in the connect | the JSON parser rejects the line, so it is a `request`-stage error, not `connect` (FR-011 says `connect`) | same outcome (a refusal with no secret), different stage name; fixing it needs a hand-written number scanner |
| Q-005 | Redaction strictness | see D-003 | a one-character password or user is redacted wherever it occurs, which can make an error unreadable; host and user also match in any ASCII case, so a user `a`, `user`, `select` or `sa`, or a host `db`, turns "A table named a is a problem" into "<user> table named <user> is <user> problem" | safe over readable on a credential path; say if a minimum length is wanted |
| Q-006 | Host names in the field allowlist | any text | ASCII only (an internationalized name is given as punycode); IPv6 without brackets | a profile with a Unicode host is refused, with the field named |

## Original behavior pinned by the contract (T002, T005)

Each line is checked against the real `dbrun.py _exec` (process connector, profile `Original`, SQLite) and against the Rust connector (profile `Port`), by the fixture named.

| Behavior of `dbrun.py` | Fixture or test |
|---|---|
| Missing timeout means 30 s (`base_request` always sends one; the Rust request defaults it) | `request` tests |
| Read mode: `fetchmany(max_rows + 1)`, `truncated` is whether one more existed | `02-row-cap`, `16-max-rows-edges`, `request::cap_rows` |
| Write mode: `fetchall()` sliced to `max_rows`, `truncated` present and `false` | `15-write-mode-max-rows`, `16-max-rows-edges` |
| `max_rows` 0: rows `[]`, truncated only when the result is not empty | `16-max-rows-edges` |
| Row count: the driver's `rowcount` (rows returned `-1`, DML its changes, DML with `RETURNING` 0 on SQLite, DDL see D-001) | `13`, `14`, `request::rowcount` |
| `connect` failure: no `results`, `seconds`, `committed`, `server`, `links` | `17-connect-failure-shape` |
| `execute` failure: rolled back, earlier results kept, `seconds` present, no `committed` | `10`, `20-read-failure-keeps-results` |
| `verify` failure: rolled back, every mismatch listed, numbered from 1, `null` skips, no `committed` | `06`, `19-verify-edges`, `request::verify_counts` |
| `test` mode identifies the server and runs no statement | `09` |
| `EXPLAIN` form per engine | `12` |
| Error text is redacted (password, host, user) | `redaction.json`, `core::redact` tests |

## Builder proposals needing owner review (not mandated by the spec)

These are choices the worker made while writing the code. The spec says what to refuse (FR-009, FR-011) but not these specifics; each is open until the owner confirms or changes it.

| Proposal | Where |
|---|---|
| The exact limits: timeout 0.001 to 3600 s, `max_rows` at most 1,000,000, request line at most 16 MiB (Q-003) | `connector::request` |
| The request-error shape for a malformed line, `stage: "request"` (D-006: Python has none, the spec names no stage for it) | `connector::parse_request` |
| `::1` among the loopback names left unredacted | `core::redact` |
| The allowlist of connection fields (ASCII hosts, IPv6 without brackets, `_-.$#` in names) and refusing an empty field (D-005, Q-002, Q-006) | `core::fields` |
| The wider redaction forms of D-003 (dialects of percent-encoding, ASCII case-insensitive host and user) | `core::redact` |
| Oracle as the only string-building engine in `STRING_BUILDING_ENGINES` | `connector::request` |

A future `_exec` reader (the process-side entry point that reads the request from stdin) must bound its own read: the 16 MiB check in `Connector::run_line` runs only after the whole line is in memory, so the reader must stop at the limit while reading.

## Added questions

| ID | Item | Why it matters |
|---|---|---|
| Q-007 | `CountKind::Rows` is `-1` for every engine | that matches `sqlite3` and pymssql only. psycopg3 and PyMySQL report the number of rows for a SELECT and the affected count for `UPDATE ... RETURNING`. The engine groups must decide per engine (their fixtures pin it) and record the choice here; the shared helper only fixes `-1` for the SQLite and SQL Server style |
| Q-008 | Default `max_rows` 200 | the CLI default is 100 |

## Worker A2 entries (T007, T010)

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
- D-A2-22. `get_profile` registers only the password with the process-wide redactor of `ai-gent-core`
  (replacement text `<token>`, values under four characters ignored, substring match). Host and user are
  not registered there, because a substring match mangles ordinary text (user `postgres` turns
  `postgresql` into `<token>ql`; found in review). `dbrun.py` redacted host and user as whole words with
  the labels `<host>` and `<user>`: `Profile::redaction_pairs` offers them for the shared whole-word
  `Redactor` of T004, which T011/T012 must apply to every driver message.
- D-A2-25. Known limits left by review (2026-11-08): `migrate --all` re-checks each value against what was
  loaded only for the single-profile path, so a change between load and the phase-2 read (milliseconds) is
  not detected; the two FIFO tests use a FIFO already in place, so they cannot tell the open-first fix from
  the old metadata-first check (a swap race is not tested); `Authorized::host()` is carried but no driver
  connects to it yet (T011/T014 must connect to `host()`).
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

## MySQL and MariaDB (worker D, T040-T042)

Ids carry `MY` so they cannot clash with the other groups' rows. `D` rows are intended and pinned by a test; `Q` rows keep the Python behavior or the builder's choice until the owner answers.

| ID | Item | Python (PyMySQL) | Rust | Pinned by |
|---|---|---|---|---|
| D-MY-01 | An item that holds more than one statement, an executable comment, a client command, `LOCAL INFILE`, or a change to how text is read | PyMySQL sets neither `CLIENT_MULTI_STATEMENTS` nor `CLIENT_LOCAL_FILES`: the server answers a syntax error (stage `execute`) | refused by the connector before it connects, stage `execute`, fixed text with no part of the statement. The `mysql` crate sets both capabilities and cannot unset them, so without this a second statement would run as one counted item. The connector reads items with its own lexer that follows the server (`mysql::server_view`): `--` needs a following blank, `#` is a comment, `/*! */` and `/*M! */` are code and are refused, a `;` outside text and comments is refused (one trailing is fine), a backslash outside text (`\G`), `DELIMITER`, unterminated quotes and comments, NUL, `sql_mode`, `character_set_client`, `character_set_connection`, `SET NAMES`, `SET CHARACTER SET`, `SET CHARSET` are refused. The shared scanner follows Python and differs from the server (it reads `SELECT 1--1` as a comment and `#'` as a string), so it must not be the only judge on MySQL; a caller that classifies must classify `server_view`'s text. The session drops `NO_BACKSLASH_ESCAPES` and `ANSI_QUOTES`, sets `SET NAMES utf8mb4`, and in read mode checks `@@transaction_read_only` (`tx_read_only` on MariaDB) after every statement, aborting and rolling back when it is not 1 | fixture `24` (`only: port`: 30 refusal steps, the Unicode-space refusals after `--`, a read-mode allowlist refusal, a write-mode CALL with two result sets; 52 steps in all), `connector::mysql` tests `every_adversarial_item_is_refused_before_connecting_and_leaks_nothing`, `text_comments_and_one_trailing_semicolon_are_one_statement`, live `guards`, `read_only_is_checked_after_every_statement`, `the_session_reads_text_as_the_lexer_assumes` |
| D-MY-02 | TLS on every MySQL and MariaDB connection (FR-015) | no `ssl` argument: unencrypted | verify by default (the CA file or the crate's roots); `unverified` only on a proven-local loopback profile; a server that offers no TLS fails naming the cause | `tests/mysql_engine.rs` (`#[ignore]`, needs Podman), `connector::mysql` tests |
| Q-MY-01 | Row count of a SELECT (Q-007 for MySQL) | rows returned (the result is buffered) | kept: the number of rows the SELECT returned, counted past `max_rows` without storing them | fixture `24` |
| Q-MY-02 | Reading a result past the request timeout | PyMySQL buffers everything with no bound but the socket timeout per read | reading rows stops with an `execute` error at the request timeout, and the socket reads and writes are bounded at `timeout + 5` s each. That is NOT a bound on the whole call: when a result set is dropped, the crate reads the rest of its rows and the remaining result sets itself, so a huge or endless result can still hold the runner. The real bound is the child process deadline of T012 (not built yet) | `run_statement` (live test only for the timeout error) |
| Q-MY-03 | Error text | `OperationalError: (1792, 'Cannot execute ...')` | `MysqlError: ERROR 1792 (25006): Cannot execute ...` (the spike's format, kept by the engines script); I/O, driver and TLS errors are `IoError:`, `DriverError:`, `TlsError:` | fixture `24` (`error_contains` the code) |
| Q-MY-04 | TLS trust (FR-015, FR-016): **DECIDED by the owner (2026-11-14): accept and document** | n/a | the `mysql` crate builds its own rustls configuration: a certificate that chains to its bundled Mozilla roots (`webpki-roots`) is accepted in addition to `tls_ca_file`, so a publicly trusted certificate for the host is accepted when only the profile's CA should be; with no CA file the bundled roots are used, not the operating system's store. FR-016 exempts only Oracle. Done: `core::tls` checks the policy and the CA file (read once, bounded; the client gets a private temporary copy of those bytes, so it never reads the path itself and reads no more than the cap); verify mode with no CA file asks `core::tls` for nothing (the system roots would be ignored). Options for the owner: (a) accept and document the wider trust for MySQL and MariaDB (the name check and signature check still hold; an attacker needs a public-CA certificate for the exact host); (b) a different client, or our own handshake over `mysql_common`'s protocol types with `core::tls::client_config` (the MySQL TLS upgrade is a short SSLRequest exchange, but the client's connection and authentication code would be ours); (c) a patch or fork of the crate with a `root_cert_only` option. T042 stays unticked until the owner chooses | no test can show the extra roots; `the_ca_copy_handed_to_the_client_is_the_validated_bytes`, live `tls_checks` | Owner decision: accept the wider trust for MySQL and MariaDB and document it for users (the signature and name checks still hold; the extra trust is the bundled public roots).
| Q-MY-05 | `tls_server_name` | n/a | refused unless it equals `host` (the crate verifies against the host it connects to) | `a_tls_server_name_that_differs_from_the_host_is_refused` |
| Q-MY-06 | A request with no `conn.host` | `host=None`: PyMySQL connects to `localhost` | `connect` failure `missing conn.host` | `missing_host_and_bad_ports_are_refused_before_connecting` |
| Q-MY-07 | A port that is not 1 to 65535 | `int()` of the text, any number accepted by the OS call | refused before connecting (`invalid port`) | same test |
| Q-MY-08 | A TIME or datetime the text cannot parse, invalid UTF-8 in a text column | PyMySQL returns the text as is, or raises on a bad byte | the text as is; invalid bytes become U+FFFD | `time_matches_python_str_of_timedelta` (unparsable TIME); the UTF-8 case has no test |
| Q-MY-09 | DNS resolution of the host | no bound of its own | no bound of its own (the crate resolves with the system call; the 10 s limit covers the TCP connect only) | none: recorded limit |
| Q-MY-10 | Login as an account the server does not have | `1045 Access denied` (PyMySQL speaks `sha256_password`) | MySQL 8.4 answers an unknown user with an `sha256_password` auth switch the `mysql` crate does not implement: `DriverError: Unknown authentication protocol` (stage `connect`, redacted); a wrong password for a real account is still `1045` | `wrong_password` in `tests/mysql_engine.rs` |
| Q-MY-10 (amended) | Login as an account the server does not have, or an account on the `sha256_password` plugin | `1045 Access denied` for the unknown user; PyMySQL speaks `sha256_password` and logs in with it | MySQL 8.4 answers an unknown user with an `sha256_password` auth switch the `mysql` crate does not implement: `DriverError: Unknown authentication protocol` (stage `connect`, redacted). The same limit means an account that really uses `sha256_password` cannot log in at all (the crate supports `mysql_native_password` and `caching_sha2_password`). A wrong password for a `caching_sha2_password` account is still `1045` | `wrong_password` in `tests/mysql_engine.rs` (unknown user only) |
| Q-MY-11 | Advisory RUSTSEC-2025-0134 (`rustls-pemfile` unmaintained), pulled by the `mysql` crate with `rustls` | n/a | `cargo deny check` with `--features ai-gent-dbrun/mysql` fails on it. No ignore entry was added: it is an owner decision (accept with a dated ignore and a reason, or move to a client that uses `rustls-pki-types`). The default build (no engine feature) passes `cargo deny check` | `cargo deny --features ai-gent-dbrun/mysql check` | Owner decision (2026-11-14): ignore RUSTSEC-2025-0134 in `deny.toml` with a dated reason; review when the MySQL client or its TLS dependency changes.
| Q-MY-12 | License `CDLA-Permissive-2.0` (`webpki-roots`, the CA list the `mysql` crate bundles) | n/a | added to `rust/deny.toml`: a permissive license like MIT and ISC (use and redistribution with a notice, no copyleft condition), so it is the same family already allowed | `cargo deny check licenses` |
| Q-MY-13 | The connection character set and SQL modes | PyMySQL: utf8mb4; the server's `sql_mode` | `SET NAMES utf8mb4`; `NO_BACKSLASH_ESCAPES` and `ANSI_QUOTES` are removed from the session so that the connector's lexer and the server read text the same way; a statement that names `sql_mode` or the connection character set is refused | `the_timeout_variable_follows_the_server_family`, live `the_session_reads_text_as_the_lexer_assumes` |
| Q-MY-14 | Images and containers of the live tests | n/a | pinned by tag (`mysql:8.4`, `mariadb:11.4`); TODO pin by digest. Containers carry `me.lucasvmigotto.ai-gent.project=ai-gent` and are removed by ID | `tests/mysql_engine.rs` |
