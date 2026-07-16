# Relay Service

This crate owns the reusable Remote Connect relay runtime shared by standalone
and embedded hosts.

## Ownership

- Room and device state, account provisioning and sync storage, HTTP/WebSocket
  routes, and memory/disk web asset stores belong here.
- Standalone host binding, environment configuration, static-file fallback,
  process lifecycle, and administrative CLI parsing/output remain in the app.
- The existing embedded host still binds TCP, installs its static fallback,
  and manages its task lifecycle in assembly as a compatibility path. That is
  follow-up ownership debt, not part of this reusable service.
- Hosts supply the version reported by the shared health and info routes.
- Keep the relay runtime zero-knowledge: it persists encrypted payloads,
  derived hashes, and wrapped keys. Operator provisioning may generate a master
  key only to wrap it before storage; plaintext keys must not be retained.

## Boundaries

- Do not depend on assembly, interface, or application crates.
- Standalone and embedded hosts must construct the same router from this crate.
- Do not introduce host-specific APIs or duplicate the relay runtime per host.

## Verification

Run `cargo test -p bitfun-relay-service` and
`node scripts/check-core-boundaries.mjs` after changes.
