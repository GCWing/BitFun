# Codex Adapter Instructions

- This crate may read and normalize Codex configuration but must never invoke Hook handlers, start Codex/app-server, or own trust, enablement, or product policy.
- Keep commands, prompts, arguments, environment values, and credentials inside the adapter. Public contracts expose only bounded summaries.
- Preserve native Codex event and handler names. Map only reviewed BitFun Hook points; report every other event as native-only.
- Do not depend on another ecosystem adapter. Shared product behavior belongs in contracts or assembly.
