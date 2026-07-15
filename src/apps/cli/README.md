# BitFun CLI

Terminal UI for BitFun (chat, tools, `/login` account + Peer Host).

The local Agent paths build the CLI product profile once per invocation. Interactive chat, `exec`,
session commands, and usage reports use that invocation-scoped runtime context and event source.
Local management queries do not start Peer Host or MCP; `exec` starts MCP but not Peer Host.
Core remains the compatibility owner for execution and persistence operations not yet covered by the
Agent Runtime SDK. When interactive mode enables Peer Host, Peer dialog submission, cancellation,
and agent-event fan-out reuse the same runtime context; Peer Host does not construct another
scheduler, persistence manager, or event queue. Plugin execution is not enabled by this assembly path.

## Common commands

```bash
bitfun-cli                                  # interactive TUI
bitfun-cli exec "summarize this project"   # non-interactive, rejects permission requests
bitfun-cli exec "run tests" --auto         # approve tool requests for this invocation
bitfun-cli sessions list
bitfun-cli usage
bitfun-cli doctor
bitfun-cli health
```

The TUI asks before protected tool calls and offers `Allow once`, `Allow always`, and `Reject`.
`Allow always` applies only to matching tools in the current runtime context; it does not update the
global configuration. Non-interactive `exec` rejects permission requests by default. Use `--auto`
only when the current invocation may approve tool requests. Non-interactive `exec` does not expose
`AskUserQuestion`; provide all required input in the initial prompt. The hidden legacy `--confirm`
flag maps to the safe default and should not be used in new automation.

### Structured output

| Format | stdout contract |
|---|---|
| `text` | Assistant text. Progress, tool status, logs, and diagnostics use stderr. |
| `json` | One final result object with status and result, plus session/turn identity once established, turn-accumulated usage, and available Patch facts. |
| `stream-json` | JSONL containing existing `AgenticEventEnvelope` values; no separate CLI event schema. |

Select a format with `--output-format text|json|stream-json`. When `--output-patch -` is used with
`json`, the Patch is included in the final object. For `stream-json`, write the Patch to an explicit
file path so protocol stdout remains valid JSONL. A Patch is the repository's `HEAD`-relative
workspace snapshot captured before an explicit Patch artifact is written. It includes staged,
unstaged, untracked, and pre-existing changes, excludes the output artifact itself, and does not
attribute changes to this invocation.

`Ctrl+C` requests cancellation of the active turn and briefly drains its terminal envelope before
returning. Cancellation, an unsuccessful completion event,
and a requested Patch that cannot be generated or written are error outcomes. An explicit Patch
file is created even when the diff is empty.

`doctor` and `health` validate product assembly and required capability registrations. They are not
live probes for Network, Git, or MCP integrations that are currently represented by compatibility
registrations.

## One-click install (Linux / macOS, amd64 + arm64)

From the repository root:

```bash
bash src/apps/cli/install.sh
```

Or from this directory:

```bash
bash install.sh
```

The script will:

1. `cargo build -p bitfun-cli --release` (native host CPU)
2. Install `bitfun-cli` to `~/.local/bin` (override with `BITFUN_CLI_BIN_DIR`)
3. Idempotently add a PATH block to `~/.bashrc` and `~/.zshrc`
4. `source` the matching rc when the current shell is interactive bash/zsh

Then run:

```bash
bitfun-cli
```

### Options / environment

| Variable | Meaning |
|----------|---------|
| `BITFUN_CLI_BIN_DIR` | Install directory (default `~/.local/bin`) |
| `BITFUN_CLI_SKIP_SHELLRC` | Set `1` to skip bashrc/zshrc edits |
| `CARGO_TARGET_DIR` | Cargo target dir (e.g. `$HOME/bitfun-build/target` on shared mounts) |
| `CARGO_BUILD_JOBS` | Limit rustc parallelism on small VPS |

Example on a small arm64 VPS:

```bash
CARGO_BUILD_JOBS=1 bash src/apps/cli/install.sh
```

### Prerequisites

- Rust toolchain (`rustup` / `cargo`)
- Repository checked out with workspace `Cargo.toml` at the root

## Dev commands (from repo root)

```bash
pnpm run cli:dev      # cargo run
pnpm run cli:build    # cargo build --release
pnpm run cli:install  # same as bash src/apps/cli/install.sh
```
