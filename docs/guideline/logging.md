# Logging and Diagnostics Standard

> Companion to root `AGENTS.md` (STD-06). This page is the repository-wide
> **index and shared ops norms** for logging. Surface-specific APIs and examples
> stay in the platform guides below — do not fork a second logging dialect here.

Platform guides (API authority for each surface):

- Frontend: [`src/web-ui/LOGGING.md`](../../src/web-ui/LOGGING.md)
- Rust backend and applications: [`src/crates/LOGGING.md`](../../src/crates/LOGGING.md)

## Non-negotiable (repository AGENTS)

Logs must be **English-only**, with **no emojis**.

New code must follow this page and the owning surface `LOGGING.md`. Existing
call sites that do not conform are legacy debt, not examples to copy.

## Applicability note (this repository)

This file keeps shared norms that match BitFun's current logging surfaces
(`log` / frontend logger, local diagnostics, and existing optional telemetry
hooks such as edit-constraint or token-usage aggregates).

It does **not** claim OpenBitFun-only observability crates, a
`TelemetryLevel::Debug` remote channel, `BITFUN_TELEMETRY_LEVEL`, or typed
`ValidatedRecord` / metric-allowlist APIs that are **not shipped in this
repository**. If those land later, document them in the owning crate and then
extend this page.

## Choose the Correct Channel

Logging level is not a privacy or performance boundary. Choose the data channel
before choosing a level.

| Channel | Purpose | Content policy |
|---|---|---|
| Operational local log | Runtime lifecycle, safe state transitions, degradation, and failures | Safe metadata only; never raw user, model, tool, file, terminal, or protocol content |
| Scoped local diagnostic | Reproduce a specific problem (for example Flow Chat layout or model exchange) | Disabled by default, explicitly enabled, narrowly scoped, bounded, and never automatically uploaded |
| Telemetry (when a sink exists) | Aggregated operational health | Typed allowlist only; no arbitrary log bodies, attributes, errors, identifiers, paths, or content |
| User-facing status | Information the user must understand or act on | Use the owning UI, CLI output, or API result rather than a log entry |

Operational logs must never be forwarded wholesale into telemetry. Scoped
diagnostics must not be implemented by lowering the global operational log
level or by adding raw payloads to ordinary `TRACE` calls.

## Default Levels

Align with the surface `LOGGING.md` defaults unless a product-specific contract
requires a stricter value:

- Development persisted logs: `DEBUG` (frontend often defaults INFO in practice —
  follow the owning guide).
- Release persisted logs: `INFO` or stricter per surface guide.
- Interactive console or stderr in release builds: `WARN` or stricter.
- `TRACE`: explicit, temporary diagnostic use only.
- Content-bearing scoped diagnostics: off until explicitly enabled.

A sink may use a stricter filter. Changing a filter must not change which data
is considered safe to log.

## Level Semantics

| Level | Use when | Do not use for |
|---|---|---|
| `TRACE` | Deep, opt-in detail about a bounded operation | Per-chunk or per-frame output, raw payloads, secrets, or expensive values built before the level check |
| `DEBUG` | State-machine transitions, branch decisions, retries, bounded summaries | Information required to operate a release build or routine high-frequency callbacks |
| `INFO` | Low-frequency lifecycle events and significant successful terminal outcomes | Every request, refresh, event, file, tool chunk, or polling iteration |
| `WARN` | Continues but degraded, data dropped, fallback used, capacity/continuity at risk | Expected absence, user cancellation, normal retries, or errors returned unchanged |
| `ERROR` | Owning operation failed, invariant violated, data may be corrupted, or a security boundary failed | Expected control flow or the same error at every propagation layer |

Additional rules:

- User cancellation is normally `DEBUG` or no log.
- An expected optional miss is `DEBUG` or no log.
- Log individual retry attempts at `DEBUG`. Log one `WARN` when recovery causes
  visible degradation, or one `ERROR` when all attempts are exhausted.
- Do not promote a message to `WARN` or `ERROR` merely to make it visible under
  a stricter production filter.

## What to Log

Prefer one terminal summary from the component that owns the operation. Useful
fields include:

- `operation` and `outcome` using stable, bounded values.
- `duration_ms` in Rust or `durationMs` in frontend diagnostics.
- Counts, byte sizes, queue depth, retry count, and dropped or suppressed count.
- A typed `error_type` or `error_code`, and whether the condition is retryable.
- An opaque local correlation ID when needed to connect related entries.

Do not log start and finish for every routine operation.

## Message and Field Format

- Messages must be English-only and contain no emojis.
- Use a stable, concise message template. Put variable data in fields rather
  than interpolating it into prose.
- Use a stable module context or target. Do not derive logger names from runtime
  input.
- Keep all values bounded. Record counts or classifications instead of arrays,
  object dumps, or unbounded strings.
- Frontend diagnostic objects use `camelCase`; Rust diagnostic fields use
  `snake_case`. Preserve protocol field names when logging a safe protocol fact.
- Avoid redundant fields already supplied by the sink (timestamps, thread IDs).

Do not log account identifiers, user identifiers, device identifiers, session
titles, project names, or other human identity in ordinary logs.

## Sensitive Data

### Prohibited everywhere

Never record these values, even in `TRACE` or a scoped diagnostic:

- API keys, access or refresh tokens, passwords, cookies, authorization headers,
  session keys, private keys, signing material, certificates, pairing secrets,
  delegated credentials, or secret-bearing environment variables.
- Credential prefixes, suffixes, reversible encodings, or stable hashes that
  allow the original secret to be correlated.

Replace the entire value with a fixed marker such as `[redacted]`. Keeping the
first or last characters is not redaction.

### Prohibited in ordinary operational logs and safe telemetry

Do not place the following in ordinary logs or telemetry sinks:

- User prompts, system prompts, assistant responses, reasoning text, or chat
  transcripts.
- Model request or response bodies, SSE frames, token deltas, tool arguments or
  results, MCP payloads, hook payloads, or full event objects.
- File content, diffs, snapshots, terminal input or output, clipboard content,
  DOM or WebView payloads, and serialized application state.
- Absolute paths, repository or workspace names, branch names, URLs with query
  data, hostnames, IP addresses, email addresses, and device names (unless a
  dedicated, explicitly enabled diagnostic artifact documents the capture).
- Raw third-party errors or stack traces that may echo any of the values above.

When a scoped local diagnostic genuinely requires private content, it must have
its own explicit user-facing switch, default to off, state what it captures,
write to a dedicated bounded artifact, and never upload automatically. Secrets
remain prohibited. Exporting or sharing that artifact is a separate user action.

### Safe alternatives

Prefer finite classes, booleans, counts, sizes, duration, outcome, retryability,
and presence flags. For example, record `payload_bytes=2048` and
`error_type=schema` rather than the payload or parser error text.

Redaction is defense in depth, not permission to log unsafe data.

## Error Ownership

An error should normally be logged once by the layer that owns its terminal
outcome.

- A lower layer that returns an error unchanged should not also log it.
- A lower layer may log when it retries, recovers, drops work, substitutes a
  fallback, or deliberately suppresses the error.
- A boundary that converts an internal failure into a user-visible or protocol
  result may log one safe terminal summary.
- Do not blindly format Rust `Display` values, JavaScript `Error` objects, HTTP
  bodies, provider errors, or subprocess output. Map them to safe typed fields
  first.
- Stack traces belong in an explicitly controlled crash or diagnostic artifact,
  not in normal release logs.

## High-Frequency Paths

The following are aggregate-only sources for operational logging:

- Model stream chunks, SSE frames, token deltas, reasoning deltas, and partial
  tool arguments.
- Terminal read/write chunks and subprocess stdout or stderr.
- File-watch events, search matches, LSP diagnostics, and progress events.
- Event buses, queues, routers, transport fanout, and frontend store subscriptions.
- Mouse, pointer, scroll, resize, animation-frame, render, and layout callbacks.
- Polling, heartbeat, presence, synchronization, Git refresh, and reconnect
  iterations.

Do not emit one operational log per item. Instead:

1. Aggregate at the owning operation or time window.
2. Emit a terminal summary containing counts, duration, outcome, and dropped or
   suppressed work.
3. Emit only state transitions rather than every check that observes the same
   state.
4. Rate-limit repeated `WARN` and `ERROR` messages. Preserve the first event and
   later emit a summary with `suppressed_count`.
5. Check the level or diagnostic switch before cloning, formatting, serializing,
   collecting a stack, or building a diagnostic object.
6. Keep queues, batch size, entry size, file size, and retention bounded.
7. Never perform synchronous file or network IO on an interaction, render,
   streaming, terminal, or event-routing hot path.

## Telemetry boundary (when a sink exists)

If this repository wires a remote telemetry or metrics sink, accept only
registered, typed facts. Do not bridge local `log` / `tracing` / frontend logger
output wholesale into a remote exporter. Do not invent arbitrary attribute names,
message bodies, or JSON records as telemetry.

Local optional telemetry files (for example edit-constraint guard traces) remain
opt-in, bounded, and must follow the sensitive-data rules above.

## Review Checklist

Before merging logging changes:

1. English-only messages, no emojis.
2. Correct channel: local operational log vs scoped diagnostic vs user-facing
   status.
3. No secrets, prompts, tool payloads, file/terminal content, or identity fields
   in ordinary logs.
4. One terminal error log from the owning layer.
5. High-frequency paths aggregate; no per-chunk operational spam.
6. Surface API usage matches [`src/web-ui/LOGGING.md`](../../src/web-ui/LOGGING.md)
   or [`src/crates/LOGGING.md`](../../src/crates/LOGGING.md).
