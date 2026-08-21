# Local CI Replica — P0 Alignment (dsh / webkit / eslint / RUSTFLAGS)

> Branch: `task/ci-fix` (base `main` @ a7d8cb723) | Author: ci-test-local executor | Date: 2026-08-18
> Mirrors `.github/workflows/ci.yml` 9-job matrix. Before this change the local
> replica covered 5 jobs / 28 steps; after it covers 6 jobs / 36 steps.

## Changes (all in `scripts/ci/local-replica.ps1`)

| # | Gap | Change | Remote baseline |
|---|---|---|---|
| 1 | `dsh-profile-windows` job missing | New Job 5: build profile (`node scripts/prepare-dsh-profile.mjs`) + verify (required files, no nested `node_modules`/`*.map`, stamp digest) | `ci.yml:377-422` |
| 2 | WebKit compatibility contract test missing | New strict step `pnpm run verify:webkit-compatibility:test` | `ci.yml:484-485` |
| 3 | eslint warning gate missing | Lint step now `pnpm --dir src/web-ui exec eslint . --max-warnings=0` | `ci.yml:490` |
| 4 | `RUSTFLAGS=-D warnings` not set | Env prelude sets it (rust-build-check gate; cli-test stays `platform-warn`) | `ci.yml:192` |

## Local verification (2026-08-18)

- dsh profile build: `EXIT 0` — `wrote dist-profile (profile bitfun-acp)`
- dsh profile verify: required files OK / no nested node_modules/.map / stamp `bitfun-acp @ 0.0.1, min dsh 0.1.0-rc.6`
- webkit contract: `tests 2 / pass 2 / fail 0`
- eslint hard gate: `EXIT 0` (zero warnings)
- `cargo check --locked --workspace` with `RUSTFLAGS=-D warnings`: `Finished` `EXIT 0`
- `check-core-boundaries`: 122/122 pass (no regression)

## Notes

- Full 36-step run: 33 PASS / 1 WARN (ppt-live platform diff, pre-existing) /
  1 SKIP (minisign, Windows platform limit) / 1 FAIL (web-ui vitest) — vitest
  FAIL is a pre-existing intermittent exit-code capture issue unrelated to this
  change: the step is untouched, all 518 files / 3860 tests pass, and a clean
  shell re-run of the exact step logic returns `EXIT 0`.
- Remaining P1: cargo-deny licenses `-A no-license-field`, Tauri resource dirs
  pre-creation (`dist`, `src/mobile-web/dist`) — proven needed by this run
  (fresh worktree lacked `dist`, first `cargo check` failed until dirs were created).
