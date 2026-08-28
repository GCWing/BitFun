---
name: debug
description: Use this skill to locate a runtime bug. Add targeted instrumentation, collect reproduction evidence, and confirm the root cause before changing code.
---

# Evidence-driven debugging

Evidence-driven debugging mode activated. Follow the workflow below:

1. Generate multiple hypotheses based on the problem described by the user.

2. Insert narrow, hypothesis-specific instrumentation for each hypothesis.

3. Provide reproduction steps and ask the user to reproduce the issue.

4. Read the captured runtime data and identify the confirmed root cause.

5. Implement fix based on the confirmed root cause.

6. If the user confirms the issue has been resolved, clean up all inserted log 
   statements; otherwise, add more logs or generate new hypotheses

IMPORTANT: Do not implement a fix until it is supported by runtime evidence.
IMPORTANT: Do not clean up instrumentation until the user confirms the issue
has been fixed.

## Rules

- Before reading `debug-agent.log`, check its size and approximate entry count.
  Do not read a large log end-to-end in one pass; filter by hypothesis ID,
  location, time range, or other relevant keywords, and write a small analysis
  script when aggregation or correlation is needed.

- Before each new request for the user to reproduce the issue, clear the
  existing `debug-agent.log` so that the next capture is attributable to that
  reproduction. If the historical entries may be needed, create a separate
  backup first (for example, with a timestamp or reproduction-batch suffix),
  then clear the active log.

- Separate diagnosis from repair selection. Assess whether the confirmed cause
  calls for a root-cause fix, a smaller mitigation, or both. A mitigation that
  merely masks the symptom must not be presented as the root-cause fix.

- If there are multiple viable fixes, or the root-cause fix has a materially
  larger change surface, present the options, trade-offs, and verification
  implications to the user and ask them to choose before editing production
  code. If one proportionate root-cause fix is clearly preferred, explain why
  and proceed only after the runtime evidence supports it.

## How to Write to Log Files

1. All logs must be written to `{project_root}/debug-agent.log`. Note: absolute paths must be used.

2. Inserted log statements should be wrapped with appropriate comment statements for location and cleanup

3. Log content should include: hypothesis ID (A/B/C/...), log location (e.g., api-get_weather), and specific information

4. Backend: write via file IO
```rust
// #region agent log
{
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r"absolute\path\to\debug-agent.log") {
        let _ = writeln!(f, "log_content");
    }
}
// #endregion
```

5. Frontend: start the bundled receiver script, then write via `fetch`.

```bash
node <this-skill-directory>/scripts/debug-log-server.mjs --root <project-root>
```

This uses the default local port (7469) and writes to `debug-agent.log` in the
project root. Run the script with `--help` to see options for a different
project root, port, log path, or startup behavior.

Inserted frontend logs should use this pattern:

```ts
// #region agent log
void fetch('http://127.0.0.1:7469/log', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    id: 'A',
    loc: 'ComponentName.eventName',
    msg: 'specific observation',
    data: { value },
  }),
}).catch(() => {});
// #endregion
```

The receiver exposes:
- `POST /clear` to clear the log file
- `GET /health` to confirm the server and log path

Keep frontend instrumentation project-agnostic: avoid importing app-specific debug utilities unless the project already has a logging adapter that can POST to this receiver cleanly.
