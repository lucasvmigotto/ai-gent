# dbrun spike source

The throwaway Rust program behind the spec 038 report (`../`): a `dbq` runner
over SQLite, PostgreSQL, MySQL and SQL Server, with the golden-case generator.
Kept so the measurements can be repeated. It is not part of the `rust/`
workspace and is not shipped.

Rebuild: `cargo build --release --features all` in this directory (`mssql` and
`parser` are separate features). `gen_golden.py` regenerates `golden.json` from
`plugins/db/scripts/dbrun.py`; `mkdb.py` makes the SQLite test database.
Build output (`target/`), the built binaries and the container ids are not kept.
