# Backend Logging Specification

## Rules

1. **Use English only** - All log messages must be in English
2. **No emojis** - Do not use emojis in log messages
3. **Structured logging** - Include relevant context and metadata in log messages using formatted strings with key-value information
4. **Avoid verbose logging** - Keep log statements concise and meaningful, avoid excessive logging in normal operation paths

## Log Levels

| Level | Value | Usage |
|-------|-------|-------|
| TRACE | 0 | Verbose diagnostic info, performance-sensitive paths |
| DEBUG | 1 | Development debugging, internal state |
| INFO | 2 | General operational info (default in dev) |
| WARN | 3 | Potential issues, degraded functionality (default in prod) |
| ERROR | 4 | Failures, exceptions, requires attention |

## Guidelines

1. Import log macros at the top of the file: `use log::{info, debug, warn, error, trace};`
2. Include relevant context in log messages using formatted strings: `info!("Registered {} adapter for session: {}", adapter_type, session_id)`
3. Pass Error objects using Display formatting: `error!("Failed to emit event for session {}: {}", session_id, e)`
4. Avoid logging sensitive data (tokens, passwords, PII, API keys)
5. Avoid excessive logging in hot paths (loops, frequent callbacks, tight loops)
6. Use TRACE for expensive computations that may impact performance
7. Include relevant context fields (session_id, request_id, user_id, operation) when available
8. Use appropriate log levels - reserve ERROR for actual failures, not expected error conditions
9. Keep log messages concise and actionable - focus on what happened and why it matters
10. Use conditional logging for expensive operations: `if log::log_enabled!(log::Level::Debug) { ... }`

## Timing And Duration Fields

Use shared timing helpers from `bitfun_core::util::timing` when recording internal durations.

```rust
use bitfun_core::util::{elapsed_ms_u64, TimingCollector};
use std::time::Instant;

let started_at = Instant::now();
let duration_ms = elapsed_ms_u64(started_at);
debug!("Git status completed: repo_path={}, duration_ms={}", repo_path, duration_ms);
```

Rules:

1. Prefer `elapsed_ms`, `elapsed_ms_u64`, and `TimingCollector` over repeated `Instant::now()` plus `elapsed().as_millis()` formatting
2. Use `duration_ms` for Rust diagnostic log keys
3. Preserve existing protocol and model field names such as `duration_ms`, `execution_time_ms`, or `response_time_ms` when they are part of events, API responses, or persisted state
4. Avoid introducing timing logs into tight loops or high-frequency runtime paths unless the diagnostic value clearly justifies it
