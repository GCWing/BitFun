# Relay Deploy: Live Logs + Docker Cache Invalidation

**Date:** 2026-07-19  
**Status:** Approved for implementation

## Problems

1. Deploy wizard log pane stays empty until the remote task finishes or fails.
2. Docker image rebuild fails with `cannot find db/admin in bitfun_relay_service`.

## Root Causes

1. Detached `nohup` redirects stdout to a file (full buffering); poll splitter is fragile on CRLF; frontend `setInterval` delays the first poll and can overlap.
2. Dockerfile dependency-cache cleanup uses `deps/bitfun_relay_service*`, which does not match Cargo artifacts `libbitfun_relay_service-*`; the second build links the empty placeholder crate.

## Design

### Docker

In `src/apps/relay-server/Dockerfile`, invalidate placeholder artifacts with globs `*bitfun_relay_service*`, `*bitfun_relay_server*`, `*relay_admin*`, remove matching `.fingerprint` dirs, and `touch` real sources before the second `cargo build`.

### Live logs

- Launch detached tasks with `stdbuf -oL -eL` when available.
- Set `BUILDKIT_PROGRESS=plain` (and compose `--progress=plain` when supported).
- Split poll stdout on `---\n`, `---\r\n`, or a trimmed `---` line.
- Frontend: immediate first poll + serial `setTimeout` chain; seed a waiting line via i18n.

### Out of scope

- WebSocket log push / PTY
- Configurable deploy git ref (default remains GitHub `main`)

## Verification

- Focused Rust check for `bitfun-services-integrations`
- Web type-check
- Local `docker compose build` for relay-server when feasible
