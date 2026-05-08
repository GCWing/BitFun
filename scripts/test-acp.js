// Simple test client for BitFun ACP server.
// Run with: node scripts/test-acp.js

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const cliPath = path.join(__dirname, '..', 'target', 'debug', 'bitfun-cli');
const cliReleasePath = path.join(__dirname, '..', 'target', 'release', 'bitfun-cli');
const usePath = fs.existsSync(cliPath)
  ? cliPath
  : fs.existsSync(cliReleasePath)
    ? cliReleasePath
    : 'bitfun-cli';

const cwd = '/tmp/test-acp-node';
fs.mkdirSync(cwd, { recursive: true });

console.log('=== BitFun ACP Server Test (Node.js) ===\n');

const child = spawn(usePath, ['acp'], {
  stdio: ['pipe', 'pipe', 'inherit'],
});

let buffer = '';
let sessionId = null;

function send(request) {
  child.stdin.write(`${JSON.stringify(request)}\n`);
}

function stopChild() {
  child.stdin.end();
  setTimeout(() => {
    if (!child.killed) {
      child.kill('SIGTERM');
    }
  }, 500);
}

child.stdout.on('data', (data) => {
  buffer += data.toString();
  const lines = buffer.split(/\n/);
  buffer = lines.pop();

  for (const line of lines) {
    if (!line.trim()) continue;
    const message = JSON.parse(line);
    console.log(JSON.stringify(message, null, 2));

    if (message.id === 2) {
      sessionId = message.result.sessionId;
      send({
        jsonrpc: '2.0',
        id: 3,
        method: 'session/list',
        params: { cwd },
      });
    } else if (message.id === 3) {
      send({
        jsonrpc: '2.0',
        id: 4,
        method: 'session/prompt',
        params: {
          sessionId,
          prompt: [{ type: 'text', text: '你好' }],
        },
      });
    } else if (message.id === 4) {
      stopChild();
    }
  }
});

child.on('close', (code) => {
  console.log(`\n=== Tests Complete: exit ${code} ===`);
  process.exit(code);
});

send({
  jsonrpc: '2.0',
  id: 1,
  method: 'initialize',
  params: {
    protocolVersion: 1,
    clientCapabilities: {
      fs: { readTextFile: true, writeTextFile: true },
      terminal: true,
    },
    clientInfo: { name: 'NodeTestClient', version: '1.0' },
  },
});

send({
  jsonrpc: '2.0',
  id: 2,
  method: 'session/new',
  params: { cwd, mcpServers: [] },
});
