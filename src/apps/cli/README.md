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
bitfun                                  # interactive TUI
bitfun exec "summarize this project"   # non-interactive, rejects permission requests
bitfun exec "run tests" --auto         # approve tool requests for this invocation
bitfun sessions list
bitfun usage
bitfun doctor
bitfun health
```

`bitfun-cli` is a deprecated compatibility entrypoint. It writes
`Warning: \`bitfun-cli\` is deprecated; use \`bitfun\` instead.` to stderr; new scripts and
integrations must use `bitfun`. Official installers and archives ship both commands as a pair; a
standalone legacy launcher is an incomplete installation and reports how to reinstall the pair.
The naming change is limited to the shell command: the Cargo package, archive prefix, service
identifiers, and persistent paths retain the `bitfun-cli` name.

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
pnpm run cli:install       # build + install both entrypoints (see below)
bitfun                     # start the TUI, then /login with your account
bitfun daemon install      # register auto-start; device stays reachable after exit/reboot
bitfun daemon status       # verify: daemon running + service installed/active
```

```bash
bitfun daemon status      # daemon liveness + auto-start service status
bitfun daemon install     # register and start the auto-start service (requires /login first)
bitfun daemon uninstall   # stop and remove the auto-start service
bitfun daemon run         # foreground mode (used by the service manager; also for debugging)
```

- Prerequisites: a persisted account session (`/login` inside the TUI first), and on Linux a
  working systemd user session (`systemctl --user`). Containers, WSL, and some minimal images do
  not have one; there `daemon install` reports a clear error and you can instead run
  `bitfun daemon run` under your own supervisor (tmux, nohup, a custom unit, ...).
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
- Logs land in `~/.config/bitfun/cli-logs/<session-timestamp>/app.log` (the daemon starts a new
  session directory per process start).

### Account settings sync

Both the interactive CLI and the daemon continuously sync account settings (model configs,
default model, agent preferences, ...) with the account cloud:

- Local changes (TUI model picker / model forms, `bitfun models set-default`, or a peer
  controller's `set_config`) upload after a ~5s debounce, deduped by content hash.
- Cloud changes from other devices are pulled right after process start, then every ~30s, and
  applied to the running process (AI client cache invalidated, config reloaded).
- While a desktop controller is attached (Peer Device Mode), the host fans out
  `account://settings-applied` after applying or uploading settings, so the controller's
  model list / settings UI refreshes without reconnecting.
- The sync cursor persists at `~/.bitfun/account_sync/<user>.settings.json`, so restarts do not
  re-apply unchanged settings.

### Upgrading

Re-run `pnpm run cli:install`, or use the direct Bash/PowerShell command below for your platform.
The installer stages and verifies both entrypoints before replacing an existing pair; a failed
replacement restores the previous pair. It also restarts the daemon's auto-start service when one
is installed (a running daemon otherwise keeps executing the old binary). If you supervise the
daemon yourself (`daemon run` under tmux/nohup/a custom unit), restart it manually after upgrading.

## One-click install (Windows / macOS / Linux)

From the repository root:

```bash
pnpm run cli:install
```

The dispatcher selects PowerShell on Windows and Bash on macOS/Linux. Direct platform commands are:

```bash
# macOS / Linux (amd64 + arm64)
bash src/apps/cli/install.sh

# Windows x64
powershell.exe -NoProfile -ExecutionPolicy Bypass -File src/apps/cli/install.ps1
```

Both installers:

1. `cargo build -p bitfun-cli --release` (native host CPU)
2. Install `bitfun` and the deprecated `bitfun-cli` compatibility entrypoint
3. Verify the primary command and the exact compatibility warning
4. Add the install directory to PATH idempotently unless requested otherwise

The Unix default directory is `~/.local/bin`; the Windows default is
`%LOCALAPPDATA%\BitFun\bin`. Unix updates managed blocks in `~/.bashrc` and `~/.zshrc`;
Windows updates the current user's PATH.

The installer process cannot update the shell that launched it. Open a new terminal, then run:

```bash
bitfun
```

For the current shell, either invoke the printed direct path or temporarily prepend the install
directory to `PATH` using the copyable command printed by the installer.

### Release archives

Official macOS/Linux archives contain `bitfun`, deprecated `bitfun-cli`, `README.md`, and
`PROJECT-README.md`; the Windows x64 ZIP contains the matching `.exe` pair and documents. Extract
the whole archive and keep the two executables together in the same directory. Run `./bitfun` on
macOS/Linux or `.\bitfun.exe` in PowerShell, then add that directory to `PATH` if desired.

Do not copy only `bitfun-cli` from an archive. The deprecated command is intentionally a thin
launcher for its sibling `bitfun`; if the sibling is missing, it reports an incomplete installation
with recovery guidance instead of attempting another lookup.

On a server that should stay reachable for account multi-device access, continue with the
[daemon section](#always-on-account-device-host-daemon) above after `/login`.

### Options / environment

| Variable | Meaning |
|----------|---------|
| `BITFUN_CLI_BIN_DIR` | Install directory (default `~/.local/bin`) |
| `BITFUN_CLI_SKIP_SHELLRC` | Set `1` to skip bashrc/zshrc edits |
| `CARGO_TARGET_DIR` | Cargo target dir (e.g. `$HOME/bitfun-build/target` on shared mounts) |
| `CARGO_BUILD_JOBS` | Limit rustc parallelism on small VPS |

Windows accepts `-BinDir <path>` and `-SkipPathUpdate` arguments. Unix accepts the environment
variables above.

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
pnpm run cli:install  # dispatch to install.ps1 on Windows or install.sh on macOS/Linux
```
