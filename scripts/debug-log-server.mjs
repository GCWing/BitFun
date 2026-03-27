/**
 * Debug Log Receiver Server
 * Receives POST requests from fetch-based instrumentation and writes to a log file.
 *
 * Usage:
 *   node scripts/debug-log-server.mjs [port] [logfile]
 *
 * Defaults:
 *   port    = 7469
 *   logfile = debug-agent.log (in project root)
 *
 * Frontend fetch template (copy into your code):
 *   fetch('http://127.0.0.1:7469/log', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({location:'file.ts:LINE', message:'desc', data:{k:v}, timestamp:Date.now()})}).catch(()=>{});
 */

import http from 'http';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');

const PORT = parseInt(process.argv[2] ?? '7469', 10);
const LOG_FILE = path.resolve(ROOT, process.argv[3] ?? 'debug-agent.log');

// Clear log file on start
fs.writeFileSync(LOG_FILE, '', 'utf8');
console.log(`[debug-log-server] started on http://127.0.0.1:${PORT}`);
console.log(`[debug-log-server] writing logs to ${LOG_FILE}`);
console.log(`[debug-log-server] fetch template:`);
console.log(`  fetch('http://127.0.0.1:${PORT}/log', {method:'POST', headers:{'Content-Type':'application/json'}, body: JSON.stringify({location:'file.ts:LINE', message:'desc', data:{}, timestamp:Date.now()})}).catch(()=>{});\n`);

const server = http.createServer((req, res) => {
  // CORS headers so browser can POST from any origin
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }

  if (req.method === 'POST' && req.url === '/log') {
    let body = '';
    req.on('data', chunk => { body += chunk; });
    req.on('end', () => {
      try {
        const payload = JSON.parse(body);
        const entry = JSON.stringify({ ...payload, _receivedAt: new Date().toISOString() });
        fs.appendFileSync(LOG_FILE, entry + '\n', 'utf8');

        // Pretty print to console for quick monitoring
        const loc = payload.location ?? '?';
        const msg = payload.message ?? '';
        const data = payload.data ? JSON.stringify(payload.data) : '';
        console.log(`[LOG] ${loc} | ${msg} | ${data}`);

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ ok: true }));
      } catch (e) {
        res.writeHead(400);
        res.end(JSON.stringify({ error: 'invalid json' }));
      }
    });
    return;
  }

  // /clear - clear log file
  if (req.method === 'POST' && req.url === '/clear') {
    fs.writeFileSync(LOG_FILE, '', 'utf8');
    console.log('[debug-log-server] log file cleared');
    res.writeHead(200);
    res.end(JSON.stringify({ ok: true }));
    return;
  }

  res.writeHead(404);
  res.end();
});

server.listen(PORT, '127.0.0.1', () => {
  console.log('[debug-log-server] ready\n');
});
