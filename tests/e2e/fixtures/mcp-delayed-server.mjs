import fs from 'node:fs';
import readline from 'node:readline';

const [
  counterPath,
  delayValue = '1500',
  shutdownDelayValue = '0',
  lifecycleEventPath,
] = process.argv.slice(2);
const initializeDelayMs = Number(delayValue);
const shutdownDelayMs = Number(shutdownDelayValue);

if (!counterPath) {
  throw new Error('counter path is required');
}

fs.appendFileSync(counterPath, `${process.pid}\n`, 'utf8');

const input = readline.createInterface({ input: process.stdin });
let shuttingDown = false;

function recordLifecycleEvent(event) {
  if (!lifecycleEventPath) return;
  fs.appendFileSync(lifecycleEventPath, `${event}:${process.pid}\n`, 'utf8');
}

function beginShutdown(signal) {
  if (shuttingDown) return;
  shuttingDown = true;
  recordLifecycleEvent(signal);
  setTimeout(() => process.exit(0), shutdownDelayMs);
}

process.on('SIGTERM', () => beginShutdown('SIGTERM'));
process.on('SIGINT', () => beginShutdown('SIGINT'));

function reply(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`);
}

input.on('line', (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return;
  }

  if (message.id === undefined || message.id === null) return;

  switch (message.method) {
    case 'initialize':
      setTimeout(() => {
        reply(message.id, {
          protocolVersion: '2025-06-18',
          capabilities: { tools: {} },
          serverInfo: { name: 'OpenBitFun delayed E2E MCP', version: '1.0.0' },
        });
      }, initializeDelayMs);
      break;
    case 'tools/list':
      reply(message.id, { tools: [] });
      break;
    case 'resources/list':
      reply(message.id, { resources: [] });
      break;
    case 'prompts/list':
      reply(message.id, { prompts: [] });
      break;
    default:
      reply(message.id, {});
      break;
  }
});
