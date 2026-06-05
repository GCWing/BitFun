//! Shared MCP tool-name helpers.

pub const MCP_TOOL_PREFIX: &str = "mcp__";
pub const MCP_TOOL_DELIMITER: &str = "__";

/// Normalize MCP server/tool names to a wire-safe format aligned with claude-code.
pub fn normalize_name_for_mcp(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn build_mcp_tool_name(server_id: &str, tool_name: &str) -> String {
    format!(
        "{}{}{}{}",
        MCP_TOOL_PREFIX,
        normalize_name_for_mcp(server_id),
        MCP_TOOL_DELIMITER,
        normalize_name_for_mcp(tool_name)
    )
}

#[cfg(test)]
mod tests {
    use super::{build_mcp_tool_name, normalize_name_for_mcp};

    #[test]
    fn normalize_name_for_mcp_replaces_spaces_and_symbols() {
        assert_eq!(
            normalize_name_for_mcp("Acme Search / Primary"),
            "Acme_Search___Primary"
        );
    }

    #[test]
    fn normalize_name_for_mcp_keeps_ascii_word_chars_and_hyphen() {
        assert_eq!(
            normalize_name_for_mcp("github-enterprise_v2"),
            "github-enterprise_v2"
        );
    }

    #[test]
    fn build_mcp_tool_name_normalizes_both_segments() {
        assert_eq!(
            build_mcp_tool_name("Claude Code", "search repos"),
            "mcp__Claude_Code__search_repos"
        );
    }
}
