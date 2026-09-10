# BitFun 0.2.19 migration fixture

This fixture is a synthetic, de-identified representation of the final BitFun
storage generation before commit `8784bbc4258131b4e74e6f9192a23b43c454fa94`.
It contains one relationship-complete record for each V1 migration domain and
explicitly includes files that must be excluded.

The fixture is source data, not a ready-to-copy OpenBitFun profile. Tests build
SQLite databases from the checked-in SQL, enable WAL where needed, and validate
the fixture through the migration readers before executing a plan.

The Memory SQL mirrors the historical `stage1_outputs` and `jobs` owner schema.
Only `stage1_outputs` contains migratable facts; `jobs` is included to prove
that resumable runtime work state is rebuilt instead of imported.

Supported source range for V1 is `>=0.2.0,<1.0.0`. The fixture's canonical
source revision is `845b4b4d2925f7c41e7e03a4a618606fbd0da8b6`.
Additional canonical versions require evidence captured from a real retired
BitFun build; synthetic version labels must not expand the support matrix.

The source-retention contract is `never_delete_automatically`. Engine and
owner-adapter tests compare source-root file hashes across successful, failed,
and cancelled runs; uninstalling BitFun or deleting its data remains a separate
explicit user action.

All identities, paths, message text, tokens, host keys, and credentials are
synthetic. Secret-bearing owners are represented only by metadata that forces
the migrator to report reauthentication; no usable secret is stored here.
