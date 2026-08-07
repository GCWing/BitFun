## Summary

Fixes #1650

The Edit tool caused extreme token amplification (957K input tokens) and a 1m23s hang on a 1185-line file. Two root causes:

### Fix 1: Remove redundant dry-run from `validate_input`

`validate_input()` in `file_edit_tool.rs` ran a full dry-run `apply_edit_to_content()` — reading the file and generating all whitespace-normalization candidates — and then `call_impl` did the exact same work again when actually applying the edit. This doubled the file reads and candidate generation on every edit.

**Fix:** Removed the dry-run block from `validate_input`. The edit is already validated during `call_impl` via `apply_edit_to_content`, so the dry-run was purely redundant work.

### Fix 2: Add fast path before candidate generation in `apply_edit_to_content`

`edit_string_candidates()` generates whitespace-normalization candidates (tabs↔spaces at width 2 and 4), each calling `find_actual_string()` which does O(n*m) char-by-char scanning. ALL candidates were generated upfront before any matching, even when the exact `old_string` already matched the file content.

**Fix:** Added a fast path that tries the exact `old_string`/`new_string` match via `apply_match_and_replace()` before calling `edit_string_candidates()`. If the exact match succeeds (the common case), it returns immediately — skipping all candidate generation and expensive scanning. If the exact match fails with "not found", it falls through to the existing candidate loop (slow path unchanged for edge cases).

## Validation

- `cargo check -p tool-runtime -p bitfun-core` — passed
- `cargo test -p tool-runtime -- fs::edit_file` — 21/21 passed
- `cargo test -p bitfun-core -- file_edit_tool` — 3/3 passed

## Impact

For the reported 1185-line file edit, the common case (exact match) now completes in a single `apply_match_and_replace` call instead of generating and scanning multiple whitespace-normalization candidates through `find_actual_string`. Combined with removing the redundant dry-run, this eliminates the token amplification and hang.
