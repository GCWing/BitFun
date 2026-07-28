# Agent hooks

Hooks let you run your own commands at fixed points in the BitFun Agent's
lifecycle: before and after a tool call, when a permission prompt would appear,
when a prompt is submitted, around context compaction, around subagents, and
when a session or turn starts or ends. A hook can observe what the Agent is
doing, add context the model will read, rewrite a tool call's arguments, or
block an action outright.

## BitFun hooks are Codex hooks

BitFun implements **the Codex hook contract**, not a BitFun dialect:

- the same `hooks.json` document — events, matcher groups, handler fields;
- the same event names (`PreToolUse`, `PostToolUse`, `PermissionRequest`,
  `UserPromptSubmit`, `PreCompact`, `PostCompact`, `SessionStart`,
  `SessionEnd`, `SubagentStart`, `SubagentStop`, `Stop`);
- the same JSON payload on stdin, with the same field names;
- the same exit-code meanings (`0` success, `2` block with stderr as the
  reason, anything else a non-blocking error);
- the same JSON decision schema on stdout (`permissionDecision`,
  `updatedInput`, `additionalContext`, `decision`/`reason`, …).

**A Codex hook script runs in BitFun unchanged, and vice versa — there is
nothing to port.**

So this page does not restate the reference. For event semantics, the exact
payload fields per event, and the decision schema, use Codex's own
documentation, which covers all of it well:

**→ <https://learn.chatgpt.com/docs/hooks>**

The rest of this page is only what is BitFun-specific: where the files live,
how to switch hooks on, and where BitFun currently differs.

## Where BitFun reads hooks

Codex reads `~/.codex/hooks.json`; BitFun reads its own config directory
instead. Everything inside the file is identical.

| Scope | Path |
| --- | --- |
| User | `<user config dir>/config/hooks.json` |
| Project | `<workspace>/.bitfun/config/hooks.json` |

The user config directory is `~/.config/bitfun` on Linux,
`~/Library/Application Support/bitfun` on macOS, and `%APPDATA%\bitfun` on
Windows.

Both layers are additive: every matching handler runs, user handlers first.
There is no override or shadowing between them. Changes are picked up without
restarting BitFun.

## Turning hooks on

**Settings → Agent Hooks**, or directly under the `app` section of
`<user config dir>/config/app.json`:

```json
{
  "app": {
    "hooks": {
      "enabled": true,
      "project_hooks_enabled": true
    }
  }
}
```

| Setting | Default | Meaning |
| --- | --- | --- |
| `app.hooks.enabled` | `true` | Master switch. `false` disables all hooks. |
| `app.hooks.project_hooks_enabled` | `false` | Whether the project hook file is honored. |

**Project hooks are off by default.** A project hook file executes commands
that live inside a checked-out repository, so anyone who can land a commit
could otherwise run code on your machine. Turn it on only for repositories
you trust, and re-check the file after pulling.

Codex's `[features] hooks = false` has no BitFun equivalent — use
`app.hooks.enabled` instead.

## Quick start

Create `<user config dir>/config/hooks.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "jq -r '.tool_input.command' >> ~/bitfun-commands.log"
          }
        ]
      }
    ]
  }
}
```

Start a new session and ask the Agent to run a shell command; each command it
runs is appended to `~/bitfun-commands.log`.

A hook that blocks — here, refusing edits under `migrations/`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [{ "type": "command", "command": "python3 ~/hooks/protect.py" }]
      }
    ]
  }
}
```

```python
#!/usr/bin/env python3
import json, sys

payload = json.load(sys.stdin)
if "/migrations/" in payload.get("tool_input", {}).get("file_path", ""):
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "Migrations are generated; edit the schema instead.",
        }
    }))
sys.exit(0)
```

## Where BitFun differs from Codex

Everything not listed here behaves as the Codex documentation describes.

### Not supported

| Codex feature | BitFun |
| --- | --- |
| `config.toml` `[hooks]` table | not read — put hooks in `hooks.json` |
| `[features] hooks = false` | use `app.hooks.enabled` |
| Plugin-bundled and managed hooks (`PLUGIN_ROOT`, `managed_dir`) | not supported |
| `prompt` and `agent` handler types | parsed so shared files stay valid, but skipped — only `type: "command"` executes |
| Remote workspaces | hooks are skipped entirely: a local hook process and a remote workspace path do not describe the same filesystem |

### Fields not populated yet

| Field or event | Current behavior |
| --- | --- |
| `transcript_path`, `agent_transcript_path` | always `null` |
| `permission_mode` | only `default` or `bypassPermissions` |
| `SessionStart.source` | only `startup`; `resume`, `clear`, `compact` are not dispatched |
| `SessionEnd.reason` | always `other` |
| `SubagentStop.stop_hook_active` | always `false` |
| `SubagentStop` | dispatched when a subagent settles successfully, not on failure, cancellation, or timeout |
| `Stop` | top-level turns only; subagent turns report through `SubagentStop` |

### Behavior worth knowing

- **A hook can narrow the permission policy, never widen it.** A `PreToolUse`
  `permissionDecision: "allow"` waives the interactive prompt, but a tool call
  denied by a permission rule stays denied.
- `suppressOutput` is parsed and currently ignored.
- `continue: false` is honored for `PreToolUse` and `UserPromptSubmit`; for
  other events use `decision: "block"`.
- `PostToolUse` fires for error results too, not only successes.
- Limits: 1 MiB per `hooks.json`, 2048 handlers inspected across all layers
  (invalid and non-`command` handlers count toward it), and 10,000 bytes of
  model-visible text per hook before truncation.

## Security

A hook is arbitrary code that runs with your user account's full privileges,
every time its event fires. Treat `hooks.json` like a shell profile:

- Review any hook you did not write before enabling it.
- Keep project hooks off unless you trust everyone who can commit to the
  repository.
- Payload values (prompts, tool arguments, file paths) are model- and
  user-supplied text. Parse them as JSON and never interpolate them into a
  shell command — that is why the examples above read fields with
  `jq`/`json.load`.
- Do not print secrets to stdout for `SessionStart`, `UserPromptSubmit`, or
  `SubagentStart`, where plain stdout becomes context the model reads.

## Troubleshooting

| Symptom | Cause |
| --- | --- |
| No hook runs at all | `app.hooks.enabled` is `false`, the file is not at the documented path, or the workspace is remote. |
| Project hooks do not run | `app.hooks.project_hooks_enabled` is `false` (the default). |
| The whole file is ignored | Invalid JSON, or a root key other than `description`/`hooks`. |
| One event is ignored | Misspelled event name — the names are case-sensitive. |
| A handler never runs | Its matcher does not match, or the matcher is not a valid pattern. Matchers are regular expressions anchored to the whole value, so `Bash` matches `Bash` but not `BashOutput`. |
| A `prompt`/`agent` handler never runs | Only `type: "command"` handlers execute. |
| Blocking has no effect | Blocking needs exit code 2 (reason on stderr), or a `decision`/`permissionDecision` field on stdout with exit code 0. |
| Plain `echo` output is not visible to the model | Only `SessionStart`, `UserPromptSubmit`, and `SubagentStart` turn plain stdout into context; elsewhere use `hookSpecificOutput.additionalContext`. |

Configuration problems, non-zero exits, timeouts, and hook decisions are
written to the BitFun backend log. See
[`src/crates/LOGGING.md`](../../src/crates/LOGGING.md) for how to raise the
log level.

## Related

- CLI `/hooks` shows the hooks described here — which files they came from,
  which layers are active, and what each matcher group would run. It reports
  the configuration only; edit `hooks.json` to change it.
- CLI `/hooks_external` inspects hooks configured for *other* AI applications
  (Claude Code, Codex, OpenCode). That view is read-only and never executes
  anything; the hooks described here are BitFun's own and do execute.
