pub(crate) fn render_plugin_source_summary(
    discovered_count: usize,
    approved_count: usize,
    warning_count: usize,
    error_count: usize,
) -> String {
    if error_count > 0 {
        format!(
            "[error] Plugin packages: {discovered_count} discovered, {approved_count} source-approved, warnings={warning_count}, errors={error_count}; execution unavailable"
        )
    } else if warning_count > 0 {
        format!(
            "[warn] Plugin packages: {discovered_count} discovered, {approved_count} source-approved, warnings={warning_count}; execution unavailable"
        )
    } else {
        format!(
            "[ok] Plugin packages: {discovered_count} discovered, {approved_count} source-approved; execution unavailable"
        )
    }
}

pub(crate) const fn plugin_source_check_passes(error_count: usize) -> bool {
    error_count == 0
}

pub(crate) fn render_source_review_epoch(trust_epoch: Option<u64>) -> String {
    match trust_epoch {
        Some(epoch) => format!("Source review epoch: {epoch}"),
        None => "Source review epoch: unavailable".to_string(),
    }
}

pub(crate) fn render_mcp_configuration_count(configured_count: usize) -> String {
    format!("MCP configuration entries: {configured_count}")
}

pub(crate) fn escape_terminal_text(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| {
            if ch.is_control() || is_bidi_format_character(ch) {
                ch.escape_default().collect::<Vec<_>>()
            } else {
                vec![ch]
            }
        })
        .collect()
}

fn is_bidi_format_character(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use super::{
        escape_terminal_text, plugin_source_check_passes, render_mcp_configuration_count,
        render_plugin_source_summary, render_source_review_epoch,
    };

    #[test]
    fn plugin_source_summary_distinguishes_clean_and_diagnostic_states() {
        assert_eq!(
            render_plugin_source_summary(0, 0, 0, 0),
            "[ok] Plugin packages: 0 discovered, 0 source-approved; execution unavailable"
        );
        assert_eq!(
            render_plugin_source_summary(2, 1, 3, 0),
            "[warn] Plugin packages: 2 discovered, 1 source-approved, warnings=3; execution unavailable"
        );
        assert_eq!(
            render_plugin_source_summary(2, 1, 1, 2),
            "[error] Plugin packages: 2 discovered, 1 source-approved, warnings=1, errors=2; execution unavailable"
        );
        assert!(plugin_source_check_passes(0));
        assert!(!plugin_source_check_passes(1));
    }

    #[test]
    fn diagnostics_do_not_claim_unavailable_trust_or_runtime_state() {
        assert_eq!(
            render_source_review_epoch(Some(7)),
            "Source review epoch: 7"
        );
        assert_eq!(
            render_source_review_epoch(None),
            "Source review epoch: unavailable"
        );
        assert_eq!(
            render_mcp_configuration_count(3),
            "MCP configuration entries: 3"
        );
    }

    #[test]
    fn plugin_diagnostics_escape_terminal_control_characters() {
        assert_eq!(
            escape_terminal_text("safe\n\u{1b}]8;;forged\u{202e}status"),
            "safe\\n\\u{1b}]8;;forged\\u{202e}status"
        );
    }
}
