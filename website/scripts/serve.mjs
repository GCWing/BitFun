#!/usr/bin/env node

import { createReadStream, existsSync, statSync, watch } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(websiteRoot, '..');
const outputRoot = path.join(websiteRoot, 'dist');
const args = process.argv.slice(2);
const portIndex = args.indexOf('--port');
const port = Number(portIndex >= 0 ? args[portIndex + 1] : 4174);
const watchMode = args.includes('--watch');
const contentTypes = new Map([
  ['.css', 'text/css; charset=utf-8'],
  ['.html', 'text/html; charset=utf-8'],
  ['.js', 'text/javascript; charset=utf-8'],
  ['.json', 'application/json; charset=utf-8'],
  ['.png', 'image/png'],
  ['.txt', 'text/plain; charset=utf-8'],
  ['.xml', 'application/xml; charset=utf-8'],
]);

function build() {
  const result = spawnSync(process.execPath, ['scripts/build.mjs'], {
    cwd: websiteRoot,
    stdio: 'inherit',
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function resolveRequest(requestUrl) {
  const pathname = decodeURIComponent(new URL(requestUrl, `http://localhost:${port}`).pathname);
  const requested = path.resolve(outputRoot, `.${pathname}`);
  if (!requested.startsWith(`${outputRoot}${path.sep}`) && requested !== outputRoot) return null;
  if (existsSync(requested) && statSync(requested).isFile()) return requested;
  const index = path.join(requested, 'index.html');
  if (existsSync(index)) return index;
  return path.join(outputRoot, 'index.html');
}

build();

if (watchMode) {
  let timer;
  const rebuild = () => {
    clearTimeout(timer);
    timer = setTimeout(build, 120);
  };
  watch(path.join(websiteRoot, 'src'), { recursive: true }, rebuild);
  watch(path.join(repositoryRoot, 'docs/interactive-capabilities'), { recursive: true }, rebuild);
}

createServer((request, response) => {
  const file = resolveRequest(request.url ?? '/');
  if (!file || !existsSync(file)) {
    response.writeHead(404).end('Not found');
    return;
  }
  response.writeHead(200, {
    'Cache-Control': file.endsWith('.html') ? 'no-cache' : 'public, max-age=300',
    'Content-Type': contentTypes.get(path.extname(file)) ?? 'application/octet-stream',
    'X-Content-Type-Options': 'nosniff',
  });
  createReadStream(file).pipe(response);
}).listen(port, '127.0.0.1', () => {
  process.stdout.write(`BitFun Playbook: http://127.0.0.1:${port}/\n`);
});
