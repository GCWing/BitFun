# dsh-openbitfun

A [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) (dsh)
plugin bundle that lets a dsh agent **use OpenBitFun**: it bridges dsh tools to the
OpenBitFun Agent SDK Host (`openbitfun-sdk-host`), the standalone local JSON-RPC-over-stdio
entry point of the OpenBitFun Agent Runtime
([protocol](../../src/crates/interfaces/sdk-host/), [app](../../src/apps/sdk-host/)).

The plugin registers one dsh tool:

| Tool | What it does |
|---|---|
| `openbitfun_run` | Runs one OpenBitFun agent turn against a workspace and returns OpenBitFun's final text answer. The result carries a `session_id`; passing it back continues the same OpenBitFun conversation (multi-turn). |

## How it works

- The plugin spawns one `openbitfun-sdk-host` process lazily per resolved binary and
  speaks the SDK Host protocol: `initialize`, `session/create`, `query/start`,
  `query/event` (streamed assistant text), `query/result` (terminal status),
  `query/cancel`, `session/close`, `shutdown`.
- Each dsh session is mapped to one transient OpenBitFun session on first use; the
  oldest mappings are closed past `maxSessions`.
- Aborting the dsh tool call (user stop, timeout policy) cancels the OpenBitFun turn.
- If the host process dies, pending calls fail with the captured stderr tail and
  the next call respawns the host.

The host binary is located per call in this order:

1. the `hostPath` patch-config option of the `openbitfun` row,
2. the `OPENBITFUN_SDK_HOST` environment variable,
3. `target/debug` / `target/release` walking up from the dsh session cwd
   (a OpenBitFun checkout-local build),
4. `openbitfun-sdk-host` on `PATH`.

## Build the SDK Host

```bash
cargo build -p openbitfun-sdk-host-app
# binary: target/debug/openbitfun-sdk-host[.exe]
```

The SDK Host reuses the OpenBitFun user config (provider/model) of the local
machine, so a configured OpenBitFun installation makes `openbitfun_run` work out of the
box.

## Install into a dsh profile

```bash
dsh plugin --profile web add /absolute/path/to/extensions/dsh-openbitfun
dsh plugin --profile headless add /absolute/path/to/extensions/dsh-openbitfun
```

`dsh plugin` installs the package into the profile and, because this package
declares `dsh.bundle.patch`, appends `dsh-openbitfun` to the profile's bundle stack.
Restart the profile for the change to take effect.

Custom host binary or caps (in the profile's `cordis.patch.yml`):

```yaml
- id: openbitfun
  config:
    hostPath: '/absolute/path/to/openbitfun-sdk-host'
    maxSessions: 8
    queryTimeoutMs: 1800000
    requestTimeoutMs: 30000
```

## Smoke test (no dsh required)

```bash
node test/smoke.mjs
```

The script boots the resolved host binary, initializes the protocol, creates a
session bound to the OpenBitFun checkout root, runs one trivial query, and asserts a
completed turn with non-empty assistant text.

## Current limitations (v1)

- **Local scenario only.** The OpenBitFun session workspace is the dsh session cwd
  on the same machine; remote-workspace and peer-device paths are not exercised.
- **Interactive permissions are auto-rejected** by the current SDK Host
  candidate (`AutoApproveAsk = false`), so a OpenBitFun turn that needs an approval
  fails; use `openbitfun_run` for non-interactive subtasks.
- **Transient sessions.** OpenBitFun sessions live only as long as the host
  connection; a dsh restart starts fresh OpenBitFun conversations. Session cleanup
  removes runtime stores, snapshots, and terminal bindings only — it never
  deletes the workspace directory.
- The SDK Host protocol is OpenBitFun's internal candidate (`stability:
  not_delivered`), so this bridge tracks the local OpenBitFun build rather than a
  frozen external API.
