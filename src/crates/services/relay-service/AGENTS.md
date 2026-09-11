# Relay Service

This crate owns the reusable Remote Connect relay runtime shared by standalone
and embedded hosts.

## Ownership

- Device state, verified GitHub identity exchange, scoped credentials, HTTP/WebSocket
  routes, and memory/disk web asset stores belong here.
- Standalone host binding, environment configuration, static-file fallback,
  process lifecycle, and administrative CLI parsing/output remain in the app.
- The Desktop embedded host binds TCP, installs its static fallback, and owns
  its task lifecycle in `src/apps/desktop`; assembly controls only product
  start/stop sequencing through a narrow host port.
- Hosts supply the version reported by the shared health and info routes.
- Keep relay messages opaque. Devices retain their own private keys; public-key
  lookup and message routing must enforce authenticated account ownership.
- Keep admission before body buffering and preserve resource permits through
  cancellation and slow-reader failures. Test quota boundaries and isolation.

## Boundaries

- Do not depend on assembly, interface, or application crates.
- Standalone and embedded hosts must construct the same router from this crate.
- Do not introduce host-specific APIs or duplicate the relay runtime per host.
- The independently built identity verifier supplies a ring `ClientConfig` to
  its own Reqwest client. This narrow standalone exception must not install or
  replace the process-wide provider; embedded product hosts retain the
  `services-core::tls_provider` owner. Boundary checks require the explicit
  client binding and forbid `install_default` in this verifier.

## Verification

Run `cargo test -p openbitfun-relay-service` and
`node scripts/check-core-boundaries.mjs` after changes.
