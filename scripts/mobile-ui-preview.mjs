#!/usr/bin/env node

import { createReadStream, existsSync, readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, normalize, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';
import { spawn } from 'node:child_process';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const PREVIEW_ROOT = join(ROOT, 'src', 'apps', 'mobile', 'design-system', 'preview');
const SHARED_ASSETS = new Map([
  ['/design-system-tokens.css', join(ROOT, 'design-system', 'packages', 'design-tokens', 'dist', 'tokens.css')],
  ['/design-system-theme.css', join(ROOT, 'design-system', 'packages', 'theme-bitfun', 'dist', 'themes.css')],
]);
const requiredFiles = ['index.html', 'preview.css', 'preview.js', 'generated/mobile-design-data.js'];

for (const file of requiredFiles) {
  const path = join(PREVIEW_ROOT, file);
  if (!existsSync(path) || readFileSync(path, 'utf8').trim().length === 0) {
    console.error(`[mobile-ui-preview] Missing preview asset: ${file}`);
    process.exit(1);
  }
}
for (const [requestPath, assetPath] of SHARED_ASSETS) {
  if (!existsSync(assetPath) || readFileSync(assetPath, 'utf8').trim().length === 0) {
    console.error(`[mobile-ui-preview] Missing canonical design-system asset for ${requestPath}.`);
    process.exit(1);
  }
}

if (process.argv.includes('--check')) {
  console.log('[mobile-ui-preview] Preview assets are present.');
  process.exit(0);
}

const portArgIndex = process.argv.indexOf('--port');
const port = portArgIndex >= 0 ? Number(process.argv[portArgIndex + 1]) : 4178;
const host = '127.0.0.1';
const url = `http://${host}:${port}`;
const server = createServer((request, response) => {
  const requestPath = decodeURIComponent((request.url ?? '/').split('?')[0]);
  const sharedAsset = SHARED_ASSETS.get(requestPath);
  if (sharedAsset) {
    response.setHeader('X-BitFun-Mobile-Preview', '1');
    response.setHeader('Content-Type', contentType(extname(sharedAsset)));
    createReadStream(sharedAsset).pipe(response);
    return;
  }
  const relativePath = requestPath === '/' ? 'index.html' : requestPath.replace(/^\/+/, '');
  const path = normalize(join(PREVIEW_ROOT, relativePath));
  if (!path.startsWith(`${PREVIEW_ROOT}${sep}`) && path !== join(PREVIEW_ROOT, 'index.html')) {
    response.writeHead(403).end('Forbidden');
    return;
  }
  if (!existsSync(path)) {
    response.writeHead(404).end('Not found');
    return;
  }
  response.setHeader('X-BitFun-Mobile-Preview', '1');
  response.setHeader('Content-Type', contentType(extname(path)));
  createReadStream(path).pipe(response);
});

server.on('error', async (error) => {
  if (error.code === 'EADDRINUSE' && await isBitFunPreview(url)) {
    console.log(`[mobile-ui-preview] Reusing existing preview at ${url}`);
    openPreview(url);
    process.exit(0);
  }
  if (error.code === 'EADDRINUSE') {
    console.error(`[mobile-ui-preview] Port ${port} is already used by another application. Pass --port <number> to choose another port.`);
  } else {
    console.error(`[mobile-ui-preview] Failed to start: ${error.message}`);
  }
  process.exit(1);
});

server.listen(port, host, () => {
  console.log(`[mobile-ui-preview] ${url}`);
  openPreview(url);
});

async function isBitFunPreview(target) {
  try {
    const response = await fetch(target, { signal: AbortSignal.timeout(1500) });
    if (response.headers.get('x-bitfun-mobile-preview') === '1') return true;
    return (await response.text()).includes('<title>BitFun Mobile Parity Bench</title>');
  } catch {
    return false;
  }
}

function openPreview(target) {
  if (!process.argv.includes('--no-open') && process.platform === 'darwin') {
    spawn('open', [target], { detached: true, stdio: 'ignore' }).unref();
  }
}

function contentType(extension) {
  return ({
    '.html': 'text/html; charset=utf-8',
    '.css': 'text/css; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.json': 'application/json; charset=utf-8',
    '.png': 'image/png',
    '.jpg': 'image/jpeg',
    '.jpeg': 'image/jpeg',
  })[extension] ?? 'application/octet-stream';
}
