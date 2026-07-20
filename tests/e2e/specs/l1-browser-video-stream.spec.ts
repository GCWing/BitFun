/**
 * L1 browser video stream spec: verifies the built-in browser can navigate a
 * native child WebView to a dynamic video stream fixture without breaking the
 * surrounding browser toolbar interactions.
 */

import { browser, expect, $ } from '@wdio/globals';
import { createServer, type Server } from 'http';
import { readFile } from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';

const currentFile = fileURLToPath(import.meta.url);
const currentDir = path.dirname(currentFile);
const fixturePath = path.resolve(currentDir, '..', 'fixtures', 'browser-video-stream.html');

async function startFixtureServer(): Promise<{ server: Server; url: string }> {
  const html = await readFile(fixturePath);
  const server = createServer((request, response) => {
    if (request.url === '/' || request.url === '/browser-video-stream.html') {
      response.writeHead(200, {
        'content-length': html.length,
        'content-type': 'text/html; charset=utf-8',
      });
      response.end(html);
      return;
    }

    response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
    response.end('Not found');
  });

  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => resolve());
  });

  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('Fixture server did not expose a TCP port');
  }

  return {
    server,
    url: `http://127.0.0.1:${address.port}/browser-video-stream.html`,
  };
}

describe('L1 Built-in browser video stream', () => {
  let fixtureServer: Server | null = null;
  let fixtureUrl = '';

  before(async () => {
    const fixture = await startFixtureServer();
    fixtureServer = fixture.server;
    fixtureUrl = fixture.url;
  });

  after(async () => {
    if (fixtureServer) {
      fixtureServer.closeAllConnections?.();
      fixtureServer.closeIdleConnections?.();
      await new Promise<void>((resolve) => fixtureServer?.close(() => resolve()));
      fixtureServer = null;
    }
  });

  it('navigates the native WebView to the video stream fixture and keeps toolbar controls clickable', async () => {
    const entry = await $('[data-testid="browser-panel-entry"]');
    await entry.waitForClickable({ timeout: 15000 });
    await entry.click();

    const input = await $('[data-testid="browser-url-input"]');
    await input.waitForDisplayed({ timeout: 15000 });
    await browser.execute((url: string) => {
      const inputElement = document.querySelector<HTMLInputElement>('[data-testid="browser-url-input"]');
      if (!inputElement) throw new Error('Browser URL input not found');

      const valueSetter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
      valueSetter?.call(inputElement, url);
      inputElement.dispatchEvent(new Event('input', { bubbles: true }));
      inputElement.form?.requestSubmit();
    }, fixtureUrl);

    await browser.waitUntil(async () => {
      const current = await $('[data-testid="browser-current-url"]');
      if (!(await current.isExisting())) return false;
      return (await current.getText()).includes('/browser-video-stream.html');
    }, {
      timeout: 15000,
      interval: 250,
      timeoutMsg: 'Browser UI did not reflect the video stream fixture URL',
    });

    const webviewLabel = await browser.waitUntil(async () => {
      const label = await browser.execute(() => {
        const host = document.querySelector('[data-webview-label]');
        return host?.getAttribute('data-webview-label') || '';
      });
      return label || false;
    }, {
      timeout: 15000,
      interval: 250,
      timeoutMsg: 'Browser WebView label was not exposed',
    }) as string;

    await browser.waitUntil(async () => {
      return browser.execute(async (label: string, expectedUrl: string) => {
        const invoke = window.__TAURI__?.core?.invoke;
        if (typeof invoke !== 'function') return false;
        const url = await invoke('browser_get_url', { request: { label } });
        return url === expectedUrl;
      }, webviewLabel, fixtureUrl);
    }, {
      timeout: 15000,
      interval: 250,
      timeoutMsg: 'Native browser WebView did not navigate to the video stream fixture',
    });

    const refresh = await $('[data-testid="browser-refresh-button"]');
    await refresh.waitForClickable({ timeout: 5000 });
    await refresh.click();

    await browser.pause(500);

    const urlAfterRefresh = await browser.execute(async (label: string) => {
      const invoke = window.__TAURI__?.core?.invoke;
      if (typeof invoke !== 'function') return '';
      return invoke('browser_get_url', { request: { label } });
    }, webviewLabel);

    expect(urlAfterRefresh).toBe(fixtureUrl);
  });
});
