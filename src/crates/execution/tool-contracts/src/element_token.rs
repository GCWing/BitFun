//! Opaque per-snapshot element tokens.
//!
//! ## Why this exists
//!
//! Element-targeting tools historically treat the bare 1-based `element_index`
//! returned by a window-state snapshot as valid until the next snapshot — but
//! there is no formal validity contract. If the underlying accessibility walk
//! ever changes its indexing, the silent failure mode is a misclick: the
//! integer still parses, the element path still resolves *something*, and the
//! action lands on the wrong target.
//!
//! This module adds an opaque token alongside the integer index whose validity
//! is **explicit** and **invalidated cheaply** when the next snapshot
//! supersedes the previous one for the same `(pid, window_id)`.
//!
//! ## Token format
//!
//! ```text
//!   s{snapshot_id_hex}:{element_index}
//! ```
//!
//! - `snapshot_id_hex` is a lowercase 4-hex-char prefix of a process-global
//!   `u32` snapshot counter ([`mint_snapshot_id`]). 4 chars gives 16 bits of
//!   namespace — collisions are statistically impossible inside the
//!   8-entry-per-pid LRU window we keep, and the prefix stays human-eyeball
//!   friendly in logs.
//! - `element_index` is the same `usize` already returned in the structured
//!   elements array. Keeping it in plain sight in the token means a log line
//!   like `element_token=s7a3f:42` is debug-grep-able without a side-table.
//!
//! Tokens are 8–12 chars (`"s0001:0"` up to `"sffff:999"`).
//!
//! ## Validity contract
//!
//! - Snapshot IDs are minted in [`TokenRegistry::register_snapshot`], called by
//!   a platform's window-state implementation immediately after the
//!   accessibility walk lands in the per-platform element cache.
//! - A snapshot is valid until either (a) the LRU evicts it, or (b) a newer
//!   snapshot for the same `pid` pushes it past the LRU cap of
//!   [`LRU_CAP_PER_PID`].
//! - Resolving a stale token returns [`TokenError::Stale`], whose
//!   [`Display`](std::fmt::Display) output equals [`STALE_TOKEN_ERROR`].
//!   Consumers MUST treat that as "re-snapshot and retry", never as
//!   "action failed".
//!
//! The LRU is **per-pid**, not global. Two snapshots from different pids never
//! collide even when their numeric counter happens to wrap (which it won't in
//! practice — `u32` wraps after 4 billion calls).
//!
//! This module is pure Rust (`std` only) and carries no platform dependencies;
//! it lives in the contracts layer so every platform adapter can share the
//! same validity contract.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::sync::OnceLock;

/// LRU cap of valid snapshots retained per pid. Past this point the oldest
/// entry for the pid is evicted and its tokens go stale.
///
/// Chosen at 8: enough for an agent that re-snapshots once per turn over a
/// multi-window session (open a chat app, open a browser, swap to an editor,
/// …) before recycling; small enough that memory pressure is irrelevant.
pub const LRU_CAP_PER_PID: usize = 8;

/// Sentinel string returned (via [`TokenError::Stale`]'s `Display` impl) when
/// the token parses but the snapshot it references has been invalidated.
/// Consumers MUST surface this as a re-snapshot-and-retry signal, not a silent
/// misclick.
pub const STALE_TOKEN_ERROR: &str =
    "element_token is stale; call get_window_state again to refresh";

/// One valid snapshot retained in the per-pid LRU.
#[derive(Debug, Clone, Copy)]
struct SnapshotEntry {
    /// Monotonic, process-global id assigned by [`mint_snapshot_id`] (masked
    /// to 16 bits by [`TokenRegistry::register_snapshot`] before storage).
    snapshot_id: u32,
    /// The window the snapshot was taken against. Resolution returns this so
    /// tools can verify the caller's `window_id` arg matches — a token-only
    /// call doesn't have to pass `window_id` at all.
    window_id: u32,
    /// Maximum `element_index` that was assigned in this snapshot. The
    /// resolver rejects out-of-range tokens up-front instead of waiting for
    /// the per-platform cache to fail.
    max_element_index: usize,
}

/// Error returned by [`TokenRegistry::resolve`] when a token cannot be
/// honoured. Implements [`std::error::Error`] so callers can propagate it with
/// `?` against any `std::error::Error`-bound context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    /// The token string doesn't match the `s{hex}:{idx}` shape produced by
    /// [`format_token`].
    InvalidFormat,
    /// The token parsed, but the snapshot id is no longer in the pid's LRU
    /// (either evicted or never registered). Resolves to [`STALE_TOKEN_ERROR`]
    /// when displayed.
    Stale,
    /// The token's `element_index` is past the max recorded for the snapshot.
    /// Carries the offending index and the element count the snapshot
    /// actually recorded.
    OutOfRange {
        /// The out-of-range index the token carried.
        index: usize,
        /// Number of actionable elements the snapshot recorded.
        element_count: usize,
    },
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenError::InvalidFormat => {
                write!(f, "element_token has invalid format")
            }
            TokenError::Stale => write!(f, "{STALE_TOKEN_ERROR}"),
            TokenError::OutOfRange {
                index,
                element_count,
            } => {
                write!(
                    f,
                    "element_token element_index {index} out of range (snapshot had {element_count} elements)"
                )
            }
        }
    }
}

impl std::error::Error for TokenError {}

/// Process-global token registry. Thread-safe; tools resolve from any task via
/// the shared [`global`] accessor.
///
/// The data model is a `HashMap<pid, Vec<SnapshotEntry>>` where each pid's vec
/// is the LRU (newest at the back). `Vec` instead of `VecDeque` because the cap
/// is tiny (8) and walks are linear either way.
pub struct TokenRegistry {
    by_pid: Mutex<HashMap<i32, Vec<SnapshotEntry>>>,
}

impl TokenRegistry {
    /// Create an empty registry. Most callers should use [`global`] instead of
    /// constructing their own.
    pub fn new() -> Self {
        Self {
            by_pid: Mutex::new(HashMap::new()),
        }
    }

    /// Record a fresh snapshot for `pid` / `window_id`. Returns the minted
    /// snapshot id so the caller can embed it in the per-element token strings
    /// emitted alongside `element_index` in the structured elements array.
    ///
    /// `element_count` is the number of actionable elements in the snapshot
    /// (the count of nodes that received an `element_index`). Used for
    /// up-front range checks on [`resolve`][Self::resolve].
    ///
    /// Side effect: if this pid already has [`LRU_CAP_PER_PID`] snapshots in
    /// its lane, the oldest is evicted and any token that referenced it
    /// becomes stale — that's the contract.
    pub fn register_snapshot(&self, pid: i32, window_id: u32, element_count: usize) -> u32 {
        // Truncate to the 16-bit space the token format actually surfaces. The
        // full u32 still increments monotonically — we just don't widen the
        // on-the-wire token namespace beyond what the 4-hex-char prefix can
        // carry. Round-trip property: `resolve(format_token(id, idx))` always
        // finds the entry.
        let id = mint_snapshot_id() & 0xffff;
        let mut by_pid = self.by_pid.lock().unwrap();
        let lane = by_pid.entry(pid).or_default();
        lane.push(SnapshotEntry {
            snapshot_id: id,
            window_id,
            max_element_index: element_count.saturating_sub(1),
        });
        // Evict oldest. The loop guards against pre-existing over-cap state
        // from a previous version of the binary; in steady state this fires
        // exactly once per call.
        while lane.len() > LRU_CAP_PER_PID {
            lane.remove(0);
        }
        id
    }

    /// Resolve `token` against the LRU for `pid`. On success returns
    /// `(window_id, element_index)` — the same pair the caller would have
    /// passed as `(window_id, element_index)` integers. On failure returns one
    /// of:
    ///
    /// - [`TokenError::InvalidFormat`] — couldn't parse the `s{hex}:{idx}`
    ///   shape.
    /// - [`TokenError::Stale`] — parsed, but the snapshot id is no longer in
    ///   the pid's LRU (either evicted or never registered).
    /// - [`TokenError::OutOfRange`] — the index in the token is past the max
    ///   recorded for the snapshot.
    pub fn resolve(&self, pid: i32, token: &str) -> Result<(u32, usize), TokenError> {
        let (sid, idx) = parse_token(token).ok_or(TokenError::InvalidFormat)?;
        let by_pid = self.by_pid.lock().unwrap();
        let lane = by_pid.get(&pid).ok_or(TokenError::Stale)?;
        let entry = lane
            .iter()
            .find(|e| e.snapshot_id == sid)
            .ok_or(TokenError::Stale)?;
        if idx > entry.max_element_index {
            return Err(TokenError::OutOfRange {
                index: idx,
                element_count: entry.max_element_index + 1,
            });
        }
        Ok((entry.window_id, idx))
    }

    /// Build the canonical token string for `snapshot_id` / `element_index`.
    /// Pure helper, mirrors the [`format_token`] free function but lives on
    /// the registry so callers don't have to import it.
    pub fn format(snapshot_id: u32, element_index: usize) -> String {
        format_token(snapshot_id, element_index)
    }

    /// Test-only: snapshot count for a pid. Used by the LRU-eviction unit test
    /// to assert the cap was honoured.
    #[cfg(test)]
    fn snapshot_count(&self, pid: i32) -> usize {
        self.by_pid
            .lock()
            .unwrap()
            .get(&pid)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// Test-only: clear all state. Lets parallel unit tests start clean
    /// without relying on the global counter being at a specific value.
    #[cfg(test)]
    fn clear(&self) {
        self.by_pid.lock().unwrap().clear();
    }
}

impl Default for TokenRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-global counter for snapshot ids. Monotonically increasing — even
/// after eviction we never reuse an id during the process lifetime (`u32`
/// wraps after 4 billion calls, well past any realistic agent run).
static SNAPSHOT_COUNTER: AtomicU32 = AtomicU32::new(1);

/// Mint a fresh snapshot id. `1`-based so `"s0000:..."` is never a legitimate
/// token — makes "uninitialised default" bugs in client code pop on the first
/// call instead of accidentally aliasing a real snapshot.
///
/// Note: [`TokenRegistry::register_snapshot`] masks the returned value to 16
/// bits before storage, matching the 4-hex-char prefix the token format
/// carries. Callers that want a token string should pass the id returned by
/// `register_snapshot` (already masked) through [`format_token`].
pub fn mint_snapshot_id() -> u32 {
    // `Relaxed` is fine: the only invariant we need is uniqueness of the
    // returned value, which `fetch_add` provides on its own. No happens-before
    // edge with the Mutex below — the lock provides that.
    SNAPSHOT_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Format `(snapshot_id, element_index)` as the canonical token string. A
/// 4-hex-char snapshot prefix means tokens stay under 12 chars even with
/// 4-digit indices.
///
/// Snapshot ids are masked to 16 bits by
/// [`TokenRegistry::register_snapshot`] before storage so the round trip
/// `resolve(format_token(id, idx))` closes cleanly without truncation drift.
/// Collision chance inside the 8-entry LRU window is 8/65536 ≈ 0.01%; the
/// registry treats the `(pid, snapshot_id)` pair as the lookup key so a
/// same-bits collision across pids never aliases.
pub fn format_token(snapshot_id: u32, element_index: usize) -> String {
    let short = snapshot_id & 0xffff;
    format!("s{short:04x}:{element_index}")
}

/// Parse a canonical token string into `(snapshot_id, element_index)`. Returns
/// `None` on any shape error (unknown prefix, missing colon, non-hex,
/// non-decimal). The token strings are produced by [`format_token`] only —
/// consumers MUST treat the format as opaque and never construct one by hand.
fn parse_token(token: &str) -> Option<(u32, usize)> {
    let body = token.strip_prefix('s')?;
    let (hex, idx) = body.split_once(':')?;
    if hex.len() != 4 {
        return None;
    }
    let sid = u32::from_str_radix(hex, 16).ok()?;
    let idx = idx.parse::<usize>().ok()?;
    Some((sid, idx))
}

/// Process-global handle to the token registry. Used by a platform's
/// window-state implementation (to register a fresh snapshot) and every
/// element-targeting tool (to resolve a passed-in token).
pub fn global() -> &'static TokenRegistry {
    static REG: OnceLock<TokenRegistry> = OnceLock::new();
    REG.get_or_init(TokenRegistry::new)
}

/// Build a `s{hex}:{idx}` token from `snapshot_id` and `element_index`.
/// Convenience for the per-platform `build_elements_array` paths that already
/// iterate over actionable nodes and want a token per row.
///
/// `snapshot_id` is the value returned by
/// [`TokenRegistry::register_snapshot`] for the current window-state call. Pass
/// the same id for every element in one snapshot — the registry tracks them as
/// a group keyed by that id.
pub fn token_for(snapshot_id: u32, element_index: usize) -> String {
    format_token(snapshot_id, element_index)
}

/// Result of dispatching the `element_token` ↔ `element_index` precedence rule
/// on a tool call's args. Returned by [`resolve_element_args`].
#[derive(Debug, Clone)]
pub enum ResolvedElement {
    /// Neither `element_token` nor `element_index` was supplied — the tool
    /// should fall through to its non-element addressing mode (typically pixel
    /// `x, y`) or error.
    None,
    /// Resolved to `(window_id, element_index)`. The `window_id` may be `None`
    /// when the caller supplied only `element_index` without a `window_id`
    /// (legacy back-compat for tools that already handled that case); when the
    /// caller supplied a token, `window_id` is always the one the snapshot was
    /// taken against.
    Element {
        window_id: Option<u32>,
        element_index: usize,
        /// `true` when the caller supplied a token and we resolved through the
        /// registry — informational, used by tools that want to report "via
        /// token" in the success summary.
        via_token: bool,
    },
}

/// Apply the precedence rule for tool args that accept both `element_index` and
/// `element_token`. Returns either a stale/format error or the resolved
/// `(window_id, element_index)` pair wrapped in [`ResolvedElement`].
///
/// Rule:
/// - **Neither**: returns [`ResolvedElement::None`]. The tool decides whether
///   to error or fall through to a pixel path.
/// - **Only `element_index`**: legacy behaviour, unchanged. Returns
///   `Element { window_id: <caller's window_id arg, if any>, element_index, via_token: false }`.
/// - **Only `element_token`**: resolves through the registry. On stale or
///   malformed token, returns an error. On success returns
///   `Element { window_id: Some(<from snapshot>), element_index, via_token: true }`.
/// - **Both supplied**: `element_token` takes precedence; the resolver's index
///   wins and the integer is treated as advisory. On stale or malformed token,
///   returns an error — the integer is NOT used as a fallback (token wins, and
///   a stale token never silently falls back to the integer, which would
///   misclick).
///
/// `args_window_id` is the `window_id` arg the caller already pulled off the
/// tool's arguments. Passing it in here lets the helper keep that lookup in
/// one place per tool rather than duplicating it.
pub fn resolve_element_args(
    pid: i32,
    args_element_index: Option<usize>,
    args_element_token: Option<&str>,
    args_window_id: Option<u32>,
) -> Result<ResolvedElement, TokenError> {
    match (args_element_index, args_element_token) {
        (None, None) => Ok(ResolvedElement::None),
        (Some(idx), None) => Ok(ResolvedElement::Element {
            window_id: args_window_id,
            element_index: idx,
            via_token: false,
        }),
        (_idx_opt, Some(tok)) => {
            // Token wins. Resolve through the registry; bail on stale or
            // malformed without falling back to the integer. The integer arg
            // (when present) is advisory only — we deliberately do not act on
            // a disagreement here; the token's resolved index is authoritative.
            let (wid, idx) = global().resolve(pid, tok)?;
            Ok(ResolvedElement::Element {
                window_id: Some(wid),
                element_index: idx,
                via_token: true,
            })
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_registry() -> TokenRegistry {
        TokenRegistry::new()
    }

    #[test]
    fn token_round_trips_through_format_then_parse() {
        // Use a low-bit id that survives the 16-bit truncation in format_token,
        // so we can compare format → parse without losing information.
        let token = format_token(0x1234, 42);
        assert_eq!(token, "s1234:42");
        let (sid, idx) = parse_token(&token).expect("parse_token should accept its own output");
        assert_eq!(sid, 0x1234);
        assert_eq!(idx, 42);
    }

    #[test]
    fn token_format_pads_to_four_hex_chars() {
        // Small ids must still have a 4-char prefix so the parser's length
        // check passes.
        let token = format_token(1, 0);
        assert_eq!(token, "s0001:0");
        let token2 = format_token(0, 999);
        assert_eq!(token2, "s0000:999");
    }

    #[test]
    fn parse_rejects_unknown_prefix_or_shape() {
        assert!(parse_token("").is_none());
        assert!(parse_token("x1234:42").is_none(), "wrong prefix");
        assert!(parse_token("s1234").is_none(), "missing colon");
        assert!(parse_token("s12345:42").is_none(), "hex too long");
        assert!(parse_token("s123:42").is_none(), "hex too short");
        assert!(parse_token("szzzz:42").is_none(), "non-hex");
        assert!(parse_token("s1234:abc").is_none(), "non-decimal index");
    }

    #[test]
    fn register_then_resolve_returns_window_and_index() {
        let reg = fresh_registry();
        let pid = 100;
        let snapshot_id = reg.register_snapshot(pid, 42, /* element_count */ 5);
        let token = format_token(snapshot_id, 3);
        let (wid, idx) = reg.resolve(pid, &token).expect("fresh token must resolve");
        assert_eq!(wid, 42);
        assert_eq!(idx, 3);
    }

    #[test]
    fn resolve_with_unknown_pid_returns_stale_error() {
        // `STALE_TOKEN_ERROR` is the contract string consumers grep for, and
        // the Stale variant's Display must reproduce it exactly.
        let reg = fresh_registry();
        let token = format_token(0x1234, 0);
        let err = reg.resolve(/* pid = */ 999, &token).unwrap_err();
        assert_eq!(err, TokenError::Stale);
        assert_eq!(err.to_string(), STALE_TOKEN_ERROR);
    }

    #[test]
    fn resolve_with_bad_format_returns_invalid_error() {
        let reg = fresh_registry();
        // Pre-register a snapshot so we know the failure isn't from an empty
        // registry — the format check must run before the lane lookup so
        // callers get the more useful error.
        reg.register_snapshot(10, 1, 1);
        let err = reg.resolve(10, "garbage").unwrap_err();
        assert_eq!(err, TokenError::InvalidFormat);
        assert!(err.to_string().contains("invalid format"), "got: {err}");
    }

    #[test]
    fn out_of_range_index_returns_actionable_error() {
        let reg = fresh_registry();
        let pid = 11;
        let snapshot_id = reg.register_snapshot(pid, 1, /* element_count */ 3);
        // Snapshot has indices 0..=2 — 7 is past the end.
        let token = format_token(snapshot_id, 7);
        let err = reg.resolve(pid, &token).unwrap_err();
        match err {
            TokenError::OutOfRange {
                index,
                element_count,
            } => {
                assert_eq!(index, 7);
                assert_eq!(element_count, 3);
            }
            other => panic!("expected OutOfRange, got {other:?}"),
        }
        assert!(err.to_string().contains("out of range"), "got: {err}");
    }

    #[test]
    fn next_snapshot_for_same_pid_keeps_old_until_lru_evicts() {
        // The contract is "previous snapshot is invalidated when a NEW snapshot
        // runs for the pid" — but we hold an LRU of size LRU_CAP_PER_PID, so
        // callers get a small grace window of recent snapshots, not strictly
        // the most recent one.
        let reg = fresh_registry();
        let pid = 12;
        let s1 = reg.register_snapshot(pid, 1, 5);
        let s2 = reg.register_snapshot(pid, 1, 5);
        // Both should still resolve.
        let _ = reg
            .resolve(pid, &format_token(s1, 0))
            .expect("s1 still in LRU");
        let _ = reg.resolve(pid, &format_token(s2, 0)).expect("s2 fresh");
    }

    #[test]
    fn lru_eviction_invalidates_oldest_snapshot() {
        let reg = fresh_registry();
        let pid = 13;
        // Fill the LRU.
        let oldest = reg.register_snapshot(pid, 1, 5);
        for _ in 0..LRU_CAP_PER_PID {
            // Push LRU_CAP_PER_PID more, which evicts `oldest`.
            let _ = reg.register_snapshot(pid, 1, 5);
        }
        // Lane size must respect the cap.
        assert_eq!(reg.snapshot_count(pid), LRU_CAP_PER_PID);
        // Oldest must be stale now.
        let err = reg.resolve(pid, &format_token(oldest, 0)).unwrap_err();
        assert_eq!(err, TokenError::Stale);
    }

    #[test]
    fn tokens_in_different_pids_dont_collide() {
        // Same snapshot counter values across pids must resolve back to each
        // pid's own window_id, never the other's. This is the per-pid lane
        // property the registry promises.
        let reg = fresh_registry();
        let s_a = reg.register_snapshot(/* pid = */ 100, /* window_id = */ 11, 3);
        let s_b = reg.register_snapshot(/* pid = */ 200, /* window_id = */ 22, 3);
        let token_a = format_token(s_a, 0);
        let token_b = format_token(s_b, 0);
        // Cross-pid attempts must NOT resolve to the other pid's window.
        assert_eq!(reg.resolve(100, &token_a).unwrap().0, 11);
        assert_eq!(reg.resolve(200, &token_b).unwrap().0, 22);
        // Attempting to use pid A's token under pid B must fail stale.
        let err = reg.resolve(200, &token_a).unwrap_err();
        assert_eq!(err, TokenError::Stale);
    }

    #[test]
    fn global_registry_is_shared_across_calls() {
        // Smoke test that `global()` returns the same instance every call.
        let reg_a = global();
        let reg_b = global();
        assert!(std::ptr::eq(reg_a, reg_b));
    }

    #[test]
    fn stale_token_returns_explicit_error_not_silent_misclick() {
        // Hard constraint: we must NEVER silently re-map a stale token to "some
        // index" — the consumer has to see the error and re-snapshot.
        let reg = fresh_registry();
        let pid = 14;
        let s1 = reg.register_snapshot(pid, 1, 5);
        // Evict by pushing LRU_CAP_PER_PID newer snapshots.
        for _ in 0..LRU_CAP_PER_PID {
            let _ = reg.register_snapshot(pid, 1, 5);
        }
        let err = reg.resolve(pid, &format_token(s1, 2)).unwrap_err();
        assert_eq!(err, TokenError::Stale);
        assert_eq!(err.to_string(), STALE_TOKEN_ERROR);
    }

    #[test]
    fn clear_then_register_starts_clean() {
        let reg = fresh_registry();
        let _ = reg.register_snapshot(1, 1, 1);
        reg.clear();
        assert_eq!(reg.snapshot_count(1), 0);
    }

    #[test]
    fn mint_snapshot_id_is_monotonic_and_one_based() {
        // 1-based so "s0000:..." is never legitimate; strictly increasing.
        let a = mint_snapshot_id();
        let b = mint_snapshot_id();
        assert!(a >= 1, "ids are 1-based, got {a}");
        assert!(
            b > a,
            "ids must be monotonically increasing, got a={a} b={b}"
        );
    }

    // ── resolve_element_args precedence rule ─────────────────────────
    //
    // These cover the dispatch contract:
    //
    // - element_index_alone_still_works
    // - element_token_alone_resolves_to_same_action
    // - both_provided_token_wins_on_disagreement
    //
    // The "stale" and "different pids" surfaces are already covered by the
    // registry-level tests above; resolve_element_args is just the thin
    // precedence layer on top.

    #[test]
    fn element_index_alone_still_works() {
        // Backward-compat regression guard: tools that only see element_index
        // keep returning the same shape.
        let resolved = resolve_element_args(
            /* pid = */ 1,
            /* element_index = */ Some(7),
            /* element_token = */ None,
            /* window_id = */ Some(99),
        )
        .expect("element_index-only must succeed");
        match resolved {
            ResolvedElement::Element {
                window_id,
                element_index,
                via_token,
            } => {
                assert_eq!(window_id, Some(99));
                assert_eq!(element_index, 7);
                assert!(
                    !via_token,
                    "element_index-only path must NOT report via_token"
                );
            }
            _ => panic!("expected Element, got {resolved:?}"),
        }
    }

    #[test]
    fn element_token_alone_resolves_to_same_action() {
        // Register a snapshot in the GLOBAL registry (resolve_element_args uses
        // `global()`), then resolve the token through the same path the tool
        // would use.
        let reg = global();
        // Use a pid unlikely to collide with other tests.
        let pid = 0x7fff_0001_i32;
        let snapshot_id = reg.register_snapshot(pid, /* window_id = */ 555, 4);
        let token = format_token(snapshot_id, 2);
        let resolved = resolve_element_args(
            pid,
            None,
            Some(&token),
            // window_id arg intentionally omitted — the token carries it.
            None,
        )
        .expect("token-only must succeed");
        match resolved {
            ResolvedElement::Element {
                window_id,
                element_index,
                via_token,
            } => {
                assert_eq!(window_id, Some(555), "window_id comes from the snapshot");
                assert_eq!(element_index, 2);
                assert!(via_token, "token path must report via_token=true");
            }
            _ => panic!("expected Element, got {resolved:?}"),
        }
    }

    #[test]
    fn both_provided_token_wins_on_disagreement() {
        // Both args supplied with disagreeing indices — token wins, no error
        // returned. The returned indices come from the token, not the integer.
        let reg = global();
        let pid = 0x7fff_0002_i32;
        let snapshot_id = reg.register_snapshot(pid, 777, 5);
        let token = format_token(snapshot_id, 3);
        let resolved = resolve_element_args(
            pid,
            Some(99), // disagrees with token (which says idx 3)
            Some(&token),
            None,
        )
        .expect("disagreement still resolves; token wins");
        match resolved {
            ResolvedElement::Element {
                window_id,
                element_index,
                via_token,
            } => {
                assert_eq!(window_id, Some(777));
                assert_eq!(element_index, 3, "token's idx wins over the integer arg");
                assert!(via_token);
            }
            _ => panic!("expected Element, got {resolved:?}"),
        }
    }

    #[test]
    fn token_only_stale_returns_error_not_silent_fallback_to_integer() {
        // Hard constraint: a stale token MUST NOT fall back to the integer —
        // that would silently misclick.
        let pid = 0x7fff_0003_i32;
        // Token references a snapshot that was never registered → stale.
        let token = format_token(0xdead, 0);
        let err = resolve_element_args(pid, Some(0), Some(&token), Some(1)).unwrap_err();
        // Stale token surfaces as the Stale variant; the integer is NOT used.
        assert_eq!(err, TokenError::Stale);
        assert_eq!(err.to_string(), STALE_TOKEN_ERROR);
    }

    #[test]
    fn malformed_token_returns_invalid_format_not_fallback_to_integer() {
        // A token that doesn't parse must surface InvalidFormat, not silently
        // fall back to the integer arg.
        let pid = 0x7fff_0004_i32;
        let err = resolve_element_args(pid, Some(5), Some("not-a-token"), Some(1)).unwrap_err();
        assert_eq!(err, TokenError::InvalidFormat);
    }

    #[test]
    fn neither_returns_none() {
        let resolved =
            resolve_element_args(1, None, None, None).expect("neither arg returns None, not error");
        assert!(matches!(resolved, ResolvedElement::None));
    }
}
