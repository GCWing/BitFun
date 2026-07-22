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
- Guaranteeing a single hash per third-party crate when Cargo legitimately needs two units (lib vs build-dep).

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
2. **.fingerprint** — group by package stem; keep newest **1** for `bitfun_*` / workspace-app stems, newest **2** for other packages (build-dep dual units). Delete older groups.
3. **deps** — delete artifacts whose trailing hash no longer appears in any remaining fingerprint directory name. Do not delete by “crate name latest only” (unsafe for dual feature units).

### Safety

- Never delete the profile root or final binaries by name (`bitfun-desktop`, `.app`, etc.) except via normal cargo replacement.
- GC failures must not fail the user command (log and continue).
- No dependency on `cargo-sweep` (macOS atime is unreliable).

## Verification

- Unit tests for grouping / keep-latest / deps orphan deletion on a temp fixture.
- `node --test scripts/cargo-target-gc.test.mjs`
- Manual: run GC dry-run against real `target/debug`, confirm incremental crate counts drop to 1 per prefix without requiring a full clean rebuild afterward for desktop:dev.
