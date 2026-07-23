# Claude Code Adapter Instructions

- This crate may read and normalize Claude Code configuration but must never invoke Hook handlers, start Claude Code, connect MCP servers, or own product policy.
- Keep handler bodies, commands, prompts, URLs, arguments, environment values, and credentials inside the adapter. Public contracts expose only bounded summaries.
- Preserve native Claude Code event and handler names. Map only reviewed BitFun Hook points; report every other event as native-only.
- Do not depend on another ecosystem adapter. Shared product behavior belongs in contracts or assembly.
