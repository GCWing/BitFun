import { spawn, type ChildProcess } from 'node:child_process';
import { createServer, type Server } from 'node:http';
import { mkdtemp, readFile, writeFile, stat, rm, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { Options } from '@wdio/types';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
let vite: ChildProcess;
let files: Server;
let storage: string;

export const config: Options.Testrunner = {
  runner: 'local',
  specs: ['../browser/markdown-editor.spec.ts'],
  maxInstances: 1,
  capabilities: [{ browserName: 'chrome', 'goog:chromeOptions': { args: ['--headless=new', '--window-size=1280,900'] } }],
  framework: 'mocha', reporters: ['spec'], logLevel: 'warn',
  mochaOpts: { timeout: 60000 }, waitforTimeout: 15000,
  baseUrl: 'http://127.0.0.1:1447',
  async onPrepare() {
    storage = await mkdtemp(join(tmpdir(), 'openbitfun-markdown-e2e-'));
    const file = join(storage, 'test.md');
    await writeFile(file, await readFile(join(root, 'tests/e2e/browser/markdown-fixture.md')));
    files = createServer(async (request, response) => {
      response.setHeader('Access-Control-Allow-Origin', 'http://127.0.0.1:1447');
      response.setHeader('Access-Control-Allow-Methods', 'GET, PUT, OPTIONS');
      if (request.method === 'OPTIONS') { response.end(); return; }
      try {
        if (request.url === '/file' && request.method === 'PUT') {
          const chunks = [];
          for await (const chunk of request) chunks.push(chunk);
          await writeFile(file, Buffer.concat(chunks));
          response.end();
        } else if (request.url === '/file' && request.method === 'GET') {
          response.end(await readFile(file));
        } else if (request.url === '/metadata') {
          const info = await stat(file);
          response.setHeader('Content-Type', 'application/json');
          response.end(JSON.stringify({ path: '/workspace/test.md', modified: info.mtimeMs, size: info.size, isFile: true, isDir: false }));
        } else { response.statusCode = 404; response.end(); }
      } catch (error) { response.statusCode = 500; response.end(String(error)); }
    });
    await new Promise<void>((done, reject) => { files.once('error', reject); files.listen(1450, '127.0.0.1', done); });
    vite = spawn('pnpm', ['exec', 'vite', '--host', '127.0.0.1', '--port', '1447', '--strictPort'], {
      cwd: join(root, 'src/web-ui'), windowsHide: true, stdio: 'pipe',
    });
    let output = '';
    vite.stdout?.on('data', chunk => { output += chunk; });
    vite.stderr?.on('data', chunk => { output += chunk; });
    for (let attempt = 0; attempt < 100; attempt++) {
      if (vite.exitCode !== null) throw new Error(output);
      try { if ((await fetch('http://127.0.0.1:1447/tests/e2e/markdown-editor.html')).ok) return; } catch { /* wait for Vite */ }
      await new Promise(done => setTimeout(done, 200));
    }
    throw new Error(`Markdown fixture server did not start: ${output}`);
  },
  async afterTest(test, _context, { passed }) {
    const { browser } = await import('@wdio/globals');
    const directory = join(root, 'tests/e2e/reports/markdown-browser');
    await mkdir(directory, { recursive: true });
    await browser.saveScreenshot(join(directory, `${passed ? 'pass' : 'fail'}-${test.title.replace(/[^a-z0-9]+/gi, '-')}.png`));
  },
  async onComplete() {
    vite?.kill('SIGTERM');
    if (files) await new Promise<void>(done => files.close(() => done()));
    if (storage) await rm(storage, { recursive: true, force: true });
  },
};
