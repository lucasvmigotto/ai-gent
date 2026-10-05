# Oracle

- **Driver:** python-oracledb in thin mode (`uv run --with oracledb`) —
  no Oracle Client installation needed. Thick mode (for very old servers
  or native network encryption) needs Instant Client; say so rather than
  installing it — see "Thick-mode fallback" below. No CLI fallback in v1.
- **Session guards (runner):** `SET TRANSACTION READ ONLY` at the start
  of each transaction (remote), `call_timeout` on the connection (Oracle
  has no per-session statement timeout), TCP connect timeout, `module` /
  `action` set to `ai-gent-db` for the DBA's view.
- **Plain plan:** refused on remote targets — `EXPLAIN PLAN` writes to
  `PLAN_TABLE`. Read the plan of an already executed statement instead,
  when the account can: `DBMS_XPLAN.DISPLAY_CURSOR(sql_id)` from
  `V$SQL` (needs `SELECT_CATALOG_ROLE`-like grants). On local containers,
  `EXPLAIN PLAN FOR` + `DBMS_XPLAN.DISPLAY` is fine.

## Row estimate (statistics)

```sql
SELECT owner, table_name, num_rows AS estimated_rows, blocks,
       last_analyzed, partitioned
FROM all_tables
WHERE owner NOT IN ('SYS', 'SYSTEM', 'XDB', 'MDSYS', 'CTXSYS', 'ORDSYS', 'WMSYS', 'OUTLN', 'DBSNMP', 'APPQOSSYS', 'GSMADMIN_INTERNAL', 'AUDSYS', 'LBACSYS', 'OJVMSYS', 'DVSYS')
ORDER BY num_rows DESC NULLS LAST
FETCH FIRST 200 ROWS ONLY;
```

`NUM_ROWS` is as of `LAST_ANALYZED`; a `NULL` means never analyzed.
Segment sizes come from `USER_SEGMENTS` / `DBA_SEGMENTS` when granted.

## Catalog for `db:inspect`

Use the `ALL_*` views (what the account can see); `DBA_*` only if
granted.

| What | Query source |
|---|---|
| version, settings | `V$VERSION` (if granted), `NLS_DATABASE_PARAMETERS` (character set, date format) |
| tables, partitions | `ALL_TABLES`, `ALL_TAB_PARTITIONS`, `ALL_PART_TABLES` |
| columns | `ALL_TAB_COLUMNS` (type, nullable, default, `IDENTITY_COLUMN`) |
| PK, FK, unique, check | `ALL_CONSTRAINTS` (`CONSTRAINT_TYPE` P/R/U/C, `STATUS`, `VALIDATED`, `DELETE_RULE`) + `ALL_CONS_COLUMNS` |
| indexes | `ALL_INDEXES` (`STATUS` = UNUSABLE), `ALL_IND_COLUMNS`; usage only via index monitoring (`DBA_INDEX_USAGE`, 12.2+) if granted |
| views, routines, triggers | `ALL_VIEWS`, `ALL_OBJECTS` (`PROCEDURE`, `FUNCTION`, `PACKAGE`), `ALL_TRIGGERS` (`STATUS`), source in `ALL_SOURCE`; DDL via `DBMS_METADATA.GET_DDL` (read-only) |
| sequences | `ALL_SEQUENCES` (`LAST_NUMBER`) — never `seq.NEXTVAL` |
| synonyms | `ALL_SYNONYMS` — follow them before trusting a name |
| grants | `ALL_TAB_PRIVS`, `USER_ROLE_PRIVS`, `USER_SYS_PRIVS` |
| links | `ALL_DB_LINKS` |
| stats freshness | `ALL_TAB_STATISTICS` (`LAST_ANALYZED`, `STALE_STATS`) |
| legacy/backup tables | `REGEXP_LIKE(table_name, '(_BAK|_BACKUP|_OLD|_TMP|_COPY|_[0-9]{6,8})$')` |

## Local writes

- **DDL commits implicitly.** In v1 the runner accepts only DML plans on
  Oracle, run in one transaction with the expected counts checked; a DDL
  experiment needs a backup taken outside the runner (a container volume
  snapshot with the container stopped, or Data Pump) that the user
  confirms first.
- Common local image: `gvenzl/oracle-free` (service `FREEPDB1`).

## Gotchas

- Empty string is `NULL` in Oracle — "missing" values may be `''` written
  by another system.
- `DATE` has a time part; comparisons against day literals miss rows.
- Disabled or `NOT VALIDATED` constraints don't protect existing data.
- A synonym or a view can make a "table" point somewhere else, even to a
  DB link.

## Thick-mode fallback (only after thin failed)

**Thin mode is always the default and always tried first.** Do not install or
configure anything for Oracle until `dbrun test` has failed in thin mode with:

```
DPY-3015: password verifier type 0x939 is not supported by python-oracledb in thin mode
```

That error means the account's password is stored with the legacy 10G verifier
(typical of old shared development databases whose users were created long ago
and never had their password reset). Thin mode cannot read it; the server and
the credentials are fine. Any other connection error (DNS, refused, timeout,
`ORA-01017`, TLS) is **not** this case — diagnose it normally; thick mode does
not fix it.

The fallback is already in `dbrun`: when thin fails with `DPY-3015` **and**
`AI_GENT_ORACLE_CLIENT_LIB` is set, it calls `oracledb.init_oracle_client`
and retries once in thick mode. Without the variable it stops with the message
that points here. Read-only rules, classification, masking and the log are
unchanged — only the network driver differs.

1. Confirm the failure first, with the variable unset:

   ```bash
   python3 <plugin>/scripts/dbrun.py test D2 --reason "..."
   # FAILED (connect): ... DPY-3015 ...
   ```

2. Install Oracle Instant Client *Basic Light* (x86_64, no root; ~75 MB zip,
   ~35 MB extracted) — ask the user first, the plugin never installs it on its own:

   ```bash
   mkdir -p ~/.local/opt && cd ~/.local/opt
   curl -fsSL -O https://download.oracle.com/otn_software/linux/instantclient/instantclient-basiclite-linuxx64.zip
   unzip -q instantclient-basiclite-linuxx64.zip      # -> instantclient_23_26 (version varies)
   ```

3. The client needs `libaio.so.1`. If `ldd instantclient_*/libclntsh.so` (run with
   `LD_LIBRARY_PATH` set to that directory) still reports it missing and there is no
   `sudo`, put it beside the client (Debian/Ubuntu names it `libaio1t64`):

   ```bash
   cd "$(mktemp -d)" && apt-get download libaio1t64 && dpkg -x libaio1t64_*.deb x
   cp -a x/usr/lib/x86_64-linux-gnu/libaio.so.1t64* ~/.local/opt/instantclient_*/
   ln -sf libaio.so.1t64 ~/.local/opt/instantclient_*/libaio.so.1
   ```

4. Point `dbrun` at it — per command, or in the shell profile to keep it:

   ```bash
   AI_GENT_ORACLE_CLIENT_LIB=$HOME/.local/opt/instantclient_23_26 \
     python3 <plugin>/scripts/dbrun.py test D2 --reason "..."
   # SUCCESS · development · <db name>
   ```

   `dbrun` also adds that directory to `LD_LIBRARY_PATH` for the driver process
   only (the client's own libraries must be on the loader path); nothing else in
   the environment changes.

Limits: x86_64 Linux; applies to remote profiles run by the host's `uv` (the
client container of a *local* database uses thin mode and is not affected). The
permanent fix is on the server side: the DBA resets the account's password so it
gets the current verifier, after which the variable can be dropped.
