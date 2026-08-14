# BitFun CLI Agent Guide

Scope: `src/apps/cli`.

Read the repository `AGENTS.md` first. For architecture-sensitive work, also
read:

- [`cli-product-line-design.md`](../../../docs/architecture/cli-product-line-design.md)
- [`product-architecture.md`](../../../docs/architecture/product-architecture.md)
- [`agent-runtime-deployment-design.md`](../../../docs/architecture/agent-runtime-deployment-design.md)
- [`product-customization-blueprint.md`](../../../docs/architecture/product-customization-blueprint.md) when changing product assembly, branding, or packaging

## Ownership

CLI owns only surface concerns:

- Clap entrypoints and CLI-local configuration
- terminal acquisition/restoration and input normalization
- TUI state, rendering, popups, local draft history, and local effects such as
  clipboard or external-editor integration
- projection of Runtime events into text, JSON, JSONL, and user diagnostics
- Direct/Shared Runtime adaptation and Peer Device host presentation

Session, turn, model round, tool execution, permissions, cancellation,
persistence, context, workspace binding, MCP, Subagent, and other product facts
belong to their shared owners. Do not add CLI-only managers or reproduce shared
behavior behind a TUI branch.

Existing Core compatibility forwarding may remain until a reviewed owner
migration has behavior-equivalence tests. A typed port is not evidence that the
runtime owner moved.

## Runtime paths

Normal interactive submissions follow:

```text
ChatView -> CliAgentRuntimeClient -> TuiRuntimePort
         -> Embedded(AgentRuntime) -> Agent Runtime owner -> ToolPipeline
         -> Shared(RuntimeIpcClient) -> private Runtime IPC v18
                                     -> Agent Runtime owner -> ToolPipeline
```

Phase 5 is delivered: the default interactive TUI calls the in-process
`AgentRuntime` directly, while explicit `--shared` uses the private versioned
Runtime IPC. `TuiRuntimePort` is private to `CliAgentRuntimeClient`; views and
controllers must not depend on Runtime, Core, or IPC deployment details. The
retired CLI path must not reintroduce `TuiBackend`, `AppServerTuiBackend`,
`SharedTuiBackend`, an embedded App Server, or an in-memory App Server transport.

Management operations use a separate composition path:

```text
TUI controller -> TuiManagementOwners
               -> concrete Model / Registry / MCP / Account / Settings Sync /
                  Worktree / Native Hook / External Hook / External Source /
                  External Command providers
               -> owning domain service
```

This composition is intentionally not a total `TuiManagementPort` and does not
route management through Runtime IPC or create CLI-local business owners or
authoritative management state.
Providers retain capability and Remote-workspace guards. Web/App Server remains
a sibling product adapter. Replacing the physical Shared transport belongs to a
separately reviewed phase. Side-effecting operations need stable identities,
controller/idle rules, bounded frames, and outcome-unknown handling before a
connection can retry.

Explicit Shell input follows:

```text
SHELL composer -> AgentUserShellCommandPort -> Core coordinator
               -> ToolPipeline(ExecCommand) -> TerminalPort / RemoteExecPort
               -> standard UserDialog + ModelRound persistence and events
```

CLI must never spawn the submitted command directly or expose a generic tool or
process API. Explicit user input may auto-approve an interactive `ask`, but
static `deny` rules, workspace routing, cancellation, audit, and tool
restrictions remain enforced.

## TUI rules

- Derive slash commands, palette actions, help, availability, and key bindings
  from the action registry. Do not add a second command table.
- Match established competitor entry flows when equivalent behavior exists.
  Prefer OpenCode names and interactions; do not invent `/shell` or aliases for
  the `!` Shell entry.
- Keep terminal input, state transitions, effects, and rendering independently
  testable. Views and reducers do not perform filesystem, network, config, or
  Agent operations.
- Shell mode is CLI presentation state only. It accepts an empty-composer `!`,
  keeps chat/shell histories separate, treats `/` as command text, and rejects
  images and structured `@` references before Runtime submission.
- Direct paste, `Ctrl+V`, and bracketed paste share `ComposerDraft`. Shared TUI
  rejects unsupported image payloads before IPC.
- Local effects such as `/editor`, copy, and export stay local. Product work
  such as shell execution, session mutation, and permissions goes through typed
  Runtime owners.
- Session-lineage membership, order, legacy relationship normalization,
  transcript reads, and targeted cancellation stay in shared Runtime owners.
  TUI may keep only the selector/read-only inspection state and must preserve
  the root composer while a descendant is visible.
- Always restore raw mode, alternate screen, mouse capture, paste mode, and the
  cursor on success, error, cancellation, initialization failure, or panic.
- Protocol stdout contains only the selected result format. Logs are English,
  contain no emoji, and use stderr or log files.

## Product and external-source boundaries

- Assemble CLI through `DeliveryProfile::Cli` and validated product Runtime
  parts. Hiding a command is not a backend capability restriction.
- The CLI selects the reviewed `bitfun-core` owner-feature closure
  (`agent-runtime`, `canvas-runtime`, `external-sources`, `plugin-runtime`, and
  `ssh-remote`). Do not replace it with `product-full` or a CLI-named umbrella;
  add a Core feature only when a production CLI path consumes that owner.
- CLI consumes typed external-source summaries and actions. It does not parse
  source files, import executable modules, start plugin workers, duplicate
  approval state, or treat static discovery as runtime availability.
- ACP agents, configuration import, executable plugins, Hooks, and Peer Device
  hosting have separate trust and lifecycle state. Do not infer one from
  another.
- Remote-unsupported local effects must fail visibly; never fall back to the
  controller machine.

Detailed compatibility rules belong in the dedicated architecture documents,
not in this file.

## Commands

```bash
pnpm run cli:dev
pnpm run cli:install
```

## Verification

Run the smallest checks matching the changed path:

```bash
cargo check -p bitfun-cli
cargo test -p bitfun-cli
```

When a CLI change crosses a shared boundary, use the focused command maintained
by that owner: Agent Runtime for port/SDK behavior, the IPC adapter for shared
protocol behavior, Core for turn/tool/persistence behavior, Terminal for
PTY/ConPTY lifecycle, and Product Assembly for packaging. Do not copy those
owners' commands into this guide.

Use [`README.md`](README.md) for user-facing behavior and installation. Keep
developer internals here or in architecture docs instead of expanding the user
guide.
