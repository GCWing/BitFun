# DeepReview Runtime Contributions

Use this guide for backend DeepReview changes in `src/crates/core`.

- Runtime changes belong in shared core, without Tauri or desktop-only APIs.
- Keep policy, manifest gate, queue state, retry behavior, task adapter, and
  report enrichment in sync.
- Preserve read-only reviewer execution; remediation requires user approval
  outside the reviewer pass.
- If event or report fields change, update the matching frontend types and UI.
- Run the narrowest relevant Rust checks; avoid broad `cargo` commands unless
  the change requires them.
