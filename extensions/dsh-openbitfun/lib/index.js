// dsh-openbitfun: a DeepSeek Harness (dsh) plugin bundle that exposes the OpenBitFun
// Agent SDK Host — a standalone `openbitfun-sdk-host` process speaking
// newline-delimited JSON-RPC 2.0 over stdio — as dsh tools.
//
// The package is dependency-free on purpose: the tool definitions are built in
// the raw JSON Schema subset the dsh tools registry accepts, so the profile
// never needs a second physical copy of @deepseek-ai/dsh-tools or
// @deepseek-ai/schemastery beside the one the running dsh installation owns.
//
// Host binary resolution order per call:
//   1. `hostPath` in this plugin's patch config,
//   2. the OPENBITFUN_SDK_HOST environment variable,
//   3. `<session cwd>/target/{debug,release}/openbitfun-sdk-host(.exe)` walking up
//      the directory tree (a OpenBitFun checkout-local build),
//   4. `openbitfun-sdk-host` on PATH.

import { spawn } from 'node:child_process';
import { existsSync, statSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';

export const name = 'openbitfun';
export const inject = ['tools', 'systemPrompt'];

const VERSION = '0.1.0';
const PROTOCOL_VERSION = 1;
const HOST_BINARY_NAME = 'openbitfun-sdk-host';
const HOST_BINARY_BASE = process.platform === 'win32' ? `${HOST_BINARY_NAME}.exe` : HOST_BINARY_NAME;
const BOOT_REQUEST_TIMEOUT_MS = 120_000;
const DEFAULT_REQUEST_TIMEOUT_MS = 30_000;
const DEFAULT_QUERY_TIMEOUT_MS = 30 * 60 * 1000;
const DEFAULT_MAX_SESSIONS = 16;
const SHUTDOWN_GRACE_MS = 5_000;
const MAX_STDERR_TAIL = 8_000;

function isNonEmptyString(value) {
  return typeof value === 'string' && value.trim().length > 0;
}

function positiveInteger(value, fallback) {
  return typeof value === 'number' && Number.isFinite(value) && Number.isInteger(value) && value > 0
    ? value
    : fallback;
}

class HostError extends Error {
  constructor(message, code, cause, rpcData) {
    super(message, cause === undefined ? undefined : { cause });
    this.name = 'OpenBitFunHostError';
    this.code = code;
    this.rpcData = rpcData;
  }
}

function abortError() {
  const error = new Error('tool call aborted');
  error.name = 'AbortError';
  return error;
}

function isPathLike(value) {
  return value.includes('/') || value.includes('\\');
}

function findOnPath(binaryName) {
  const extensions = process.platform === 'win32' ? ['.exe', ''] : [''];
  for (const entry of (process.env.PATH ?? '').split(path.delimiter)) {
    if (!isNonEmptyString(entry)) continue;
    for (const extension of extensions) {
      const candidate = path.join(entry, `${binaryName}${extension}`);
      try {
        if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
      } catch {
        // unreadable PATH entry: keep looking
      }
    }
  }
  return null;
}

/**
 * Resolve the SDK Host binary for a call whose workspace root is `cwd`.
 * Returns { spawnSpec } or null when nothing usable was found.
 */
function resolveHostPath(config, cwd) {
  const candidates = [];
  const configured = isNonEmptyString(config?.hostPath) ? config.hostPath : undefined;
  const fromEnv = isNonEmptyString(process.env.OPENBITFUN_SDK_HOST) ? process.env.OPENBITFUN_SDK_HOST : undefined;
  for (const candidate of [configured, fromEnv]) {
    if (candidate === undefined) continue;
    if (isPathLike(candidate)) {
      candidates.push(candidate);
    } else {
      const found = findOnPath(candidate);
      if (found !== null) candidates.push(found);
      candidates.push(candidate); // keep as a last-resort spawn spec
    }
  }
  let dir = path.resolve(cwd);
  for (;;) {
    candidates.push(path.join(dir, 'target', 'debug', HOST_BINARY_BASE));
    candidates.push(path.join(dir, 'target', 'release', HOST_BINARY_BASE));
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  candidates.push(HOST_BINARY_BASE);

  const tried = new Set();
  for (const candidate of candidates) {
    if (tried.has(candidate)) continue;
    tried.add(candidate);
    if (isPathLike(candidate)) {
      try {
        if (existsSync(candidate) && statSync(candidate).isFile()) return { spawnSpec: candidate };
      } catch {
        // fall through
      }
      continue;
    }
    const found = findOnPath(candidate);
    if (found !== null) return { spawnSpec: found };
  }
  return null;
}

/**
 * One connection to a `openbitfun-sdk-host` process: newline-delimited JSON-RPC
 * request/response plus `query/event` and `query/result` notifications.
 * The client respawns lazily after an unexpected process exit.
 */
class SdkHostClient {
  constructor(spawnSpec, options) {
    this.spawnSpec = spawnSpec;
    this.requestTimeoutMs = options.requestTimeoutMs;
    this.child = null;
    this.readyPromise = null;
    this.disposed = false;
    this.nextId = 1;
    this.pending = new Map();
    this.queries = new Map();
    this.buffer = '';
    this.stderrTail = '';
  }

  stderrSuffix() {
    const tail = this.stderrTail.trim();
    return tail === '' ? '' : `\nOpenBitFun SDK Host stderr tail:\n${tail}`;
  }

  async ensureReady() {
    if (this.disposed) throw new Error('the OpenBitFun SDK Host bridge is disposed');
    if (this.readyPromise !== null) return this.readyPromise;
    this.readyPromise = this.#boot().catch((error) => {
      this.readyPromise = null;
      throw error;
    });
    return this.readyPromise;
  }

  async #boot() {
    const child = spawn(this.spawnSpec, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
      windowsHide: true,
    });
    this.child = child;
    this.buffer = '';
    this.stderrTail = '';
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      this.buffer += chunk;
      this.#drain();
    });
    child.stderr.on('data', (chunk) => {
      this.stderrTail = (this.stderrTail + chunk).slice(-MAX_STDERR_TAIL);
    });

    let booted = false;
    const exitDeferred = deferred();
    const spawnErrorDeferred = deferred();
    child.once('exit', (code, signal) => {
      this.child = null;
      this.readyPromise = null;
      const message =
        `OpenBitFun SDK Host process exited unexpectedly (code ${code ?? 'null'}, signal ${signal ?? 'null'})` +
        this.stderrSuffix();
      const error = new HostError(message, 'PROCESS_LOST');
      if (!booted) exitDeferred.reject(error);
      for (const entry of this.pending.values()) entry.reject(error);
      this.pending.clear();
      for (const query of this.queries.values()) query.onLost(error);
      this.queries.clear();
      this.buffer = '';
    });
    child.once('error', (error) => {
      this.readyPromise = null;
      spawnErrorDeferred.reject(
        new HostError(`cannot start OpenBitFun SDK Host "${this.spawnSpec}": ${error.message}`, 'SPAWN_FAILED', error),
      );
    });

    try {
      const result = await Promise.race([
        this.#requestRaw(
          'initialize',
          {
            protocolVersion: PROTOCOL_VERSION,
            clientInfo: { name: 'dsh-openbitfun', version: VERSION },
            capabilities: { serverNotifications: true },
          },
          BOOT_REQUEST_TIMEOUT_MS,
        ),
        exitDeferred.promise,
        spawnErrorDeferred.promise,
      ]);
      booted = true;
      return result;
    } catch (error) {
      try {
        child.kill();
      } catch {
        // process already gone
      }
      throw error;
    }
  }

  #requestRaw(method, params, timeoutMs = this.requestTimeoutMs) {
    const id = this.nextId++;
    const pending = deferred();
    const timer = setTimeout(() => {
      this.pending.delete(id);
      pending.reject(new HostError(`OpenBitFun SDK Host request timed out: ${method} after ${timeoutMs}ms`, 'TIMEOUT'));
    }, timeoutMs);
    this.pending.set(id, {
      resolve: (value) => {
        clearTimeout(timer);
        pending.resolve(value);
      },
      reject: (error) => {
        clearTimeout(timer);
        pending.reject(error);
      },
    });
    const child = this.child;
    if (child === null || child.stdin.destroyed) {
      this.pending.delete(id);
      clearTimeout(timer);
      pending.reject(new HostError('the OpenBitFun SDK Host is not running', 'NOT_RUNNING'));
      return pending.promise;
    }
    child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params: params ?? {} })}\n`, (error) => {
      if (error === null || error === undefined) return;
      if (this.pending.delete(id)) {
        clearTimeout(timer);
        pending.reject(
          new HostError(`cannot write to OpenBitFun SDK Host stdin: ${error.message}`, 'WRITE_FAILED', error),
        );
      }
    });
    return pending.promise;
  }

  #drain() {
    let index;
    while ((index = this.buffer.indexOf('\n')) !== -1) {
      const line = this.buffer.slice(0, index);
      this.buffer = this.buffer.slice(index + 1);
      if (line.trim() === '') continue;
      let message;
      try {
        message = JSON.parse(line);
      } catch {
        continue;
      }
      this.#dispatch(message);
    }
  }

  #dispatch(message) {
    if (typeof message !== 'object' || message === null) return;
    if (typeof message.method === 'string') {
      if (message.id === undefined || message.id === null) this.#dispatchNotification(message);
      return; // server-to-client requests are not part of this protocol version
    }
    const entry = this.pending.get(message.id);
    if (entry === undefined) return;
    this.pending.delete(message.id);
    if (message.error !== undefined && message.error !== null) {
      const data = typeof message.error === 'object' && message.error !== null ? message.error.data : undefined;
      entry.reject(
        new HostError(
          `OpenBitFun SDK Host error: ${String(message.error?.message ?? 'request failed')}`,
          'RPC_ERROR',
          undefined,
          data,
        ),
      );
    } else {
      entry.resolve(message.result);
    }
  }

  #dispatchNotification(message) {
    const params = typeof message.params === 'object' && message.params !== null ? message.params : {};
    if (message.method === 'query/event') {
      const query = this.queries.get(params.queryId);
      if (query !== undefined) query.onEvent(params.event, params.sequence);
    } else if (message.method === 'query/result') {
      const query = this.queries.get(params.queryId);
      if (query !== undefined) {
        this.queries.delete(params.queryId);
        query.onTerminal(params.status, params.error);
      }
    }
  }

  async request(method, params) {
    await this.ensureReady();
    return this.#requestRaw(method, params);
  }

  /**
   * Start one OpenBitFun turn and wait for its terminal `query/result`.
   * Resolves with { status, text, error } where status is
   * completed | failed | cancelled.
   */
  async startQuery(params) {
    const started = await this.request('query/start', params);
    if (typeof started?.queryId !== 'string' || typeof started?.sessionId !== 'string') {
      throw new HostError('OpenBitFun SDK Host returned an invalid query/start response', 'PROTOCOL');
    }
    const terminal = deferred();
    let settled = false;
    let text = '';
    const settle = (outcome) => {
      if (settled) return;
      settled = true;
      this.queries.delete(started.queryId);
      terminal.resolve(outcome);
    };
    this.queries.set(started.queryId, {
      onEvent: (event) => {
        if (
          event !== null &&
          typeof event === 'object' &&
          event.type === 'assistant_text_delta' &&
          typeof event.text === 'string'
        ) {
          text += event.text;
        }
      },
      onTerminal: (status, error) => {
        settle({
          status: typeof status === 'string' ? status : 'failed',
          text,
          error:
            error !== null && typeof error === 'object' && typeof error.message === 'string'
              ? error.message
              : undefined,
        });
      },
      onLost: (hostError) => {
        settle({ status: 'failed', text, error: hostError.message });
      },
    });
    return { ...started, result: terminal.promise };
  }

  async cancelQuery(queryId) {
    try {
      await this.request('query/cancel', { queryId });
    } catch {
      // best effort; the query/result notification settles the turn either way
    }
  }

  async closeSession(sessionId) {
    try {
      await this.request('session/close', { sessionId });
    } catch {
      // best effort
    }
  }

  async dispose() {
    this.disposed = true;
    const child = this.child;
    if (child === null || child.exitCode !== null) return;
    try {
      await Promise.race([this.#requestRaw('shutdown', {}).catch(() => undefined), sleep(SHUTDOWN_GRACE_MS)]);
    } catch {
      // fall through to kill
    }
    try {
      child.kill();
    } catch {
      // process already gone
    }
  }
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function truncate(value, maxLength) {
  const text = String(value);
  return text.length <= maxLength ? text : `${text.slice(0, maxLength)}...`;
}

const RUN_TOOL_PARAMETERS = {
  type: 'object',
  properties: {
    prompt: {
      type: 'string',
      description:
        'The task for the OpenBitFun agent. OpenBitFun runs it autonomously with its own agent loop and returns its final text answer.',
    },
    cwd: {
      type: 'string',
      description:
        'Optional absolute workspace directory for the OpenBitFun session. Defaults to this dsh session cwd.',
    },
    session_id: {
      type: 'string',
      description:
        'Optional OpenBitFun session id returned by a previous openbitfun_run call. Pass it to continue that OpenBitFun conversation instead of starting a new one.',
    },
    agent: {
      type: 'string',
      description: 'Optional OpenBitFun agent type for a new session (default: OpenBitFun built-in default).',
    },
    model: {
      type: 'string',
      description:
        'Optional OpenBitFun model id for a new session (default: the user configured OpenBitFun model).',
    },
  },
  required: ['prompt'],
};

const RUN_TOOL_OUTPUT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    status: { type: 'string', enum: ['completed', 'failed', 'cancelled'] },
    text: { type: 'string' },
    session_id: { type: 'string' },
    query_id: { type: 'string' },
    turn_id: { type: 'string' },
    error: { type: 'string' },
  },
  required: ['status', 'text', 'session_id', 'query_id', 'turn_id'],
};

function renderRunOutput(_args, value) {
  const text = value.status === 'completed'
    ? value.text
    : `OpenBitFun turn ended with status "${value.status}".\n` +
      `${value.error !== undefined ? `Error: ${value.error}\n` : ''}` +
      `Partial answer:\n${value.text}`;
  return [{ type: 'text', text }];
}

function isStaleSessionError(error) {
  return (
    error instanceof HostError &&
    (error.rpcData?.code === 'NotFound' || /must belong to the same SDK Host connection/i.test(error.message))
  );
}

/**
 * Register the `openbitfun_run` tool plus a short system-prompt section.
 * One SDK Host client is kept per resolved binary; OpenBitFun sessions are
 * transient per Host connection, so the plugin maps each dsh session to one
 * OpenBitFun session and closes the oldest mappings past `maxSessions`.
 */
export function apply(ctx, config) {
  const options = {
    hostPath: isNonEmptyString(config?.hostPath) ? config.hostPath : undefined,
    maxSessions: positiveInteger(config?.maxSessions, DEFAULT_MAX_SESSIONS),
    requestTimeoutMs: positiveInteger(config?.requestTimeoutMs, DEFAULT_REQUEST_TIMEOUT_MS),
    queryTimeoutMs: positiveInteger(config?.queryTimeoutMs, DEFAULT_QUERY_TIMEOUT_MS),
  };
  const clients = new Map(); // spawnSpec -> SdkHostClient
  const sessions = new Map(); // dsh session key -> { sessionId, client, lastUsed }

  function ensureClient(cwd) {
    const resolved = resolveHostPath(options, cwd);
    if (resolved === null) {
      throw new Error(
        'cannot locate the OpenBitFun SDK Host binary ("openbitfun-sdk-host"). ' +
          'Build it with `cargo build -p openbitfun-sdk-host-app`, set the OPENBITFUN_SDK_HOST environment variable, ' +
          'or configure the hostPath option of the dsh-openbitfun plugin row.',
      );
    }
    let client = clients.get(resolved.spawnSpec);
    if (client === undefined) {
      client = new SdkHostClient(resolved.spawnSpec, { requestTimeoutMs: options.requestTimeoutMs });
      clients.set(resolved.spawnSpec, client);
    }
    return client;
  }

  function sessionKeyFor(exec, cwd) {
    const id = exec?.agent?.session?.header?.id;
    return typeof id === 'string' && id !== '' ? id : `cwd:${cwd}`;
  }

  function evictSessions() {
    while (sessions.size > options.maxSessions) {
      let oldestKey;
      let oldestAt = Infinity;
      for (const [key, entry] of sessions) {
        if (entry.lastUsed < oldestAt) {
          oldestAt = entry.lastUsed;
          oldestKey = key;
        }
      }
      if (oldestKey === undefined) break;
      const entry = sessions.get(oldestKey);
      sessions.delete(oldestKey);
      if (entry !== undefined) void entry.client.closeSession(entry.sessionId);
    }
  }

  const openbitfunRunTool = {
    name: 'openbitfun_run',
    description:
      'Delegate a task to the OpenBitFun agent runtime: OpenBitFun executes the prompt in a workspace with its own ' +
      'agent loop and tools, then returns its final text answer. The result includes a session_id; pass it in a ' +
      'later call to continue the same OpenBitFun conversation. OpenBitFun interactive permission prompts are auto-rejected ' +
      'by the SDK host, so use this for tasks that do not require interactive approval.',
    parameters: RUN_TOOL_PARAMETERS,
    output: {
      schema: RUN_TOOL_OUTPUT_SCHEMA,
      render: renderRunOutput,
    },
    timeoutMs: options.queryTimeoutMs,
    isConcurrencySafe: () => false,
    async execute(args, exec) {
      if (!isNonEmptyString(args.prompt)) throw new Error('prompt must be a non-empty string');
      const prompt = args.prompt.trim();
      const sessionCwd = exec?.agent?.session?.header?.cwd;
      let cwd = isNonEmptyString(args.cwd) ? args.cwd : sessionCwd;
      if (!isNonEmptyString(cwd)) cwd = process.cwd();
      if (!path.isAbsolute(cwd)) throw new Error(`cwd must be an absolute path, got "${cwd}"`);

      const client = ensureClient(cwd);
      const key = sessionKeyFor(exec, cwd);

      let sessionId = isNonEmptyString(args.session_id) ? args.session_id : undefined;
      let mapped = false;
      if (sessionId === undefined) {
        const existing = sessions.get(key);
        if (existing !== undefined) {
          sessionId = existing.sessionId;
          existing.lastUsed = Date.now();
          mapped = true;
        }
      }

      const createSession = async () => {
        const createParams = { cwd };
        if (isNonEmptyString(args.agent)) createParams.agent = args.agent;
        if (isNonEmptyString(args.model)) createParams.model = args.model;
        const created = await client.request('session/create', createParams);
        if (typeof created?.sessionId !== 'string' || created.sessionId === '') {
          throw new HostError('OpenBitFun SDK Host returned an invalid session/create response', 'PROTOCOL');
        }
        sessions.set(key, { sessionId: created.sessionId, client, lastUsed: Date.now() });
        evictSessions();
        return created.sessionId;
      };

      if (sessionId === undefined) sessionId = await createSession();

      let started;
      try {
        started = await client.startQuery({ prompt, sessionId });
      } catch (error) {
        if (!mapped || !isStaleSessionError(error)) throw error;
        // The host connection restarted (or evicted this session); drop the
        // stale mapping and retry once with a fresh OpenBitFun session.
        sessions.delete(key);
        sessionId = await createSession();
        started = await client.startQuery({ prompt, sessionId });
      }

      const signal = exec?.signal;
      const onAbort = () => {
        void client.cancelQuery(started.queryId);
      };
      let removeAbort = () => {};
      if (signal instanceof AbortSignal) {
        if (signal.aborted) {
          onAbort();
          throw abortError();
        }
        signal.addEventListener('abort', onAbort, { once: true });
        removeAbort = () => signal.removeEventListener('abort', onAbort);
      }

      try {
        const outcome = await started.result;
        if (signal instanceof AbortSignal && signal.aborted) throw abortError();
        return {
          status: outcome.status,
          text: outcome.text,
          session_id: started.sessionId,
          query_id: started.queryId,
          turn_id: started.turnId,
          ...(outcome.error !== undefined ? { error: outcome.error } : {}),
        };
      } finally {
        removeAbort();
      }
    },
    presentCall(args) {
      return {
        card: 'generic',
        title: `OpenBitFun: ${truncate(args.prompt, 80)}`,
        kind: 'tool',
      };
    },
    presentResult() {
      return { card: 'generic', title: 'OpenBitFun', kind: 'tool' };
    },
  };

  ctx.tools.register(openbitfunRunTool);

  ctx.systemPrompt.section({
    name: 'tool:openbitfun',
    order: 200,
    text:
      'Use the openbitfun_run tool to delegate a task to the OpenBitFun agent runtime when the user asks to use OpenBitFun, ' +
      'or when handing a whole subtask to OpenBitFun is appropriate. OpenBitFun runs the prompt autonomously in the ' +
      'workspace and returns its final text answer; pass the returned session_id back to continue that OpenBitFun ' +
      'conversation across calls. Do not use it for tasks that need interactive permission approval, because the ' +
      'SDK host auto-rejects OpenBitFun permission prompts.',
  });

  ctx.on('dispose', () => {
    for (const client of clients.values()) void client.dispose();
    clients.clear();
    sessions.clear();
  });
}
