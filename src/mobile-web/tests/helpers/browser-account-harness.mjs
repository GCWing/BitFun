import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { createServer } from 'vite';
import puppeteer from 'puppeteer-core';
import { x25519 } from '@noble/curves/ed25519.js';
import { hkdf } from '@noble/hashes/hkdf.js';
import { sha256 } from '@noble/hashes/sha2.js';
import { gcm } from '@noble/ciphers/aes.js';

export const LAN = 'http://192.168.50.7:9700';
export const OFFICIAL = 'https://remote.openbitfun.com/v/1.0.0';
export const invitation = (endpoint = LAN, device = 'desktop-a') => `${endpoint}/#/pair?did=${device}`;
const mobileRoot = fileURLToPath(new URL('../../', import.meta.url));
const hostPrivateKey = new Uint8Array(32).fill(11);
const hostPublicKey = x25519.getPublicKey(hostPrivateKey);

function messageKey(publicKey) {
  const shared = x25519.getSharedSecret(hostPrivateKey, publicKey);
  const sorted = [hostPublicKey, publicKey].sort((a, b) => Buffer.compare(a, b));
  return hkdf(sha256, shared, new TextEncoder().encode('OpenBitFun Relay v1.0.0 device key'),
    Buffer.concat(sorted.map(key => Buffer.from(key))), 32);
}

export async function until(check, message, timeout = 15_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await check()) return;
    await delay(25);
  }
  assert.fail(message);
}

export async function launchBrowser(options = {}) {
  const executablePath = [
    process.env.PUPPETEER_EXECUTABLE_PATH,
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/usr/bin/google-chrome', '/usr/bin/google-chrome-stable', '/usr/bin/chromium', '/usr/bin/chromium-browser',
    process.env.PROGRAMFILES && `${process.env.PROGRAMFILES}/Google/Chrome/Application/chrome.exe`,
  ].find(path => path && existsSync(path));
  assert.ok(executablePath, 'Set PUPPETEER_EXECUTABLE_PATH to an installed Chrome/Chromium browser.');
  return puppeteer.launch({ executablePath, headless: 'new',
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-background-timer-throttling'], ...options });
}

export async function startSourceServer() {
  const vite = await createServer({ root: mobileRoot, configFile: `${mobileRoot}/vite.config.ts`,
    logLevel: 'error', server: { host: '127.0.0.1', port: 0, hmr: false } });
  await vite.listen();
  return { origin: `http://127.0.0.1:${vite.httpServer.address().port}`, close: () => vite.close() };
}

/** Real browser fetches and encrypted RPC envelopes; only Relay/host IO is simulated. */
export class RelayFixture {
  devices = new Map();
  tokens = new Map();
  logins = [];
  revoked = [];
  pings = [];
  clients = new Map();
  errors = [];
  directoryStatus = 200;
  holdLogins = false;
  pendingLogins = [];
  online = true;

  register(endpoint, deviceId, privateKey, userId = '123', token = `fixture-${this.tokens.size + 1}`) {
    this.devices.set(`${endpoint}:${userId}:${deviceId}`, Buffer.from(x25519.getPublicKey(privateKey)));
    this.tokens.set(token, { endpoint, userId, deviceId });
    return token;
  }

  async api(endpoint, path, request) {
    const json = body => ({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
    const authorization = request.headers().authorization ?? '';
    const token = authorization.replace(/^Bearer /, '');
    const auth = this.tokens.get(token);
    if (path === '/api/auth/login') {
      const body = JSON.parse(request.postData());
      assert.match(body.access_token, /^fixture-user-/);
      const userId = body.access_token.slice('fixture-user-'.length);
      const publicKey = Buffer.from(body.public_key, 'base64');
      const deviceKey = `${endpoint}:${userId}:${body.device_id}`;
      const existing = this.devices.get(deviceKey);
      if (existing) assert.deepEqual(publicKey, existing, 'concurrent sign-ins rotated an existing device key');
      this.devices.set(deviceKey, publicKey);
      const issued = `fixture-login-${this.logins.length + 1}`;
      this.tokens.set(issued, { endpoint, userId, deviceId: body.device_id });
      this.logins.push({ ...body, token: issued });
      if (this.holdLogins) await new Promise(resolve => this.pendingLogins.push(resolve));
      return json({ token: issued, user_id: userId });
    }
    if (!auth || auth.endpoint !== endpoint) return { status: 401, body: 'Unauthorized' };
    if (path === '/api/auth/logout') {
      this.tokens.delete(token); this.revoked.push(token);
      return { status: 204, body: '' };
    }
    if (path === '/api/devices') {
      if (this.directoryStatus !== 200) return { status: this.directoryStatus, body: 'Unavailable' };
      return json(['desktop-a', 'desktop-b'].map(device_id => ({ device_id, device_name: device_id, online: this.online })));
    }
    const keyPath = path.match(/^\/api\/devices\/([^/]+)\/key$/);
    if (keyPath) return json({ device_id: keyPath[1], public_key: Buffer.from(hostPublicKey).toString('base64') });
    const rpcPath = path.match(/^\/api\/devices\/([^/]+)\/rpc$/);
    if (rpcPath) {
      const target = rpcPath[1];
      const key = messageKey(this.devices.get(`${endpoint}:${auth.userId}:${auth.deviceId}`));
      const envelope = JSON.parse(request.postData());
      const command = JSON.parse(new TextDecoder().decode(gcm(key, Buffer.from(envelope.nonce, 'base64'))
        .decrypt(Buffer.from(envelope.encrypted_data, 'base64'))));
      let response;
      switch (command.cmd) {
        case 'ping': {
          const ping = { endpoint, target, controller: auth.deviceId, client: command.client };
          this.pings.push(ping);
          this.clients.set(`${endpoint}:${target}:${command.client.id}`, command.client);
          response = { resp: 'pong' }; break;
        }
        case 'get_workspace_info': response = { resp: 'workspace_info', has_workspace: false, capabilities: [] }; break;
        case 'list_recent_workspaces': response = { resp: 'recent_workspaces', workspaces: [], opened_workspaces: [] }; break;
        case 'list_sessions': response = { resp: 'sessions', sessions: [], has_more: false }; break;
        case 'list_assistants': response = { resp: 'assistants', assistants: [] }; break;
        case 'host_invoke': response = { resp: 'host_invoke_result', ok: true, result: {} }; break;
        default: throw new Error(`Unhandled test host command: ${command.cmd}`);
      }
      const nonce = randomBytes(12);
      const encrypted = gcm(key, nonce).encrypt(new TextEncoder().encode(JSON.stringify(response)));
      return json({ encrypted_data: Buffer.from(encrypted).toString('base64'), nonce: nonce.toString('base64') });
    }
    throw new Error(`Unhandled Relay route: ${path}`);
  }

  async page(context, sourceOrigin, url = invitation(), init) {
    const page = await context.newPage();
    await page.setViewport({ width: 1280, height: 850 });
    page.on('pageerror', error => this.errors.push(error));
    await page.setRequestInterception(true);
    page.on('request', request => {
      void (async () => {
        const requested = new URL(request.url());
        if (requested.hostname === 'api.github.com') {
          const id = Number(requested.pathname.split('/').at(-1));
          await request.respond({ status: 200, contentType: 'application/json',
            body: JSON.stringify({ id, login: `user-${id}`, avatar_url: '' }) });
          return;
        }
        const endpoint = requested.hostname === 'remote.openbitfun.com' ? OFFICIAL : LAN;
        if (![new URL(LAN).origin, new URL(OFFICIAL).origin].includes(requested.origin)) {
          await request.abort(); return;
        }
        const path = requested.pathname.replace(/^\/v\/1\.0\.0(?=\/|$)/, '') || '/';
        if (path === '/' && requested.searchParams.has('account-store-test')) {
          await request.respond({ status: 200, contentType: 'text/html', body: '<!doctype html><title>Browser storage contract</title>' });
          return;
        }
        if (path.startsWith('/api/')) {
          await request.respond(await this.api(endpoint, path, request)); return;
        }
        const source = await fetch(`${sourceOrigin}${path}${requested.search}`);
        await request.respond({ status: source.status, contentType: source.headers.get('content-type') || 'text/plain',
          body: Buffer.from(await source.arrayBuffer()) });
      })().catch(async error => {
        this.errors.push(error);
        if (!request.isInterceptResolutionHandled()) await request.abort().catch(() => {});
      });
    });
    if (init) await page.evaluateOnNewDocument(init);
    await page.goto(url, { waitUntil: 'networkidle0' });
    return page;
  }
}

export async function readAccount(page) {
  return page.evaluate(async () => {
    const { getBrowserAccountStore, releaseBrowserAccount } = await import('/src/services/BrowserAccountStore.ts');
    const { currentRelayUrl } = await import('/src/services/pairingLink.ts');
    const saved = await getBrowserAccountStore(currentRelayUrl()).read();
    const summary = { controllerDeviceId: saved.controllerDeviceId, revision: saved.revision,
      token: saved.session?.token ?? null, userId: saved.session?.userId ?? null };
    releaseBrowserAccount(saved);
    return summary;
  });
}

export async function signIn(page, user = '123') {
  await page.bringToFront();
  await page.waitForSelector('.pairing-page__form button[type="submit"]');
  // OAuth itself is covered by the account contract suite; no real GitHub
  // authorization or user credentials are used by these browser tests.
  await page.evaluate(async user => {
    const { CloudAccountClient } = await import('/src/services/CloudAccountClient.ts');
    CloudAccountClient.prototype.authorize = async () => `fixture-user-${user}`;
  }, user);
  await page.click('.pairing-page__form button[type="submit"]');
}

export async function connected(page, device = 'desktop-a') {
  await page.waitForFunction(async expected => {
    const { useMobileStore } = await import('/src/services/store.ts');
    const state = useMobileStore.getState();
    return state.controlTarget?.deviceId === expected && state.connectionHealth === 'connected';
  }, { timeout: 15_000, polling: 100 }, device);
  assert.equal(await page.$('.pairing-page__form'), null);
}

export async function signOut(page) {
  await page.bringToFront();
  // Open the device directory from the UI if this tab is controlling a host.
  const button = await page.$('button[aria-label="Devices"]');
  if (button) { await button.click(); await page.waitForSelector('.devices-page'); }
  const clicked = await page.evaluate(() => {
    const button = [...document.querySelectorAll('button')].find(node => node.textContent.trim() === 'Sign out');
    if (!button) return false;
    button.click(); return true;
  });
  assert.ok(clicked, 'Sign out must be reachable in the device directory');
  await page.waitForSelector('.pairing-page__form');
}
