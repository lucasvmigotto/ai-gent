# ai-gent-dbrun

The database runner core of [ai-gent](https://docs.lucasvmigotto.me/ai-gent/): the safety rules that decide
whether a statement may run, and the connectors that run it.

- `core` is driver-free: statement splitting and classification (read or write, with the reason), the cost
  rules, column masking, credential redaction and the network helpers. It is a port of the Python
  `dbrun.py` and is checked against golden cases made by running the Python itself.
- `connector` defines the `Connector` trait from the runner's JSON contract (one request in, one JSON
  result line out, errors redacted) and the contract cases every connector must pass.
- `connector::sqlite` (feature `sqlite`, bundled SQLite) is the first connector. A read opens the file
  read-only and sets `query_only`, so the server refuses a write the classifier missed.

## Features

All off by default. `sqlite` has code. `postgres`, `mysql`, `mssql` and `oracle` only reserve their names
until their connectors land.

## Not a general SQL validator

This crate is built for ai-gent's own policy: a conservative, engine-aware read-or-write verdict that errs
toward "write". It is not a SQL parser and does not validate SQL. Against the same 1026-string corpus a
parser-based classifier (`sqlparser`) disagrees on 163 verdicts, 119 of them READ versus WRITE, because
it answers a different question (is this valid SQL, and of what kind) from the one the policy asks (could
this change or lock anything, on this engine). Do not use it to vet untrusted SQL for another system.

## License

GPL-3.0-or-later.
