// Protocol smoke test for the dsh-openbitfun bridge: drives a real
// `openbitfun-sdk-host` process through initialize -> session/create ->
// query/start -> query/event* -> query/result and asserts a completed turn
// with non-empty assistant text. No dsh is required.
//
// Usage: node test/smoke.mjs [host-binary]

import { spawn } from 'node:child_process';
import { existsSync, statSync } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const PACKAGE_ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const HOST_BINARY_BASE = process.platform === 'win32' ? 'openbitfun-sdk-host.exe' : 'openbitfun-sdk-host';

function resolveHost() {
  const explicit = process.argv[2] ?? process.env.OPENBITFUN_SDK_HOST;
  if (explicit !== undefined) return explicit;
  for (let dir = PACKAGE_ROOT; ; dir = path.dirname(dir)) {
    for (const profile of ['debug', 'release']) {
      const candidate = path.join(dir, 'target', profile, HOST_BINARY_BASE);
      try {
        if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
      } catch {
        // keep looking
      }
    }
    const parent = path.dirname(dir);
    if (parent === dir) break;
  }
  return HOST_BINARY_BASE;
}

function fail(message) {
  console.error(`smoke failed: ${message}`);
  process.exit(1);
}

const hostPath = resolveHost();
console.log(`using SDK Host binary: ${hostPath}`);

const child = spawn(hostPath, [], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
let buffer = '';
let stderrTail = '';
const pending = new Map();
const queries = new Map();
let nextId = 1;

child.stdout.setEncoding('utf8');
child.stderr.setEncoding('utf8');
child.stdout.on('data', (chunk) => {
  buffer += chunk;
  drain();
});
child.stderr.on('data', (chunk) => {
  stderrTail = (stderrTail + chunk).slice(-4000);
});
child.on('error', (error) => fail(`cannot start ${hostPath}: ${error.message}`));
child.on('exit', (code, signal) => {
  fail(`host exited (code ${code ?? 'null'}, signal ${signal ?? 'null'})\nstderr tail:\n${stderrTail}`);
});

function drain() {
  let index;
  while ((index = buffer.indexOf('\n')) !== -1) {
    const line = buffer.slice(0, index);
    buffer = buffer.slice(index + 1);
    if (line.trim() === '') continue;
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      continue;
    }
    dispatch(message);
  }
}

function dispatch(message) {
  if (typeof message !== 'object' || message === null) return;
  if (typeof message.method === 'string') {
    if (message.id === undefined || message.id === null) {
      const params = typeof message.params === 'object' && message.params !== null ? message.params : {};
      if (message.method === 'query/event') {
        const query = queries.get(params.queryId);
        if (query !== undefined) query.onEvent(params.event);
      } else if (message.method === 'query/result') {
        const query = queries.get(params.queryId);
        if (query !== undefined) {
          queries.delete(params.queryId);
          query.onResult(params.status, params.error);
        }
      }
    }
    return;
  }
  const entry = pending.get(message.id);
  if (entry === undefined) return;
  pending.delete(message.id);
  if (message.error !== undefined && message.error !== null) {
    entry.reject(new Error(`rpc error: ${JSON.stringify(message.error)}`));
  } else {
    entry.resolve(message.result);
  }
}

function request(method, params, timeoutMs = 60_000) {
  const id = nextId++;
  const result = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`request timed out: ${method}`));
    }, timeoutMs);
    pending.set(id, {
      resolve: (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      reject: (error) => {
        clearTimeout(timer);
        reject(error);
      },
    });
  });
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params: params ?? {} })}\n`);
  return result;
}

try {
  const init = await request('initialize', {
    protocolVersion: 1,
    clientInfo: { name: 'dsh-openbitfun-smoke', version: '0.1.0' },
    capabilities: { serverNotifications: true },
  });
  console.log(`initialized: protocol ${init.protocolVersion}, runtime ${init.runtimeVersion}`);

  const created = await request('session/create', { sessionName: 'dsh-openbitfun smoke', cwd: PACKAGE_ROOT });
  console.log(`session created: ${created.sessionId} (${created.agent})`);

  const started = await request('query/start', { prompt: 'Reply with exactly the word OK.', sessionId: created.sessionId });
  console.log(`query started: ${started.queryId} on turn ${started.turnId}`);

  const outcome = await new Promise((resolve) => {
    let text = '';
    let settled = false;
    const settle = (value) => {
      if (settled) return;
      settled = true;
      resolve(value);
    };
    queries.set(started.queryId, {
      onEvent: (event) => {
        if (event !== null && typeof event === 'object' && event.type === 'assistant_text_delta' && typeof event.text === 'string') {
          text += event.text;
        }
      },
      onResult: (status, error) => {
        settle({ status, text, error });
      },
    });
  });
  console.log(`query result: status=${outcome.status}, text=${JSON.stringify(outcome.text)}`);
  if (outcome.status !== 'completed') fail(`expected completed, got ${outcome.status}: ${JSON.stringify(outcome.error)}`);
  if (outcome.text.trim().length === 0) fail('assistant text is empty');

  await request('session/close', { sessionId: created.sessionId });
  await request('shutdown', {});
  console.log('smoke passed');
  process.exit(0);
} catch (error) {
  fail(error?.stack ?? String(error));
}
