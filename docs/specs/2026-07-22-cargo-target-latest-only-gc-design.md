# Cargo Target Latest-Only GC

**Date:** 2026-07-22  
**Status:** Approved (timing: exit of `desktop:dev` / end of `desktop:build`)

## Problem

`target/debug` grows without bound across `pnpm run desktop:dev` / build sessions (observed ~139GB). Growth is dominated by:

1. Multiple `incremental/<crate>-<hash>/` roots for the same crate (rustc only GCs sessions *inside* one hash root).
2. Stale `.fingerprint` / `deps` artifacts left after feature or unit-graph changes.

## Goal

After a desktop dev session exits or a desktop build finishes, keep only the latest *useful* cache for the active profile so disk usage stops ratcheting upward, without `cargo clean` and without disabling incremental compilation.

## Non-goals

- Changing default `profile.dev` debuginfo (optional later).
- GC on every incremental rebuild during a live `tauri dev` session.
- Pruning `.fingerprint` directories by keep-newest-N (unsafe; caused full cold rebuilds).

## Design

### Trigger (option B)

| Entry | When GC runs |
|---|---|
| `pnpm run desktop:dev` | After `tauri dev` exits (including Ctrl+C), in a `finally` path |
| `pnpm run desktop:preview:debug` | On preview shutdown |
| `pnpm run desktop:build*` (`scripts/desktop-tauri-build.mjs`) | After `tauri build` returns (success or fail; GC is best-effort) |
| `pnpm run target:gc` | Manual |

Skip GC when `BITFUN_TARGET_GC=0`. Dry-run when `BITFUN_TARGET_GC_DRY_RUN=1`.

Skip when another `cargo` / `rustc` process still appears active (avoid deleting in-use artifacts).

### What is pruned

For `target/<triple?>/<profile>/` (default host triple omitted; profile `debug` for dev, build profile from argv):

1. **incremental** — group directories by crate prefix (name before final `-`); keep the newest mtime; delete older roots. Inside a kept root, keep the newest finalized `s-*` session when multiple remain.
2. **.fingerprint** — **do not mtime-prune**. Cargo may keep multiple concurrent units per package (lib / build-script / feature variants). Deleting a still-referenced fingerprint forces a cold rebuild on the next `desktop:dev`.
3. **deps** — delete artifacts whose trailing hash no longer appears in **any existing** fingerprint directory name (true orphans only). Never delete deps solely because another fingerprint for the same stem is newer.

### Safety

- Never delete the profile root or final binaries by name (`bitfun-desktop`, `.app`, etc.) except via normal cargo replacement.
- GC failures must not fail the user command (log and continue).
- No dependency on `cargo-sweep` (macOS atime is unreliable).

## Verification

- Unit tests for grouping / keep-latest / deps orphan deletion on a temp fixture.
- `node --test scripts/cargo-target-gc.test.mjs`
- Manual: run GC dry-run against real `target/debug`, confirm incremental crate counts drop to 1 per prefix without requiring a full clean rebuild afterward for desktop:dev.
