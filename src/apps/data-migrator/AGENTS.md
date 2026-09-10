# Data Migrator Agent Guide

Scope: src/apps/data-migrator. This is a separately versioned, offline local tool.
Read README.md for its user-facing contract.

## Boundaries

- Launch with no arguments. Own directory selection, discovery, durable task
  recovery and completion; never depend on a Desktop request or restart Desktop.
- Keep preferences under the tool's Tauri app-config identity. Migration run
  journals/backups remain under the destination data directory for recovery.
  Do not write Desktop onboarding/reminder state.
- Consume config-contracts, shared storage services and legacy-migration-adapters.
  Do not depend on Core, product assembly, Agent execution, Web UI, updater,
  plugin lifecycle or a main-application sibling executable.
- Use typed Rust commands for filesystem/process operations. Cancellation is
  advisory and is honored only at engine-declared safe boundaries.
- Reject unsupported data formats and unsafe directory overlaps. Preserve source
  data, old plans/reports, snapshots, journals, backups and owner conflict policies.
- The UI uses public @openbitfun/design-tokens and @openbitfun/theme-openbitfun
  exports bundled in ui/generated/design-system.css. Regenerate through
  pnpm run data-migrator:theme:generate. Direct Cargo builds are offline;
  Desktop dev/build must never generate or build migrator assets.
- ui/theme.js selects system scheme/contrast before paint. Workflow translations
  are app-owned; do not import Web UI catalogs. No mocks or browser automation
  for visual verification.
- Tool version lives in Cargo.toml and tauri.conf.json. Packaging/signing uses
  the independent Data Migrator workflow and data-migrator-v tags.

## Focused verification

```bash
cargo test -p openbitfun-data-migrator -p openbitfun-legacy-migration-adapters -p openbitfun-legacy-migration --lib
cargo test -p openbitfun-legacy-migration --test migration_engine_contracts
node --test scripts/data-migrator-tauri-build.test.mjs
node --check src/apps/data-migrator/ui/app.js
pnpm run theme:color-audit:all
```

Run pnpm run check:core-boundaries for dependency ownership changes and
pnpm run check:github-config for release workflow changes. Platform packaging
and native visual checks are separate from these contract checks.
