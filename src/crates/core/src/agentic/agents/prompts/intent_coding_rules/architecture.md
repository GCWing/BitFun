# Architecture Rules

These rules are long-lived constraints for Coding Agent work in this repository.

## Platform Boundaries

- Keep product logic platform-agnostic, then expose it through platform adapters.
- Do not call Tauri APIs directly from shared UI components.
- Desktop-only integrations belong under `src/apps/desktop`, then flow through transport/API layers.
- Shared core code must avoid host-specific APIs such as `tauri::AppHandle`; use shared abstractions such as `bitfun_events::EventEmitter`.
- Consider remote workspace and remote control synchronization when adding behavior. If a feature cannot support remote scenarios, gate it or show a clear unsupported state.

## Core Changes

- For `bitfun-core` decomposition, feature-boundary, dependency-boundary, or Rust build-speed refactors, read `docs/architecture/core-decomposition.md` before editing.
- Do not confuse DTO or contract extraction with runtime owner migration.
- Moving runtime ownership requires a reviewed port/provider design, old-path compatibility, behavior equivalence tests, and explicit confirmation when behavior boundaries could change.

## Deep Review

- Keep target resolution and manifest construction on the frontend.
- Keep policy validation, queue/retry state, and report enrichment in shared core.
- Keep Deep Review documentation aligned with implementation changes.
