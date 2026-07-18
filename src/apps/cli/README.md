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

## Always-on account device host (daemon)

Account multi-device access requires the target device to hold a live relay connection. On a
server that is usually not true while no interactive CLI is running. The daemon solves this: it is
a headless Peer Host process that restores the persisted account session and holds the relay
device-routing connection, so other devices on the account can reach this machine whenever it is
up.

Full setup flow on a server:

```bash
bash src/apps/cli/install.sh    # build + install the CLI (see below)
bitfun-cli                      # start the TUI, then /login with your account
bitfun-cli daemon install       # register auto-start; device stays reachable after exit/reboot
bitfun-cli daemon status        # verify: daemon running + service installed/active
```

```bash
bitfun-cli daemon status      # daemon liveness + auto-start service status
bitfun-cli daemon install     # register and start the auto-start service (requires /login first)
bitfun-cli daemon uninstall   # stop and remove the auto-start service
bitfun-cli daemon run         # foreground mode (used by the service manager; also for debugging)
```

- Prerequisites: a persisted account session (`/login` inside the TUI first), and on Linux a
  working systemd user session (`systemctl --user`). Containers, WSL, and some minimal images do
  not have one; there `daemon install` reports a clear error and you can instead run
  `bitfun-cli daemon run` under your own supervisor (tmux, nohup, a custom unit, ...).
- Linux: installs a systemd user unit (`~/.config/systemd/user/bitfun-cli-daemon.service`) and
  enables linger, so the daemon starts at boot and keeps running without an interactive login
  session. macOS: installs a LaunchAgent. Windows is not supported for auto-start; use
  `daemon run` instead.
- The interactive CLI detects a running daemon and skips its own relay connection (same-machine
  processes share one `device_id`; last AuthConnect wins). Without a daemon, the interactive CLI
  connects by itself as before.
- Logging out (`/logout`) signals the daemon to shut down so the device goes offline immediately;
  a daemon whose token is rejected by the relay exits on its own instead of staying "online" with
  a doomed token.

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

On a server that should stay reachable for account multi-device access, continue with the
[daemon section](#always-on-account-device-host-daemon) above after `/login`.

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
