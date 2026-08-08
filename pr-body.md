## Fix: Detect successful tool-call loops and sanitize leaked XML in assistant text (#1492)

### Problem

When LLM inference is abnormal (e.g. DeepSeek), tool call XML tags (`<tool_calls><invoke>`) leak into assistant text. The system detects and executes them, creating a recursive loop that infinitely expands context with repeated successful Write/Read/Exec calls.

### Root Cause

The existing loop protections only track **failed** tool rounds. A loop of **successful** Write/Read/Exec calls trips no detector. Additionally, `write_content_sanitizer.rs` exists to detect/strip leaked XML but had zero production callers — it was never wired into the assistant text path.

### Fix

**1. Detect successful tool-call loops** (`execution_engine.rs`)

- Added `recent_successful_tool_signatures` and `successful_tool_recovery_attempts` tracking alongside the existing failed-tool tracking.
- Modified round-signature tracking to also track successful rounds: when not all tools fail, push to `recent_successful_tool_signatures` and clear the failed streak.
- Added two new detection blocks mirroring the existing failed-tool detectors:
  - **Strict consecutive check**: `tail.windows(2).all(|w| w[0] == w[1])` — detects identical consecutive successful tool calls.
  - **Periodic-pattern check**: `is_periodic_tool_signature_loop()` — detects repeating patterns of successful tool calls.
- Both inject `LoopRecovery`/`PeriodicLoopRecovery` reminders, clear the successful streak, and finalize after max attempts with `finalization_reason = "repeated_successful_tool_calls"`.

**2. Wire `write_content_sanitizer` into assistant text** (`round_executor.rs`)

- Before building `Message::assistant_with_reasoning`, checks `contains_tool_invocation_artifacts(&clean_text)` and if true, strips with `strip_tool_invocation_artifacts(&clean_text)` and logs a warning.
- Changed `clean_text` from `let` to `let mut` to allow mutation.

### Validation

- `cargo check -p bitfun-core` passes (exit code 0).
- Existing `write_content_sanitizer` and `is_periodic_tool_signature_loop` unit tests are unchanged in behavior (these functions were not modified, only called from new sites).

### Testing

Closes #1492
