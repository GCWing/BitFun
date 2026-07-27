---
name: arkts-runtime-fix
description: Load for ArkTS/JavaScript jscrash, runtime crash, uncaught exception, stack trace, faultlog, or hilog diagnosis. Also load when the app 闪退/崩溃/白屏, exits after 点击/启动/launch, or build succeeds but runtime fails (no compile error). Use before broad Read/Glob on crash-only tasks.
---

# Harmony JSCrash Fixes

Use this skill to diagnose and fix ArkTS or JavaScript runtime crashes with minimal edits.

Use this skill's private Node scripts under `skills/arkts-runtime-fix/scripts/` to parse crash evidence, inspect recent faultlogger entries, or collect hilog when no better evidence is available.

If `node` is unavailable, stop and explain that the private scripts cannot run.

## When To Load

Load this skill when the issue looks like one of these:

- Runtime logs show `TypeError`, `ReferenceError`, `RangeError`, `SyntaxError`, `BusinessError`, or similar exceptions.
- The app exits, flashes back, or white-screens during launch or after a tap.
- The user provides a `jscrash` log, stack trace, or a temporary log file with `@file`.
- Build succeeds, but runtime behavior fails immediately.

## Core Approach

Prefer a concrete crash anchor before broad code exploration. A good anchor can come from:

- a provided crash log
- a stack trace
- a clear page or module named by the user
- a recent device-side faultlog or hilog when no better evidence is available

Avoid broad `Read` / `Glob` / `Explore` across the whole project until you have at least one concrete anchor such as:

- `error_type`
- `error_message`
- `suspected_file`
- `top_stack`
- or a clearly named crash entry point from the user

Do not over-collect logs. If the user already gave enough crash evidence, parse that evidence first and move into focused reading and minimal fixes.

## Tool And Script Contract

### Private Node scripts

Run the private scripts through Shell like this:

```bash
node "{SKILL_DIR}/scripts/<script>.mjs" ...
```

`{SKILL_DIR}` is the absolute path of this skill directory at runtime.

Scripts print stable `key: value` text to stdout. A non-zero exit code means the current step could not continue and the agent should report the reason instead of guessing.

## Preferred Flows

### Case A: The user already provided raw crash text

```bash
node "{SKILL_DIR}/scripts/jscrash-report.mjs" --log-text "{crashLog}" --bundle-name "{bundleName}" --include-text
```

### Case B: The user provided `@file` or a local log path

```bash
node "{SKILL_DIR}/scripts/parse-jscrash-log.mjs" --log-file "{logFilePath}" --bundle-name "{bundleName}" --include-text
```

### Case C: The user only described symptoms and did not provide logs

Log collection is optional here. Use it when you still need a concrete runtime anchor.

Before collecting device evidence:

1. Read `AppScope/app.json5` and take the exact `app.bundleName` value (for example `com.example.hmos.sample`). Use that string as `{bundleName}` in every script below.
2. Do not guess `bundleName` from `vendor`, module folder names, or prefixes such as `com.example`.
3. Resolve the target device:
   - If the user provided a `deviceId`, use it for every device-side command.
   - If no `deviceId` is provided, prefer Bash `devecocli device list` first.
   - If exactly one device is connected, use that device.
   - If multiple devices are connected, ask the user which device to inspect. Do not probe faultlogger, fetch faultlog, or collect hilog until the user selects a device.
   - If no devices are connected, report that device-side evidence cannot be collected and ask for a connected device or a local crash log.

After the user reproduces the crash on the selected device, inspect recent faultlogger evidence:

```bash
node "{SKILL_DIR}/scripts/probe-faultlogger.mjs" --bundle-name "{bundleName}" --device-id "{deviceId}" --max-age-minutes "30" --limit "10"
```

If `status: found`, fetch and parse the latest faultlog:

```bash
node "{SKILL_DIR}/scripts/fetch-faultlog.mjs" --faultlog-name "{latestFaultlog}" --device-id "{deviceId}" --output-dir "{tempDir}"
node "{SKILL_DIR}/scripts/parse-jscrash-log.mjs" --log-file "{localFaultlogPath}" --bundle-name "{bundleName}" --source file --include-text
```

If `status: not_found`, do not broad-read the project or guess from symptoms alone. Ask the user to reproduce the crash on the selected device, then probe faultlogger again immediately.

If faultlogger is unavailable, the reproduced crash still does not create a matching faultlog, or the parsed faultlog is still not enough, fall back to a `devecocli log`

```bash
node "{SKILL_DIR}/scripts/collect-hilog.mjs" --device-id "{deviceId}" --lines "4000" --output-dir "{tempDir}"
node "{SKILL_DIR}/scripts/parse-jscrash-log.mjs" --log-file "{hilogPathFromCollect}" --bundle-name "{bundleName}" --source hilog --include-text
```

## Output Contract

The leading `key: value` block from `jscrash-report.mjs` and `parse-jscrash-log.mjs` includes:

- `status`: `detected` | `no_crash_signature` | `parse_failed`
- `source`: `file` | `text` | `hilog` and similar sources
- `error_type`
- `error_message`
- `suspected_file`
- `top_stack`: `|`-joined frames
- `keywords`: comma-separated
- `next_action`

`--include-text` appends a human-readable summary after the structured block.

If `status: no_crash_signature`, explain that the evidence is weak and ask for a better log, clearer repro, or an additional runtime clue before broad code reading.

## Common Crash Signatures

| Signature | Typical Cause | First Fix Direction |
|---|---|---|
| `TypeError` on property access | Null or undefined state during render or lifecycle | Guard null state, initialize earlier, or move logic to a safer lifecycle |
| `ReferenceError` | Wrong scope, stale import, missing symbol | Fix symbol ownership, import path, or callback capture |
| `RangeError` | Invalid index, recursion loop, oversized access | Add bounds checks, break loops, clamp indexes |
| `BusinessError` / `ParameterError` | Framework API preconditions not met | Validate args, permissions, or call timing |

## Interpretation Rules

- Prefer application frames over framework noise.
- Treat the first concrete `.ets`, `.ts`, or `.js` path as the starting point, not the final truth.
- If the user gave repro steps, trust them over a simplistic stack-only guess.
- If the stack points to a non-entry page, assume an interaction-triggered path unless evidence proves a cold-start crash.
- Do not refactor broadly. Fix the crash path first.

## Conversational Shape

1. Say what evidence you already have.
2. If logs are missing, say whether you are using faultlogger or hilog to get a better anchor.
3. Once you have an anchor, switch into focused code reading and minimal fixing.

## Constraints

- Never claim a crash fix from prompt reasoning alone.
- Never replace a root-cause fix with retries, arbitrary delays, or broad defensive rewrites.
- If unfamiliar `@ohos.*` or `@kit.*` APIs are involved, check their constraints before editing.
- This skill does not decide the final compile / run / verification order; the primary agent owns that.
