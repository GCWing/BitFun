#!/usr/bin/env node
/**
 * Frontend debug log receiver for the debug skill.
 *
 * Start it from any project root:
 *   node <skill_dir>/scripts/debug-log-server.mjs
 *
 * Or pass an explicit project root/log file:
 *   node <skill_dir>/scripts/debug-log-server.mjs --root C:\path\to\project
 *   node <skill_dir>/scripts/debug-log-server.mjs --log-file C:\path\to\debug-agent.log
 */

import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';

function parseArgs(argv) {
  const options = {
    host: '127.0.0.1',
    port: 7469,
    root: process.cwd(),
    logFile: null,
    clear: true,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];

    if (arg === '--help' || arg === '-h') {
      options.help = true;
    } else if (arg === '--host' && next) {
      options.host = next;
      i += 1;
    } else if (arg === '--port' && next) {
      options.port = Number.parseInt(next, 10);
      i += 1;
    } else if (arg === '--root' && next) {
      options.root = next;
      i += 1;
    } else if (arg === '--log-file' && next) {
      options.logFile = next;
      i += 1;
    } else if (arg === '--no-clear') {
      options.clear = false;
    } else if (!arg.startsWith('--') && !options._legacyPort) {
      const parsedPort = Number.parseInt(arg, 10);
      if (Number.isFinite(parsedPort)) {
        options.port = parsedPort;
        options._legacyPort = true;
      }
    } else if (!arg.startsWith('--') && !options.logFile) {
      options.logFile = arg;
    } else {
      throw new Error(`Unknown or incomplete argument: ${arg}`);
    }
  }

  if (!Number.isInteger(options.port) || options.port <= 0 || options.port > 65535) {
    throw new Error(`Invalid --port value: ${options.port}`);
  }

  options.root = path.resolve(options.root);
  options.logFile = path.resolve(options.root, options.logFile ?? 'debug-agent.log');
  return options;
}

function printHelp() {
  console.log(`Frontend debug log receiver

Usage:
  node <skill_dir>/scripts/debug-log-server.mjs [options]

Options:
  --host <host>          Bind host. Default: 127.0.0.1
  --port <port>          Bind port. Default: 7469
  --root <path>          Project root used for default log file. Default: current working directory
  --log-file <path>      Log file path. Relative paths resolve from --root. Default: debug-agent.log
  --no-clear             Do not clear the log file on startup
  -h, --help             Show this help

Legacy positional form is also supported:
  node <script> 7469 debug-agent.log
`);
}

function sendJson(res, status, payload) {
  res.writeHead(status, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(payload));
}

function requestBody(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.setEncoding('utf8');
    req.on('data', (chunk) => {
      body += chunk;
      if (body.length > 1024 * 1024) {
        reject(new Error('request body too large'));
        req.destroy();
      }
    });
    req.on('end', () => resolve(body));
    req.on('error', reject);
  });
}

const options = parseArgs(process.argv.slice(2));

if (options.help) {
  printHelp();
  process.exit(0);
}

fs.mkdirSync(path.dirname(options.logFile), { recursive: true });
if (options.clear) {
  fs.writeFileSync(options.logFile, '', 'utf8');
}

console.log(`[debug-log-server] started on http://${options.host}:${options.port}`);
console.log(`[debug-log-server] writing logs to ${options.logFile}`);
console.log('[debug-log-server] frontend snippet:');
console.log(`  fetch('http://${options.host}:${options.port}/log', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ hypothesis: 'A', location: 'file.tsx:LINE', message: 'description', data: {} }) }).catch(() => {});\n`);

const server = http.createServer(async (req, res) => {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }

  if (req.method === 'GET' && req.url === '/health') {
    sendJson(res, 200, { ok: true, logFile: options.logFile });
    return;
  }

  if (req.method === 'POST' && req.url === '/clear') {
    fs.writeFileSync(options.logFile, '', 'utf8');
    console.log('[debug-log-server] log file cleared');
    sendJson(res, 200, { ok: true });
    return;
  }

  if (req.method === 'POST' && req.url === '/log') {
    try {
      const rawBody = await requestBody(req);
      const payload = JSON.parse(rawBody || '{}');
      const entry = {
        ...payload,
        _receivedAt: new Date().toISOString(),
      };

      fs.appendFileSync(options.logFile, `${JSON.stringify(entry)}\n`, 'utf8');

      const loc = payload.location ?? '?';
      const msg = payload.message ?? '';
      const data = payload.data === undefined ? '' : JSON.stringify(payload.data);
      console.log(`[LOG] ${loc} | ${msg} | ${data}`);

      sendJson(res, 200, { ok: true });
    } catch (error) {
      sendJson(res, 400, { ok: false, error: error instanceof Error ? error.message : 'invalid request' });
    }
    return;
  }

  sendJson(res, 404, { ok: false, error: 'not found' });
});

server.listen(options.port, options.host, () => {
console.log('[debug-log-server] ready');
});
