export interface PeerHostInvokeExecutionResult {
  ok: boolean;
  value: unknown;
  error: string | null;
}

interface CachedExecution {
  expiresAt: number;
  promise: Promise<PeerHostInvokeExecutionResult>;
}

const IDEMPOTENT_HOST_INVOKE_COMMANDS = new Set([
  'start_dialog_turn',
  'start_acp_dialog_turn',
]);

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null
    ? value as Record<string, unknown>
    : null;
}

/**
 * Returns a stable key only for mutations whose execution side has an
 * explicit idempotency identity.
 */
export function peerHostInvokeIdempotencyKey(
  command: string,
  args: unknown,
): string | null {
  if (!IDEMPOTENT_HOST_INVOKE_COMMANDS.has(command)) {
    return null;
  }
  const request = asRecord(asRecord(args)?.request);
  const sessionId = typeof request?.sessionId === 'string'
    ? request.sessionId.trim()
    : '';
  const turnId = typeof request?.turnId === 'string'
    ? request.turnId.trim()
    : '';
  if (!sessionId || !turnId) {
    return null;
  }
  return `${command}:${sessionId}:${turnId}`;
}

/**
 * Coalesces concurrent retries and briefly caches the completed result. The
 * controller can therefore replay the same turn submission after an
 * ambiguous Relay failure without executing the prompt twice on the host.
 */
export class PeerHostInvokeIdempotencyCache {
  private readonly entries = new Map<string, CachedExecution>();

  constructor(
    private readonly ttlMs = 2 * 60_000,
    private readonly maxEntries = 128,
    private readonly now: () => number = Date.now,
  ) {}

  execute(
    command: string,
    args: unknown,
    operation: () => Promise<PeerHostInvokeExecutionResult>,
  ): Promise<PeerHostInvokeExecutionResult> {
    const key = peerHostInvokeIdempotencyKey(command, args);
    if (!key) {
      return operation();
    }

    this.prune();
    const cached = this.entries.get(key);
    if (cached) {
      return cached.promise;
    }

    const promise = Promise.resolve().then(operation);
    this.entries.set(key, {
      expiresAt: this.now() + this.ttlMs,
      promise,
    });
    this.enforceCapacity();
    return promise;
  }

  private prune(): void {
    const now = this.now();
    for (const [key, entry] of this.entries) {
      if (entry.expiresAt <= now) {
        this.entries.delete(key);
      }
    }
  }

  private enforceCapacity(): void {
    while (this.entries.size > this.maxEntries) {
      const oldestKey = this.entries.keys().next().value;
      if (oldestKey === undefined) {
        return;
      }
      this.entries.delete(oldestKey);
    }
  }
}
