use crate::service::session::{DialogTurnData, ToolItemIdentityExt};

pub(crate) fn dialog_turn_uses_external_context(turn: &DialogTurnData) -> bool {
    turn.model_rounds.iter().any(|round| {
        round
            .tool_items
            .iter()
            .any(|item| is_external_context_tool_name(item.effective_name()))
    })
}

pub(crate) fn session_uses_external_context(turns: &[DialogTurnData]) -> bool {
    turns.iter().any(dialog_turn_uses_external_context)
}

pub(crate) fn is_external_context_tool_name(tool_name: &str) -> bool {
    let normalized = tool_name.trim().to_ascii_lowercase();
    is_direct_external_context_tool_name(&normalized)
        || is_mcp_external_context_tool_name(&normalized)
        || normalized.contains("__enterprise/")
}

fn is_direct_external_context_tool_name(normalized: &str) -> bool {
    normalized == "webfetch"
        || normalized == "web_fetch"
        || normalized == "web_search_exa"
        || normalized == "websearch"
        || normalized == "browser_search"
        || normalized == "browser_fetch"
        || normalized.starts_with("web_search")
        || normalized.starts_with("external_search")
}

fn is_mcp_external_context_tool_name(normalized: &str) -> bool {
    let Some(rest) = normalized.strip_prefix("mcp__") else {
        return false;
    };

    const KEYWORDS: &[&str] = &[
        "web",
        "search",
        "fetch",
        "browser",
        "browse",
        "url",
        "http",
        "internet",
        "online",
        "exa",
        "tavily",
        "perplexity",
    ];

    KEYWORDS.iter().any(|keyword| rest.contains(keyword))
}

#[cfg(test)]
mod tests {
    use super::is_external_context_tool_name;

    #[test]
    fn external_context_tool_detection_covers_direct_web_tools() {
        assert!(is_external_context_tool_name("WebFetch"));
        assert!(is_external_context_tool_name("web_fetch"));
        assert!(is_external_context_tool_name("WebSearch"));
        assert!(is_external_context_tool_name("web_search_exa"));
        assert!(is_external_context_tool_name("browser_search"));
        assert!(is_external_context_tool_name("browser_fetch"));
        assert!(is_external_context_tool_name("github__enterprise/prod"));

        assert!(!is_external_context_tool_name("Read"));
        assert!(!is_external_context_tool_name("Edit"));
        assert!(!is_external_context_tool_name("Task"));
    }

    #[test]
    fn mcp_external_context_detection_matches_web_like_tools_only() {
        assert!(is_external_context_tool_name("mcp__exa__search"));
        assert!(is_external_context_tool_name("mcp__tavily__web_search"));
        assert!(is_external_context_tool_name("mcp__browser__fetch_url"));
        assert!(is_external_context_tool_name("mcp__perplexity__ask"));
        assert!(is_external_context_tool_name("mcp__server__http_request"));

        assert!(!is_external_context_tool_name("mcp"));
        assert!(!is_external_context_tool_name("mcp_tool"));
        assert!(!is_external_context_tool_name("mcp__server__tool"));
        assert!(!is_external_context_tool_name("mcp__rust__cargo_check"));
        assert!(!is_external_context_tool_name("mcp__local__compile"));
        assert!(!is_external_context_tool_name("mcp__tester__run_tests"));
        assert!(!is_external_context_tool_name("mcp__filesystem__read"));
        assert!(!is_external_context_tool_name("mcp__workspace__grep"));
        assert!(!is_external_context_tool_name("mcp__validator__validate"));
    }
}
