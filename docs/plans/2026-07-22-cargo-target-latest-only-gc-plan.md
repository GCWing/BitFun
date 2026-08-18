# Cargo Target Latest-Only GC Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On desktop:dev exit and desktop:build end, prune Cargo target caches so only the latest useful artifacts remain for the active profile.

**Architecture:** A shared Node module `scripts/cargo-target-gc.mjs` owns prune logic and CLI. `scripts/dev.cjs` and `scripts/desktop-tauri-build.mjs` call it best-effort after the session/build ends.

**Tech Stack:** Node.js (`node:test`, `fs`, `path`, `child_process`)

## File map

| File | Responsibility |
|---|---|
| `scripts/cargo-target-gc.mjs` | Prune incremental / fingerprint / deps; CLI + exported API |
| `scripts/cargo-target-gc.test.mjs` | Fixture tests for keep-latest rules |
| `scripts/dev.cjs` | Call GC after desktop / preview exit |
| `scripts/desktop-tauri-build.mjs` | Call GC after tauri build |
| `package.json` | `target:gc` script |
| `src/apps/desktop/AGENTS.md` (+ CN) | One short note on GC behavior |

## Tasks

### Task 1: Core GC module + tests

- [x] Write failing tests for incremental keep-latest and fingerprint/deps orphan cleanup
- [x] Implement `scripts/cargo-target-gc.mjs`
- [x] Pass `node --test scripts/cargo-target-gc.test.mjs`

### Task 2: Wire desktop entrypoints

- [x] Hook `dev.cjs` desktop + preview shutdown/`finally`
- [x] Hook `desktop-tauri-build.mjs` after build
- [x] Add `pnpm run target:gc`

### Task 3: Docs + verify

- [x] Brief AGENTS note
- [x] Re-run unit tests; dry-run against real target if present
