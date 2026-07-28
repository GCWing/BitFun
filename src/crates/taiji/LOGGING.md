# Taiji Crate Logging Specification

## Rules

1. **Use English only** - All log messages must be in English
2. **No emojis** - Do not use emojis in log messages
3. **Include context** - Log messages should include relevant key-value context

## Log Levels

| Level | Usage |
|-------|-------|
| ERROR | User-visible failures requiring attention (config errors, data source down, upload failures) |
| WARN  | Recoverable issues, degraded functionality (auth expiry, retry, missing optional file) |
| INFO  | State changes and operational milestones (pipeline start/stop, bar generation, publish progress) |
| DEBUG | Development debugging, internal state dumps, fine-grained execution traces |

## Format

```
[{LEVEL}] {module}: {message}
```

Examples:
```
[ERROR] biliup: biliup execution failed: No such file or directory (os error 2)
[WARN]  social_auto: Cookie expired, please re-login
[INFO]  composer: FFmpeg compose finished: output/final.mp4
[DEBUG] bar_gen: Bucket boundary crossed, closing bar freq=5m dt=2026-07-21T09:35:00+00:00
```

## Guidelines

1. Use `tracing::error!`, `tracing::warn!`, `tracing::info!`, `tracing::debug!` macros or the `taiji-engine::log::Logger` facade.
2. ERROR is for failures that need operator attention.
3. WARN is for transient or recoverable conditions.
4. INFO is for key state transitions; avoid verbose per-tick INFO logging.
5. DEBUG is for development diagnostics only; never ship DEBUG-heavy paths to production.
6. Include relevant fields: instrument, freq, platform, path, error source.
7. Never log sensitive data (API keys, tokens, cookies, passwords).
8. Closed-source crates (taiji-dvmi, taiji-magnet, taiji-thrust, taiji-risk) may use Chinese log messages internally; this spec applies to open-source crates (taiji-engine, taiji-bar, taiji-publisher, taiji-content).
