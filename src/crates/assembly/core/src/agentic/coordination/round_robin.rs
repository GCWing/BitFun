//! Round-robin member selector for group chat mode B (轮转调度).
//!
//! Pure index arithmetic: the next member is `members[cursor % len]` and the
//! cursor advances by one after each pick. The caller owns the cursor value;
//! the selector never mutates caller state.
//!
//! Contract: dispatch-prompts v1.3 R-GC-09 (no dependencies).

/// Selects the next member by round-robin from `members[cursor % len]`.
///
/// Returns `None` when the list is empty (never panics, 铁则 6 防呆).
/// A single-element list always selects that element.
pub fn next(members: &[String], cursor: usize) -> Option<String> {
    if members.is_empty() {
        return None;
    }
    Some(members[cursor % members.len()].clone())
}

#[cfg(test)]
mod tests {
    use super::next;

    fn members(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn round_robin_cycles_through_members_in_order() {
        let members = members(&["A", "B", "C"]);

        assert_eq!(next(&members, 0).as_deref(), Some("A"));
        assert_eq!(next(&members, 1).as_deref(), Some("B"));
        assert_eq!(next(&members, 2).as_deref(), Some("C"));
        // 多轮循环：cursor 3 回到 A
        assert_eq!(next(&members, 3).as_deref(), Some("A"));
        assert_eq!(next(&members, 4).as_deref(), Some("B"));
        assert_eq!(next(&members, 5).as_deref(), Some("C"));
    }

    #[test]
    fn round_robin_empty_list_returns_none_without_panicking() {
        let members: Vec<String> = Vec::new();
        assert_eq!(next(&members, 0), None);
        assert_eq!(next(&members, 7), None);
        assert_eq!(next(&members, 100), None);
    }

    #[test]
    fn round_robin_single_element_is_always_selected() {
        let members = members(&["Solo"]);
        assert_eq!(next(&members, 0).as_deref(), Some("Solo"));
        assert_eq!(next(&members, 1).as_deref(), Some("Solo"));
        assert_eq!(next(&members, 42).as_deref(), Some("Solo"));
    }

    #[test]
    fn round_robin_cursor_advances_without_touching_caller_state() {
        let members = members(&["A", "B"]);
        let mut cursor = 0usize;

        let picked = next(&members, cursor).expect("member");
        cursor += 1;
        assert_eq!(picked, "A");
        assert_eq!(cursor, 1);

        let picked = next(&members, cursor).expect("member");
        cursor += 1;
        assert_eq!(picked, "B");
        assert_eq!(cursor, 2);

        let picked = next(&members, cursor).expect("member");
        cursor += 1;
        assert_eq!(picked, "A");
        assert_eq!(cursor, 3);
    }

    #[test]
    fn round_robin_large_cursor_wraps_via_modulo() {
        let members = members(&["X", "Y"]);
        assert_eq!(next(&members, usize::MAX).as_deref(), Some("Y"));
        assert_eq!(next(&members, usize::MAX - 1).as_deref(), Some("X"));
    }
}
