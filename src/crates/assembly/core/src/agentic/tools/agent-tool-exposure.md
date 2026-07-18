## Current Tool Default Exposure States and Agent Overrides

Notes:
- "Default state" comes from `Tool::default_exposure()`. Tools that do not implement this method default to `Direct`.
- "Overriding agents" only lists built-in agents that explicitly define `tool_exposure_overrides()` in the current code.
- Custom subagents do not currently support independent exposure overrides and inherit the default behavior.
- Dynamically registered MCP tools default to `Deferred`; they are discovered at runtime and are not enumerated in the built-in table below.
- Global `ai.enable_deferred_tool_loading=false` overrides all allowed tools to `Direct` and removes `GetToolSpec` and `CallDeferredTool` from the model-visible manifest.

**Tool Exposure Table**

| Tool | Default State | Overridden By | Override State |
|---|---|---|---|
| `LS` | Direct | None | - |
| `Read` | Direct | None | - |
| `Glob` | Direct | None | - |
| `Grep` | Direct | None | - |
| `Write` | Direct | None | - |
| `Edit` | Direct | None | - |
| `Delete` | Direct | None | - |
| `ExecCommand` | Direct | None | - |
| `WriteStdin` | Direct | None | - |
| `ExecControl` | Direct | None | - |
| `GetTime` | Direct | None | - |
| `Task` | Direct | None | - |
| `Skill` | Direct | None | - |
| `AskUserQuestion` | Direct | None | - |
| `TodoWrite` | Direct | None | - |
| `CodeReview` | Direct | None | - |
| `GetToolSpec` | Direct | None | - |
| `CallDeferredTool` | Direct | None | - |
| `CreatePlan` | Deferred | None | - |
| `GetFileDiff` | Deferred | `ReviewFixer`, `ReviewBusinessLogic`, `ReviewPerformance`, `ReviewSecurity`, `ReviewArchitecture`, `ReviewFrontend`, `ReviewJudge` | Direct |
| `SessionControl` | Deferred | None | - |
| `SessionMessage` | Deferred | None | - |
| `SessionHistory` | Deferred | None | - |
| `Cron` | Deferred | None | - |
| `WebSearch` | Deferred | `DeepResearch` | Direct |
| `WebFetch` | Deferred | `DeepResearch` | Direct |
| `ListMCPResources` | Deferred | None | - |
| `ReadMCPResource` | Deferred | None | - |
| `ListMCPPrompts` | Deferred | None | - |
| `GetMCPPrompt` | Deferred | None | - |
| `GenerativeUI` | Deferred | None | - |
| `Git` | Deferred | `ReviewFixer`, `ReviewBusinessLogic`, `ReviewPerformance`, `ReviewSecurity`, `ReviewArchitecture`, `ReviewFrontend`, `ReviewJudge` | Direct |
| `InitMiniApp` | Deferred | None | - |
| `ControlHub` | Deferred | `ComputerUse` | Direct |
| `ComputerUse` | Deferred | `ComputerUse` | Direct |
| `Playbook` | Deferred | None | - |

**Agents With Override Policies**

| agent id | Overridden Tools |
|---|---|
| `DeepResearch` | `WebSearch`, `WebFetch` |
| `ComputerUse` | `ControlHub`, `ComputerUse` |
| `ReviewFixer` | `GetFileDiff`, `Git` |
| `ReviewBusinessLogic` | `GetFileDiff`, `Git` |
| `ReviewPerformance` | `GetFileDiff`, `Git` |
| `ReviewSecurity` | `GetFileDiff`, `Git` |
| `ReviewArchitecture` | `GetFileDiff`, `Git` |
| `ReviewFrontend` | `GetFileDiff`, `Git` |
| `ReviewJudge` | `GetFileDiff`, `Git` |
