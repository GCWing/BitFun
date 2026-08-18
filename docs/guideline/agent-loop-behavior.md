# Agent loop behavior

> Companion to the root `AGENTS.md` entry (STD-06). Open when changing the agent
> loop, tool-call repetition handling, or anti-loop safeguards.
>
> Also see nearest module docs under `src/crates/execution/` (especially
> `agent-runtime`).
>
> [中文](agent-loop-behavior.zh-CN.md)

## Hard limits

- Do not add hard-coded limits or pattern checks to the agent loop as a first
  response to looping behavior, such as blocking repeated tool calls by string
  or count alone.
- Excessive hard-coding turns the agent loop into a brittle workflow engine.
  Investigate the root cause first: tool behavior, model interaction, session
  context packaging, prompt/tool schema design, or state synchronization issues.
