# Frontend Logging Specification

## Rules

1. **Use English only** - All log messages must be in English
2. **No emojis** - Do not use emojis in log messages
3. **Structured logging** - Use data objects instead of string concatenation
4. **Avoid verbose logging** - Keep log statements concise and meaningful, avoid excessive logging in normal operation paths

## Log Levels

| Level | Value | Usage |
|-------|-------|-------|
| TRACE | 0 | Verbose diagnostic info, performance-sensitive paths |
| DEBUG | 1 | Development debugging, internal state |
| INFO | 2 | General operational info (default in dev) |
| WARN | 3 | Potential issues, degraded functionality (default in prod) |
| ERROR | 4 | Failures, exceptions, requires attention |

## Usage

```typescript
import { createLogger, logger } from '@/shared/utils/logger';

// Create scoped logger (recommended)
const log = createLogger('ModuleName');
log.info('Operation completed');
log.info('Loaded items', { count: 10 });

// Logging errors with context data
log.error('Failed to load config', { configPath, error });

// Or use global logger with context string
logger.info('Message', 'Context', { data });
```

## Message Format

```
[context] message data
```

- `context`: Component or module name in brackets
- `message`: Descriptive text, no trailing punctuation
- `data`: Optional structured data (objects auto-serialized)

## Error Handling

Error objects in data are automatically extracted and appended separately from JSON:

```typescript
// Input
log.error('Failed to load', { location: 'initializeApp', error });

// Output format
[App] Failed to load {"location":"initializeApp"}, Error: Something went wrong
    at initializeApp (file.ts:143:13)
```

- Regular data is serialized as JSON
- Error stack traces are appended after the JSON, separated by comma
- Only first-level Error objects in data are processed

Examples:
```
[WorkspaceManager] Loading workspace configuration
[LspService] Failed to start server {"code":"ENOENT"}, Error: spawn node ENOENT
    at ChildProcess._handle.onexit (...)
```

## Guidelines

1. Use `createLogger('ModuleName')` at file top, matching component/service name
2. Prefer structured data over string interpolation: `log.info('Loaded items', { count })`
3. Include Error objects in data object: `log.error('Request failed', { requestId, error })`
4. Avoid logging sensitive data (tokens, passwords, PII)
5. Avoid excessive logging in hot paths (loops, frequent callbacks)
6. Use TRACE for expensive computations that may impact performance

## Timing And Probes

Use `src/web-ui/src/shared/utils/timing.ts` as the single timing helper for frontend diagnostics.

```typescript
import { createLogger } from '@/shared/utils/logger';
import { measureAsyncAndLog, sendDebugProbe } from '@/shared/utils';

const log = createLogger('ModuleName');

await measureAsyncAndLog(log, 'Workspace loaded', () => loadWorkspace(), {
  data: { workspacePath },
});

sendDebugProbe('ModuleName.ts:42', 'Workspace refresh completed', { workspacePath }, {
  startedAt,
});
```

Rules:

1. Prefer `measureSync`, `measureAsync`, `measureSyncAndLog`, `measureAsyncAndLog`, `logDuration`, and `logElapsed` over handwritten `performance.now()` / `Date.now()` timing logs
2. Use `durationMs` for frontend diagnostic log fields
3. Treat `sendDebugProbe` as a thin wrapper over the shared logger/timing helpers, not as a separate logging system
4. Do not replace protocol or persisted fields such as `duration_ms` when they are part of API payloads, events, or stored data
5. Do not migrate animation, polling, or deadline logic that depends on raw clock semantics into the logging helper layer
