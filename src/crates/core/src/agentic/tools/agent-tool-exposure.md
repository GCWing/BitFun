## Current Tool Default Exposure / Collapse States and Agent Overrides

Notes:
- "Default state" comes from `Tool::default_exposure()`. Tools that do not implement this method default to `Expanded`.
- "Overriding agents" only lists built-in agents that explicitly define `tool_exposure_overrides()` in the current code.
- Custom subagents do not currently support independent exposure overrides and inherit the default behavior.

**Tool Exposure Table**

| Tool | Default State | Overridden By | Override State |
|---|---|---|---|
| `LS` | Expanded | None | - |
| `Read` | Expanded | None | - |
| `Glob` | Expanded | None | - |
| `Grep` | Expanded | None | - |
| `Write` | Expanded | None | - |
| `Edit` | Expanded | None | - |
| `Delete` | Expanded | None | - |
| `Bash` | Expanded | None | - |
| `Task` | Expanded | None | - |
| `Skill` | Expanded | None | - |
| `AskUserQuestion` | Expanded | None | - |
| `TodoWrite` | Expanded | None | - |
| `CreatePlan` | Expanded | None | - |
| `CodeReview` | Expanded | None | - |
| `GetToolSpec` | Expanded | None | - |
| `GetFileDiff` | Collapsed | `ReviewFixer`, `ReviewBusinessLogic`, `ReviewPerformance`, `ReviewSecurity`, `ReviewArchitecture`, `ReviewFrontend`, `ReviewJudge` | Expanded |
| `Log` | Collapsed | None | - |
| `TerminalControl` | Collapsed | None | - |
| `SessionControl` | Collapsed | None | - |
| `SessionMessage` | Collapsed | None | - |
| `SessionHistory` | Collapsed | None | - |
| `Cron` | Collapsed | None | - |
| `WebSearch` | Collapsed | `DeepResearch` | Expanded |
| `WebFetch` | Collapsed | `DeepResearch` | Expanded |
| `ListMCPResources` | Collapsed | None | - |
| `ReadMCPResource` | Collapsed | None | - |
| `ListMCPPrompts` | Collapsed | None | - |
| `GetMCPPrompt` | Collapsed | None | - |
| `GenerativeUI` | Collapsed | None | - |
| `Git` | Collapsed | `ReviewFixer`, `ReviewBusinessLogic`, `ReviewPerformance`, `ReviewSecurity`, `ReviewArchitecture`, `ReviewFrontend`, `ReviewJudge` | Expanded |
| `InitMiniApp` | Collapsed | None | - |
| `ControlHub` | Collapsed | `ComputerUse` | Expanded |
| `ComputerUse` | Collapsed | `ComputerUse` | Expanded |
| `Playbook` | Collapsed | None | - |

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
