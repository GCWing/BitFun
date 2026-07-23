# Streaming Output Collection Timing

This document describes how the terminal service collects command output during
streaming execution, covering the OSC 633 state machine, the polling-based
completion detection, and the Windows ConPTY workarounds.

## Architecture Overview

Output collection involves two independent layers:

```
┌──────────────────────────────────────────────────────────────┐
│  bash_tool.rs                                                │
│  Consumes CommandStream, accumulates output for tool result  │
└────────────────────────┬─────────────────────────────────────┘
                         │ CommandStream (mpsc channel)
                         │   Started / Output / Completed / Error
┌────────────────────────┴─────────────────────────────────────┐
│  manager.rs — execute_command_stream_with_options             │
│  Layer 2: Polls integration every 50ms, detects completion   │
│  by output stabilization, sends Completed with               │
│  explicit completion_reason                                  │
└────────────────────────┬─────────────────────────────────────┘
                         │ reads integration.get_output().len()
                         │ reads integration.state()
┌────────────────────────┴─────────────────────────────────────┐
│  integration.rs — ShellIntegration                           │
│  Layer 1: Parses OSC 633 sequences from PTY data stream,     │
│  drives CommandState machine, accumulates output_buffer      │
└────────────────────────┬─────────────────────────────────────┘
                         │ raw PTY data (process_data calls)
┌────────────────────────┴─────────────────────────────────────┐
│  PTY process (ConPTY on Windows, unix PTY on Linux/macOS)    │
└──────────────────────────────────────────────────────────────┘
```

## Layer 1: ShellIntegration State Machine

### OSC 633 Sequence Lifecycle

A single command execution produces the following OSC 633 sequence:

```
633;A ─→ 633;B ─→ (user types) ─→ 633;E ─→ 633;C ─→ [output] ─→ 633;D ─→ 633;A ─→ ...
  │         │                        │         │                     │         │
  │         │                        │         │                     │         └ next prompt
  │         │                        │         │                     └ CommandFinished
  │         │                        │         └ CommandExecutionStart (Enter pressed)
  │         │                        └ CommandLine (command text recorded)
  │         └ CommandInputStart (prompt ended, cursor waiting for input)
  └ PromptStart (shell begins rendering prompt)
```

### CommandState Transitions

```
                    ┌──────────┐
       ─────────────│   Idle   │ (initial state)
                    └────┬─────┘
                         │ 633;A
                    ┌────▼─────┐
                    │  Prompt  │
                    └────┬─────┘
                         │ 633;B
                    ┌────▼─────┐
                    │  Input   │
                    └────┬─────┘
                         │ 633;C
                    ┌────▼──────┐
             ┌──────│ Executing │ ← output_buffer.clear()
             │      └────┬──────┘
             │           │ 633;D
             │      ┌────▼──────┐
             │      │ Finished  │ ← post_command_collecting = true
             │      └────┬──────┘
             │           │ 633;A
             │      ┌────▼─────┐
             │      │  Prompt  │ ← begin ConPTY reorder detection
             │      └────┬─────┘
             │           │ 633;B
             │      ┌────▼─────┐
             │      │  Input   │ ← ConPTY reorder detection resolved
             │      └────┬─────┘
             │           │ 633;C (next command)
             └───────────┘
```

### Output Collection Rules

`should_collect()` returns `true` when either condition is met:

1. **State-based**: `state` is `Executing` or `Finished`
2. **Flag-based**: `post_command_collecting` is `true`

Within `process_data()`, plain text (non-OSC content) is handled as follows:

- **Before OSC sequences**: Accumulated in a local `plain_output` buffer
- **At `should_flush` sequences** (CommandFinished, PromptStart): If `should_collect()`,
  flush `plain_output` into `output_buffer` before the state transition
- **At end of data chunk**: If `should_collect()`, append remaining `plain_output` to
  `output_buffer`; otherwise discard

### Key Flags

| Flag | Set `true` | Set `false` | Purpose |
|------|-----------|------------|---------|
| `post_command_collecting` | CommandFinished (D) | PromptStart (A), CommandExecutionStart (C), or ConPTY reorder detection at CommandInputStart (B) | Keep collecting late ConPTY output after state leaves Executing/Finished |
| `detecting_conpty_reorder` | PromptStart (A) when `post_command_collecting` was true | CommandInputStart (B), CommandExecutionStart (C) | Detect whether ConPTY reordered sequences ahead of rendered output |
| `command_just_finished` | CommandFinished (D) | Cleared by manager after reading | One-shot flag so manager catches Finished even if state already moved to Prompt/Input |

## Layer 2: Manager Polling Loop

`execute_command_stream_with_options` spawns a task that polls
`ShellIntegration` every **50ms** and decides when the command stream is
complete.

### Completion Decision Logic

```
poll every 50ms:
    read state, output, output_len

    if timeout reached:
        send SIGINT immediately
        keep polling during a short interrupt grace window
        if command still has not settled when grace expires:
            COMPLETE with completion_reason = TimedOut

    if command_just_finished and no finished_exit_code yet:
        record finished_exit_code, reset idle counter

    match state:
        Finished:
            if first time seeing Finished:
                record finished_exit_code
            else:
                if output_len == last_output_len:
                    idle++ → if idle >= 4 (200ms): COMPLETE ✓
                else:
                    reset idle

        Idle / Prompt / Input:
            if finished_exit_code is set:
                if output_len == last_output_len:
                    idle++ → if idle >= 10 (500ms): COMPLETE ✓
                else:
                    reset idle
            else (no finish signal):
                if output_len == last_output_len:
                    idle++ → if idle >= 20 (1000ms): COMPLETE ✓ (fallback)
                else:
                    reset idle

        Executing:
            reset all counters (still running)
```

`Completed` now carries an explicit `completion_reason`:

- `Completed` - command reached a normal terminal state
- `TimedOut` - timeout fired, terminal sent `SIGINT`, and the stream returned the best available output snapshot

### Stabilization Thresholds

| Condition | Idle polls required | Wall time |
|-----------|-------------------|-----------|
| State = Finished, output stable | 4 | 200ms |
| State = Prompt/Input, has finished_exit_code | 10 | 500ms |
| No finish signal (fallback) | 20 | 1000ms |

The longer 500ms window for Prompt/Input exists specifically because ConPTY may
deliver rendered output **after** the state has already transitioned past
Finished. The `post_command_collecting` flag ensures this late data enters
`output_buffer`, which resets the idle counter and extends the wait.

When a timeout occurs, the manager also uses a separate **500ms interrupt grace
window** after sending `SIGINT` so partial output and the final exit transition
can still be collected before the stream completes as `TimedOut`.

## Interaction Between Layers

A typical bash tool execution timeline:

```
Time   PTY Data Stream              integration.rs              manager.rs
─────  ─────────────────────────    ─────────────────────────   ──────────────────
 0ms   633;A                        state → Prompt
 2ms   633;B                        state → Input
 4ms   (bash_tool writes cmd+\n)
 6ms   633;E;ls                     record command text
 8ms   633;C                        state → Executing            poll: Executing
                                    output_buffer.clear()
10ms   "file1.txt\r\n"             output_buffer += 12B         poll: Executing
15ms   "file2.txt\r\n"             output_buffer += 12B
20ms   633;D;0                      state → Finished             poll: Finished (1st)
                                    post_command_collecting=true      record exit_code
22ms   633;A                        state → Prompt
                                    detecting_conpty_reorder=true
24ms   "PS E:\path> "              (between A and B, prompt)
26ms   633;B                        state → Input
                                    plain_output not empty →
                                    don't re-enable collecting
50ms                                                             poll: Input, len=24
100ms                                                            poll: Input, len=24, idle=1
...                                                              ...
500ms                                                            poll: Input, len=24, idle=10
                                                                 → COMPLETE ✓ (send 24B)
```

## Windows ConPTY Reordering

ConPTY is the Windows pseudo-terminal layer that translates VT sequences for
the Windows console subsystem. It introduces a well-known issue: **rendered
output and pass-through OSC sequences may be delivered out of order**.

### Observed Reordering Patterns

**Pattern 1 — Late output (sequences arrive before rendered content):**

```
Expected:  [output] [633;D] [633;A] [prompt] [633;B]
Actual:    [633;D] [633;A] [633;B] [output+prompt]
```

The shell integration sequences pass through immediately, but ConPTY's
rendering pipeline buffers the actual text and delivers it later. Without
mitigation, the state machine reaches Input before the output arrives, causing
data loss.

**Fix**: `post_command_collecting` flag keeps `should_collect()` returning true
after Finished, so late-arriving output still enters the buffer.

**Pattern 2 — Early prompt (rendered content arrives before sequences):**

```
Expected:  [output] [633;D] [633;A] [prompt] [633;B]
Actual:    [output+prompt] [633;D] [633;A] [633;B]
```

ConPTY renders both the command output AND the prompt text before delivering
the CommandFinished sequence. Since the prompt is part of the `plain_output`
when the `should_flush` before CommandFinished fires, it gets flushed into
`output_buffer` as command output.

**Status**: This pattern cannot be reliably fixed at the shell integration
level without content-based heuristics (e.g., regex matching the prompt text).
The prompt may appear in tool output in this case.

### ConPTY Reorder Detection Mechanism

To handle Pattern 1 while minimizing prompt inclusion, the code uses a
two-phase detection between PromptStart (A) and CommandInputStart (B):

```
At 633;A (PromptStart):
    if post_command_collecting:
        post_command_collecting = false    // tentatively stop
        detecting_conpty_reorder = true    // start watching

At 633;B (CommandInputStart):
    if detecting_conpty_reorder:
        if plain_output between A and B is empty:
            // No prompt text arrived → ConPTY reordered (Pattern 1)
            post_command_collecting = true  // re-enable for late output
        else:
            // Prompt text present → normal ordering
            // post_command_collecting stays false
        detecting_conpty_reorder = false
```

This heuristic correctly excludes the prompt in normal ordering while still
capturing late output in Pattern 1. Pattern 2 remains unmitigated.
