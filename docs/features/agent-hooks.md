# Agent hooks

Hooks let you run your own commands at fixed points in the BitFun Agent's
lifecycle: before and after a tool call, when a permission prompt would appear,
when a prompt is submitted, around context compaction, around subagents, and
when a session or turn starts or ends. A hook can observe what the Agent is
doing, add context the model will read, rewrite a tool call's arguments, or
block an action outright.

The `hooks.json` document, the event names, the JSON payload on stdin, the
exit-code meanings, and the JSON decision schema on stdout are **the same as
Codex hooks**, so a Codex hook script runs in BitFun unchanged and vice versa.
What differs is where the files live and which extras are supported:

| | Codex | BitFun |
| --- | --- | --- |
| User hooks | `~/.codex/hooks.json` | `<user config dir>/config/hooks.json` |
| Project hooks | `<repo>/.codex/hooks.json` | `<workspace>/.bitfun/config/hooks.json` |
| Hooks inside the main config | `config.toml` `[hooks]` table | not supported — use `hooks.json` |
| Master switch | `[features] hooks = false` | `app.hooks.enabled` in `app.json` |
| Plugin-bundled and managed hooks | supported | not supported |

Some payload fields are also not populated yet; see
[Current gaps](#current-gaps).

> Scope: local workspaces. Hooks are skipped for remote SSH and container
> workspaces, because a local hook process and a remote workspace path do not
> describe the same filesystem.
>
> Non-goal: `prompt` and `agent` handler types. BitFun parses them (so shared
> configuration files stay valid) but only executes `type: "command"` handlers.

## Quick start

1. Create `hooks.json` in your user config directory:

   | Platform | Path |
   | --- | --- |
   | Linux | `~/.config/bitfun/config/hooks.json` |
   | macOS | `~/Library/Application Support/bitfun/config/hooks.json` |
   | Windows | `%APPDATA%\bitfun\config\hooks.json` |

2. Add a hook that logs every shell command the Agent runs:

   ```json
   {
     "description": "My hooks",
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

3. Start a new BitFun session and ask the Agent to run a shell command. Each
   command it runs is appended to `~/bitfun-commands.log`.

Configuration is re-read when the file changes — there is no need to restart
BitFun after editing `hooks.json`.

## Configuration

### File locations and layering

Hooks are read from two layers, in this order:

| Order | Scope | Path |
| --- | --- | --- |
| 1 | User | `<user config dir>/config/hooks.json` (see the table above) |
| 2 | Project | `<workspace>/.bitfun/config/hooks.json` |

Layers are additive: every matching handler from both layers runs, user
handlers first. There is no override or shadowing between layers.

**Project hooks are disabled by default.** A project hook file executes
commands that live inside a checked-out repository, so anyone who can land a
commit could otherwise run code on your machine. Enable them only for
workspaces you trust, under the `app` section of
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
| `app.hooks.project_hooks_enabled` | `false` | Whether `<workspace>/.bitfun/config/hooks.json` is honored. |

### Document schema

```json
{
  "description": "optional, free text",
  "hooks": {
    "<EventName>": [
      {
        "matcher": "optional pattern",
        "hooks": [
          {
            "type": "command",
            "command": "the command to run",
            "commandWindows": "optional Windows override",
            "timeout": 30,
            "statusMessage": "optional label"
          }
        ]
      }
    ]
  }
}
```

The root object accepts only `description` and `hooks`. Any other root key
rejects the whole file — this matches Codex and catches typos early rather than
silently ignoring your configuration.

Each event maps to a list of **matcher groups**. Each group has an optional
`matcher` and a required `hooks` array of handlers.

#### Handler fields

| Field | Required | Meaning |
| --- | --- | --- |
| `type` | yes | Must be `"command"` to execute. `"prompt"` and `"agent"` are accepted but skipped. |
| `command` | yes | Run through `sh -c` (Unix) or `cmd /C` (Windows), with the workspace root as the working directory. |
| `commandWindows` | no | Used instead of `command` on Windows. |
| `timeout` | no | Seconds. Defaults to 600 (`SessionEnd`: 1, capped at 3). |
| `statusMessage` | no | Short label describing what the hook does. |

#### Matcher semantics

A matcher filters which handlers run, based on one value per event (see the
event table). Matchers are regular expressions anchored to the whole value, so
a plain name is an exact match.

| Matcher | Matches |
| --- | --- |
| omitted, `""`, or `"*"` | everything |
| `"Bash"` | exactly `Bash` (not `BashOutput`) |
| `"^Bash$"` | exactly `Bash` |
| `"Edit\|Write"` | `Edit` or `Write` |
| `"mcp__filesystem__.*"` | every tool in that MCP server |
| `"startup"` | that `SessionStart` source (see [Current gaps](#current-gaps)) |

A matcher that is not a valid pattern (or is not a string) never matches
anything, and is reported in the logs. It is not silently treated as
match-all.

### Limits

| Limit | Value |
| --- | --- |
| Maximum size per `hooks.json` | 1 MiB |
| Maximum handlers inspected across all layers | 2048 (invalid and non-`command` handlers count toward it) |
| Maximum model-visible text per hook | 10,000 bytes (truncated with a marker) |

## Events

| Event | Fires when | Matcher value | Can block? |
| --- | --- | --- | --- |
| `SessionStart` | a session is created | `source` (currently always `startup`) | no |
| `SessionEnd` | a session is deleted | — | no |
| `UserPromptSubmit` | a prompt is submitted, before the turn starts | — | yes — rejects the prompt |
| `PreToolUse` | a tool call is scheduled, before permission evaluation and before the tool runs | `tool_name` | yes — denies the tool call |
| `PermissionRequest` | a tool call is about to prompt you for permission | `tool_name` | yes — allows or denies instead of prompting |
| `PostToolUse` | a tool call returned a result (successful or error) | `tool_name` | feedback only (appended to the tool result) |
| `PreCompact` | before context compaction | `trigger` (`auto`, `manual`) | no |
| `PostCompact` | after context compaction | `trigger` | no |
| `SubagentStart` | a subagent turn begins | `agent_type` | no |
| `SubagentStop` | a subagent turn settles successfully | `agent_type` | recorded in logs only |
| `Stop` | a top-level turn is about to finish with a final answer | — | yes — reopens the turn (max 3 times) |

Unknown event names are ignored with a warning; the rest of the file still
loads.

### Current gaps

These are real differences from the Codex contract in this release. They
affect what a hook can rely on, so they are listed rather than implied:

| Field or event | Current behavior |
| --- | --- |
| `transcript_path`, `agent_transcript_path` | always `null` — session transcripts are not exposed to hooks yet |
| `permission_mode` | only `default` or `bypassPermissions`; the other Codex modes never appear |
| `SessionStart.source` | only `startup`; `resume`, `clear`, and `compact` are not dispatched yet |
| `SessionEnd.reason` | always `other` |
| `SubagentStop.stop_hook_active` | always `false` — `SubagentStop` never reopens a subagent turn |
| `SubagentStop` | dispatched when a subagent settles successfully; a failed, cancelled, or timed-out subagent does not dispatch it |
| `Stop` | top-level turns only; subagent turns report through `SubagentStop` |

## The hook process interface

### Input: JSON on stdin

Every payload contains these fields:

```json
{
  "session_id": "string",
  "transcript_path": "string or null",
  "cwd": "string",
  "hook_event_name": "string",
  "model": "string",
  "permission_mode": "default | acceptEdits | plan | dontAsk | bypassPermissions"
}
```

Every event except `SessionStart` and `SessionEnd` also carries `turn_id`.
Each event then adds its own fields:

| Event | Additional fields |
| --- | --- |
| `SessionStart` | `source` |
| `SessionEnd` | `reason` |
| `UserPromptSubmit` | `prompt` |
| `PreToolUse` | `tool_name`, `tool_use_id`, `tool_input` |
| `PermissionRequest` | `tool_name`, `tool_input` |
| `PostToolUse` | `tool_name`, `tool_use_id`, `tool_input`, `tool_response` |
| `PreCompact` / `PostCompact` | `trigger` |
| `SubagentStart` | `agent_id`, `agent_type` |
| `SubagentStop` | `agent_id`, `agent_type`, `agent_transcript_path`, `stop_hook_active`, `last_assistant_message` |
| `Stop` | `stop_hook_active`, `last_assistant_message` |

`stop_hook_active` is `true` when the Agent is already continuing because a
`Stop` hook blocked an earlier attempt to finish. Check it to avoid blocking
forever; BitFun independently caps reopenings at three per turn.

### Output: exit code, and optional JSON on stdout

| Exit code | Meaning |
| --- | --- |
| `0` | Success. If stdout parses as JSON, it is read as a decision document (below). |
| `2` | Block the event. **stderr** is the reason shown to the Agent. |
| anything else | Non-blocking error: a warning is logged and the Agent continues. |

A hook that fails to start, or exceeds its timeout, is also a non-blocking
warning — a broken hook slows the Agent down but never wedges it.

For `SessionStart`, `UserPromptSubmit`, and `SubagentStart`, plain (non-JSON)
stdout on exit 0 becomes context the model reads. For all other events, plain
stdout is ignored, so `echo` debugging never leaks into the conversation.

#### Decision document

All fields are optional:

```json
{
  "continue": true,
  "stopReason": "shown when continue is false",
  "systemMessage": "logged for you, not sent to the model",
  "suppressOutput": false,
  "decision": "block",
  "reason": "why the event was blocked",
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow | deny",
    "permissionDecisionReason": "string",
    "updatedInput": { "command": "the rewritten arguments" },
    "additionalContext": "text the model will read",
    "decision": { "behavior": "allow | deny", "message": "string" }
  }
}
```

Which fields apply per event:

| Event | Fields it honors |
| --- | --- |
| `PreToolUse` | `permissionDecision` (`allow` skips the permission prompt, `deny` blocks the call), `permissionDecisionReason`, `updatedInput`, and `continue: false` (treated as a denial of that tool call) |
| `PermissionRequest` | `decision.behavior` with `decision.message` |
| `PostToolUse` | `decision: "block"` with `reason`, and `additionalContext` — both appended to the tool result the model reads |
| `UserPromptSubmit` | `decision: "block"` with `reason` (rejects the prompt), `additionalContext`, and `continue: false` (also rejects the prompt) |
| `Stop` / `SubagentStop` | `decision: "block"` with `reason` |
| any | `systemMessage` (written to the BitFun log, never sent to the model) |

`permissionDecision: "allow"` waives the interactive permission prompt only.
A tool call that a permission rule denies stays denied — a hook can never
widen the permission policy, only narrow it.

Two fields are accepted for Codex compatibility but currently have no effect
beyond the rows above: `suppressOutput` is parsed and ignored, and
`continue`/`stopReason` are honored only for `PreToolUse` and
`UserPromptSubmit` (elsewhere, use `decision: "block"` to stop the event).

When several handlers match one event, they run in order (user layer first)
and their results merge: the first blocking or denying decision stops the
remaining handlers, a `deny` outranks an earlier `allow`, a later
`updatedInput` replaces an earlier one, and context and system messages
accumulate.

## Examples

### Block edits to protected paths

`PreToolUse` returning a deny decision. The Agent is told why, and continues
the turn without the edit.

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
path = payload.get("tool_input", {}).get("file_path", "")

if "/migrations/" in path:
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "Migrations are generated; edit the schema instead.",
        }
    }))
sys.exit(0)
```

### Format and report after every write

`PostToolUse` returning feedback the model reads. Note that plain stdout and
stderr are ignored for `PostToolUse` — feedback must be `additionalContext`
in a JSON decision document, so the hook is a small script rather than a
one-liner.

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "bash ~/hooks/format.sh",
            "timeout": 60,
            "statusMessage": "Formatting Rust code"
          }
        ]
      }
    ]
  }
}
```

```bash
#!/usr/bin/env bash
# ~/hooks/format.sh
cat >/dev/null                       # drain the payload
output=$(cargo fmt 2>&1 | head -20)
[ -z "$output" ] && exit 0           # nothing to report
jq -n --arg ctx "cargo fmt output:\n$output" \
  '{hookSpecificOutput: {hookEventName: "PostToolUse", additionalContext: $ctx}}'
```

### Require a clean test run before finishing

`Stop` blocking with exit code 2. The stderr text becomes the reason, and the
Agent keeps working instead of finishing.

```bash
#!/usr/bin/env bash
# ~/hooks/require-tests.sh
payload=$(cat)
if [ "$(printf '%s' "$payload" | jq -r '.stop_hook_active')" = "true" ]; then
  exit 0        # already reopened once; don't loop
fi
if ! cargo test --quiet >/tmp/hook-tests.log 2>&1; then
  echo "Tests are failing. See /tmp/hook-tests.log, fix them, then finish." >&2
  exit 2
fi
```

```json
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "bash ~/hooks/require-tests.sh", "timeout": 300 }
        ]
      }
    ]
  }
}
```

### Inject project context at session start

Plain stdout from `SessionStart` becomes context the model reads.

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "git log --oneline -5 && git status --short" }
        ]
      }
    ]
  }
}
```

### Auto-approve a safe tool without prompting

`PermissionRequest` deciding instead of asking you.

```json
{
  "hooks": {
    "PermissionRequest": [
      {
        "matcher": "Read",
        "hooks": [
          {
            "type": "command",
            "command": "printf '{\"hookSpecificOutput\":{\"hookEventName\":\"PermissionRequest\",\"decision\":{\"behavior\":\"allow\",\"message\":\"Reads are always allowed\"}}}'"
          }
        ]
      }
    ]
  }
}
```

## Security

A hook is arbitrary code that runs with your user account's full privileges,
every time its event fires. Treat `hooks.json` like a shell profile:

- Review any hook you did not write before enabling it.
- Keep `app.hooks.project_hooks_enabled` off unless you trust everyone who can
  commit to the repository, and re-check the project hook file after pulling.
- Payload values (prompts, tool arguments, file paths) are model- and
  user-supplied text. Parse them as JSON and never interpolate them into a
  shell command — the examples above read fields with `jq`/`json.load` for
  exactly this reason.
- Hooks inherit BitFun's environment. Do not print secrets to stdout for
  context events, where the text is sent to the model.

## Troubleshooting

| Symptom | Cause |
| --- | --- |
| No hook runs at all | `app.hooks.enabled` is `false`, the file is not at the documented path, or the workspace is remote. |
| Project hooks do not run | `app.hooks.project_hooks_enabled` is `false` (the default). |
| The whole file is ignored | Invalid JSON, or a root key other than `description`/`hooks`. |
| One event is ignored | Misspelled event name — the names are case-sensitive. |
| A handler never runs | Its matcher does not match, or the matcher is not a valid pattern. |
| A `prompt`/`agent` handler never runs | Only `type: "command"` handlers execute. |
| Blocking has no effect | Blocking needs exit code 2 (reason on stderr) or a `decision`/`permissionDecision` field on stdout with exit code 0. |
| Plain `echo` output is not visible to the model | Only `SessionStart`, `UserPromptSubmit`, and `SubagentStart` turn plain stdout into context; elsewhere use `hookSpecificOutput.additionalContext`. |

Configuration problems, non-zero exits, timeouts, and every hook decision are
written to the BitFun backend log. See
[`src/crates/LOGGING.md`](../../src/crates/LOGGING.md) for how to raise the log
level.

## Related

- [`/hooks`](../../src/apps/cli/src/modes/chat/external_hooks.rs) in the CLI
  inspects hooks configured for *other* AI applications (Claude Code, Codex,
  OpenCode). That view is read-only and never executes anything; the hooks
  described in this document are BitFun's own and do execute.
