## Fix: Mark truncated assistant replies as incomplete instead of complete

Closes #1980

### Problem

When the model output stream ends prematurely (provider-side interruption, timeout, connection drop, or `max_tokens` cut), the execution engine marks the dialog turn as `complete` / `hasFinalResponse=true` instead of `interrupted`/`incomplete`. This causes the frontend to persist and display an incomplete assistant reply as if it were a complete, final response.

### Root Cause

In [`execution_engine.rs`](src/crates/assembly/core/src/agentic/execution/execution_engine.rs), three code paths contribute to the bug:

1. **`partial_truncated` marks as final (line ~4464):** When continuation attempts are exhausted after a partial recovery, `finalization_reason` is set to `"partial_truncated"`, but the code then sets `has_final_response = true` — treating the truncated text as a complete final response.

2. **Success calculation excludes `partial_truncated` (line ~4480):** The `success` variable is `has_final_response || matches!(effective_finish_reason, "max_rounds" | "repeated_tool_failures")`. Since `"partial_truncated"` is not in the matches, and after fix #1 `has_final_response` is `false`, the turn would be marked as failed (`success=false`), which is too harsh — the turn did produce partial output.

3. **Cancellation path leaves `finalization_reason = None` (line ~4194):** When `should_continue_after_partial_response(reason)` returns `false` (reason contains "cancelled"), the code breaks immediately without setting `finalization_reason`, so it defaults to `None` → `effective_finish_reason = "complete"` → `has_final_response = true`. A cancelled stream with partial text is reported as complete.

### Fix

Three surgical changes:

1. **Line ~4465:** `has_final_response = true` → `has_final_response = false` for the `partial_truncated` branch. A truncated response is not a complete final response.

2. **Line ~4483:** Add `"partial_truncated"` to the `matches!` list for `success`. The turn produced partial output, so `success=true` with `has_final_response=false` tells the UI: "the turn produced output, but it is incomplete."

3. **Line ~4199:** Set `finalization_reason = Some("partial_truncated")` before `break` in the cancellation path. This ensures a cancelled stream with partial text is reported as `partial_truncated` (incomplete) rather than `complete`.

### Event Flow After Fix

The `DialogTurnCompleted` event now carries for truncated turns:
- `finish_reason: "partial_truncated"` (was `"complete"`)
- `has_final_response: false` (was `true`)
- `success: true` (was `true` for path 1, `false` would have been too harsh)
- `partial_recovery_reason: <actual reason>` (unchanged — e.g., "idle_timeout", "cancelled")

The frontend projection (`frontend_projection.rs`) forwards these fields as `finishReason`, `hasFinalResponse`, `success`, and `partialRecoveryReason`, allowing the UI to distinguish incomplete turns from complete ones.

### Validation

- `cargo build -p bitfun-core --lib` — compiles clean (1 pre-existing warning, unrelated)
- `cargo test -p bitfun-core --lib execution::execution_engine` — all 38 existing tests pass
