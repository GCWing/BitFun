import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

async function loadSource(relativePath, imports = {}) {
  const source = await readFile(new URL(relativePath, import.meta.url), 'utf8');
  let code = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
  }).outputText;
  code = code.replace(/from (['"])([^'"]+)\1/g, (_, quote, specifier) => (
    `from ${JSON.stringify(imports[specifier] ?? import.meta.resolve(specifier))}`
  ));
  return { url: `data:text/javascript;base64,${Buffer.from(code).toString('base64')}`, source };
}

const selection = await loadSource('../src/services/accountDeviceSelection.ts');
const { selectAccountDevice } = await import(selection.url);
const offline = { device_id: 'desktop-a', device_name: 'Offline desktop', online: false };
const online = { device_id: 'desktop-b', device_name: 'Online desktop', online: true };
const controller = { device_id: 'browser', device_name: 'Browser', online: true };

test('empty, offline and controller-only directories leave the account without a target', () => {
  for (const devices of [[], [offline], [controller], [offline, controller]]) {
    assert.equal(selectAccountDevice(devices, 'browser'), null);
  }
});

test('online selection preserves exact QR targeting and supports later device availability', () => {
  assert.equal(selectAccountDevice([controller, offline, online], 'browser'), online);
  assert.equal(selectAccountDevice([online], 'browser', offline.device_id), null);
  assert.equal(selectAccountDevice([offline, online], 'browser', offline.device_id), null);
  const reconnected = { ...offline, online: true };
  assert.equal(selectAccountDevice([reconnected, online], 'browser', offline.device_id), reconnected);
});

test('real account authentication does not request a device or QR room', async () => {
  const { argon2idAsync } = await import('@noble/hashes/argon2.js');
  const { gcm } = await import('@noble/ciphers/aes.js');
  const encryption = await loadSource('../src/services/E2EEncryption.ts');
  const authModule = await loadSource('../src/services/CloudAccountClient.ts', {
    './E2EEncryption': encryption.url,
  });
  const { CloudAccountClient } = await import(authModule.url);
  const params = { m: 8192, t: 1, p: 1 };
  const password = 'local-test-only';
  const salt = new Uint8Array(16).fill(1);
  const kdfSalt = new Uint8Array(16).fill(2);
  const masterKey = new Uint8Array(32).fill(3);
  const nonce = new Uint8Array(12).fill(4);
  const kek = await argon2idAsync(password, salt, { ...params, dkLen: 32 });
  const passwordHash = await argon2idAsync(password, kdfSalt, { ...params, dkLen: 32 });
  const b64 = value => Buffer.from(value).toString('base64');
  const originalFetch = globalThis.fetch;
  const originalWindow = globalThis.window;
  const requests = [];
  globalThis.window = { setTimeout, clearTimeout };
  globalThis.fetch = async (url, options) => {
    const path = new URL(url).pathname;
    requests.push(path);
    if (path.endsWith('/challenge')) return Response.json({
      salt: b64(salt), kdf_salt: b64(kdfSalt), argon2_params: JSON.stringify(params),
      wrapped_master_key: `${b64(gcm(kek, nonce).encrypt(masterKey))}.${b64(nonce)}`,
    });
    assert.equal(path, '/api/auth/login');
    const body = JSON.parse(options.body);
    assert.equal(body.password_hash, b64(passwordHash));
    return Response.json({ token: 'test-account-token', user_id: 'test-account' });
  };
  try {
    const account = await new CloudAccountClient().login('http://test.invalid', 'test', password, 'browser');
    assert.equal(account.userId, 'test-account');
    assert.deepEqual(account.masterKey, masterKey);
    assert.deepEqual(requests, ['/api/auth/login/challenge', '/api/auth/login']);
  } finally {
    globalThis.fetch = originalFetch;
    if (originalWindow === undefined) delete globalThis.window;
    else globalThis.window = originalWindow;
  }
});

test('account UI entry precedes discovery and mounts no remote workspace surface', async () => {
  const pairing = await readFile(new URL('../src/pages/PairingPage.tsx', import.meta.url), 'utf8');
  const direct = pairing.slice(pairing.indexOf('const restoredAccount ='), pairing.indexOf('const initialSync ='));
  assert.match(direct, /saveCloudAccountSession/);
  assert.match(direct, /store\.setControlTarget\(null\)/);
  assert.match(direct, /onPairedRef\.current/);
  assert.doesNotMatch(direct, /listDevices\(|sendDeviceRpc\(|\.online|throw new Error/);
  const app = await readFile(new URL('../src/App.tsx', import.meta.url), 'utf8');
  assert.match(app, /useConnectionHealth\(accountDirectoryOpen \? null : sessionMgr\)/);
  assert.match(app, /!accountDirectoryOpen && page !== 'pairing' && isWideLayout/);
  assert.match(app, /!accountDirectoryOpen && !isWideLayout/);
  const devices = await readFile(new URL('../src/pages/DevicesPage.tsx', import.meta.url), 'utf8');
  assert.match(devices, /if \(!d.online \|\| switchingId\) return/);
  assert.match(devices, /automaticSelectionAttemptedRef\.current = true/);
  assert.match(devices, /selectDevice\(target, false\)/, 'initial account selection must not require a new peer command');
  assert.ok(devices.indexOf('await client.sendDeviceRpc') < devices.indexOf('client.setPairedDeviceId(d.device_id)'));
});
