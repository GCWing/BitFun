# Codex Adapter Instructions

- This crate may read and normalize Codex Subagent, MCP, and Hook configuration but must never invoke Hook handlers, start Codex/app-server, connect MCP servers, or own trust, enablement, or product policy.
- Keep commands, prompts, arguments, environment values, and credentials inside the adapter. Public contracts expose only bounded summaries.
- Preserve native Codex event and handler names. Map only reviewed BitFun Hook points; report every other event as native-only.
- Preserve Codex configuration-layer and field-overlay semantics inside each typed provider. Unknown behavior fields must block explicitly; do not invent Codex Command or standalone Tool sources.
- Do not depend on another ecosystem adapter. Shared product behavior belongs in contracts or assembly.
