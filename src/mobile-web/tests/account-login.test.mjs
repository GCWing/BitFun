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

const links = await loadSource('../src/services/pairingLink.ts');
const selection = await loadSource('../src/services/accountDeviceSelection.ts');
const { selectAccountDevice } = await import(selection.url);
const offline = { device_id: 'desktop-a', device_name: 'Offline desktop', online: false };
const online = { device_id: 'desktop-b', device_name: 'Online desktop', online: true };
const controller = { device_id: 'browser', device_name: 'Browser', online: true };

const agentContract = await loadSource('../../shared/agent-harness/contract.generated.ts');
const navigationModule = await loadSource('../src/services/MobileNavigationStore.ts', {
  '../../../shared/agent-harness/contract.generated': agentContract.url,
});
const { loadMobileNavigation, saveMobileNavigation, clearMobileNavigation } = await import(navigationModule.url);

test('same-tab reload restores the selected device and session only within the authenticated QR scope', () => {
  const entries = new Map();
  const storage = {
    getItem: key => entries.get(key) ?? null,
    setItem: (key, value) => entries.set(key, value),
    removeItem: key => entries.delete(key),
  };
  const scope = {
    accountId: 'account-a', controllerDeviceId: 'browser-a',
    relayUrl: 'https://relay.example.com', routeKey: '/relay/r/room/#/pair?did=desktop-a',
  };
  const navigation = { deviceId: 'desktop-b', session: { id: 'session-b', name: 'Task B', agentType: 'agentic' } };
  saveMobileNavigation(scope, navigation, storage);
  assert.deepEqual(loadMobileNavigation(scope, storage), {
    ...navigation, session: { ...navigation.session, agentType: 'Standard' },
  });
  for (const replacement of [
    { accountId: 'account-b' }, { controllerDeviceId: 'browser-b' },
    { relayUrl: 'https://another-relay.example.com' }, { routeKey: '/relay/r/new/#/pair?did=desktop-c' },
  ]) {
    assert.equal(loadMobileNavigation({ ...scope, ...replacement }, storage), null);
  }
  clearMobileNavigation(storage);
  assert.equal(loadMobileNavigation(scope, storage), null);
});

test('unsupported or unreadable navigation stays intact and unavailable browser storage is optional', () => {
  for (const raw of ['{', JSON.stringify({ version: 2, deviceId: 'future' })]) {
    let retained = raw;
    const storage = { getItem: () => retained, setItem: value => { retained = value; }, removeItem: () => { retained = ''; } };
    assert.equal(loadMobileNavigation({}, storage), null);
    assert.equal(retained, raw);
  }
  const blocked = { getItem: () => { throw new Error('blocked'); }, setItem: () => { throw new Error('blocked'); }, removeItem: () => { throw new Error('blocked'); } };
  assert.equal(loadMobileNavigation({}, blocked), null);
  assert.doesNotThrow(() => saveMobileNavigation({}, { deviceId: 'device' }, blocked));
  assert.doesNotThrow(() => clearMobileNavigation(blocked));
});

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

test('GitHub login registers an independent device public key and reuses the device key across sign-ins', async () => {
  const encryption = await loadSource('../src/services/E2EEncryption.ts');
  const { deriveDeviceMessageKey } = await import(encryption.url);
  const { x25519 } = await import('@noble/curves/ed25519.js');
  const authModule = await loadSource('../src/services/CloudAccountClient.ts', { './E2EEncryption': encryption.url, './pairingLink': links.url });
  const { CloudAccountClient } = await import(authModule.url);
  const originalFetch = globalThis.fetch;
  const originalWindow = globalThis.window;
  const requests = [];
  globalThis.window = { setTimeout, clearTimeout };
  const originalStorage = globalThis.sessionStorage;
  const keys = new Map();
  globalThis.sessionStorage = { getItem: key => keys.get(key) ?? null, setItem: (key, value) => keys.set(key, value) };
  globalThis.fetch = async (url, options) => {
    assert.equal(url, 'https://remote.openbitfun.com/v/1.0.0/api/auth/login');
    const body = JSON.parse(options.body);
    requests.push(body);
    assert.equal(body.access_token, 'verified-github-token');
    assert.equal(body.device_id, 'browser');
    assert.equal(body.password_hash, undefined);
    assert.equal(Buffer.from(body.public_key, 'base64').length, 32);
    return Response.json({ token: 'test-account-token', user_id: '101' });
  };
  try {
    const first = await new CloudAccountClient('https://remote.openbitfun.com/v/1.0.0').login('verified-github-token', 'browser');
    const second = await new CloudAccountClient('https://remote.openbitfun.com/v/1.0.0').login('verified-github-token', 'browser');
    assert.equal(first.userId, '101');
    assert.deepEqual(first.masterKey, second.masterKey);
    assert.equal(requests[0].public_key, requests[1].public_key);
    assert.deepEqual(Buffer.from(requests[0].public_key, 'base64'), Buffer.from(x25519.getPublicKey(first.masterKey)));
    assert.deepEqual(deriveDeviceMessageKey(first.masterKey, x25519.getPublicKey(second.masterKey)),
      deriveDeviceMessageKey(second.masterKey, x25519.getPublicKey(first.masterKey)));
    assert.throws(() => deriveDeviceMessageKey(first.masterKey, new Uint8Array(32)));
    assert.equal(Buffer.from(deriveDeviceMessageKey(new Uint8Array(32).fill(7), x25519.getPublicKey(new Uint8Array(32).fill(11)))).toString('hex'), '6e8f5da837e91e9ddb09c5aa7dee229e731fc94499d29d10dcf5f1437193f56c');
  } finally {
    globalThis.sessionStorage = originalStorage;
    globalThis.fetch = originalFetch;
    if (originalWindow === undefined) delete globalThis.window;
    else globalThis.window = originalWindow;
  }
});

test('account UI entry precedes discovery and mounts no remote workspace surface', async () => {
  const pairing = await readFile(new URL('../src/pages/PairingPage.tsx', import.meta.url), 'utf8');
  const direct = pairing.slice(pairing.indexOf('const connect ='), pairing.indexOf('  useEffect('));
  assert.match(direct, /saveCloudAccountSession/);
  assert.match(direct, /store\.setControlTarget\(null\)/);
  assert.match(direct, /onPairedRef\.current/);
  assert.doesNotMatch(direct, /listDevices\(|sendDeviceRpc\(|\.online|throw new Error/);
  const app = await readFile(new URL('../src/App.tsx', import.meta.url), 'utf8');
  assert.match(app, /useConnectionHealth\(accountDirectoryOpen \? null : sessionMgr\)/);
  assert.match(app, /!accountDirectoryOpen && page !== 'pairing' && sessionMgrRef\.current/);
  assert.equal((app.match(/\{renderDetailPage\(\)\}/g) || []).length, 1, 'one gated detail tree must preserve chat state across layout changes');
  const devices = await readFile(new URL('../src/pages/DevicesPage.tsx', import.meta.url), 'utf8');
  assert.match(devices, /if \(!d.online \|\| switchingId\) return/);
  assert.match(devices, /automaticSelectionAttemptedRef\.current = true/);
  assert.match(devices, /selectDevice\(target, false\)/, 'initial account selection must not require a new peer command');
  assert.ok(devices.indexOf('await client.sendDeviceRpc') < devices.indexOf('client.setTargetDeviceId(d.device_id)'));
});

test('authorization follows the central GitHub OAuth URL and rejects lookalike destinations', async () => {
  const encryption = await loadSource('../src/services/E2EEncryption.ts');
  const authModule = await loadSource('../src/services/CloudAccountClient.ts', { './E2EEncryption': encryption.url, './pairingLink': links.url });
  const { CloudAccountClient } = await import(authModule.url);
  const previous = { fetch: globalThis.fetch, window: globalThis.window, setTimeout: globalThis.setTimeout };
  globalThis.window = { setTimeout: previous.setTimeout, clearTimeout };
  globalThis.setTimeout = callback => { queueMicrotask(callback); return 0; };
  try {
    for (const authorizationUrl of [
      'https://github.com/login/oauth/authorize?state=test',
      'https://github.com.attacker.example/login/oauth/authorize',
      'https://github.com/login', 'https://user@github.com/login/oauth/authorize',
      'http://github.com/login/oauth/authorize',
    ]) {
      let polls = 0;
      globalThis.fetch = async url => {
        if (url.endsWith('/start')) return Response.json({
          transactionId: 'txn', transactionSecret: 'secret', authorizationUrl,
          expiresAt: Date.now() / 1000 + 60, pollIntervalSeconds: 3,
        });
        polls++;
        return Response.json({ status: 'authorized', tokens: { accessToken: 'verified' } });
      };
      const popup = { location: { href: 'about:blank' } };
      const result = new CloudAccountClient('https://remote.openbitfun.com/v/1.0.0').authorize(popup, new AbortController().signal);
      if (authorizationUrl === 'https://github.com/login/oauth/authorize?state=test') {
        assert.equal(await result, 'verified');
        assert.equal(popup.location.href, authorizationUrl);
        assert.equal(polls, 1);
      } else {
        await assert.rejects(result, /Untrusted/);
        assert.equal(popup.location.href, 'about:blank');
        assert.equal(polls, 0);
      }
    }
  } finally {
    globalThis.fetch = previous.fetch;
    globalThis.setTimeout = previous.setTimeout;
    if (previous.window === undefined) delete globalThis.window;
    else globalThis.window = previous.window;
  }
});


test('official and local invitations share strict device-only targeting', async () => {
  const { currentRelayUrl, pairingRelayUrl, accountDeviceIdFromHash } = await import(links.url);
  for (const base of ['https://remote.openbitfun.com/v/1.0.0/', 'http://192.168.1.9:9700/']) {
    const url = new URL(`${base}#/pair?did=desktop-1`);
    assert.equal(currentRelayUrl(url), base.replace(/\/$/, ''));
    assert.equal(accountDeviceIdFromHash(url.hash), 'desktop-1');
    for (const hash of ['did=a&did=b', 'did=a&pk=untrusted', 'did=a&relay=https://evil.example',
      'did=%2Fother', 'room=room&pk=key', 'did=..']) {
      assert.equal(accountDeviceIdFromHash(`#/pair?${hash}`), null);
    }
  }
  for (const base of ['https://evil.example/', 'https://remote.openbitfun.com.evil.example/v/1.0.0/',
    'https://user@remote.openbitfun.com/v/1.0.0/', 'http://remote.openbitfun.com/v/1.0.0/',
    'https://remote.openbitfun.com/relay/']) {
    assert.equal(pairingRelayUrl(base), null);
  }
  assert.equal(accountDeviceIdFromHash('#/pair?did=desktop'), 'desktop');
  assert.equal(accountDeviceIdFromHash('#/chat?did=desktop'), null);
});

const githubModule = await loadSource('../src/services/GitHubAccountProfile.ts');
const { loadGitHubAccountProfile, normalizeGitHubProfile } = await import(githubModule.url);
const githubUser = { id: 123, login: 'example', avatar_url: 'https://avatars.githubusercontent.com/u/123' };

test('GitHub public profile validates stable identity and never forwards account credentials', async () => {
  assert.throws(() => normalizeGitHubProfile('124', githubUser));
  assert.equal(normalizeGitHubProfile('123', { ...githubUser, avatar_url: 'https://other.example/avatar' }).avatarUrl, '');
  const profiles = [];
  await loadGitHubAccountProfile('123', p => profiles.push(p), new AbortController().signal, null, async (url, options) => {
    assert.equal(url, 'https://api.github.com/user/123');
    assert.equal(options.credentials, 'omit');
    assert.deepEqual(options.headers, { Accept: 'application/vnd.github+json' });
    return { ok: true, json: async () => githubUser };
  });
  assert.equal(profiles[0].login, 'example');
});

test('GitHub cached profiles survive refresh failure and fresh cache avoids a request', async () => {
  for (const age of [0, 2 * 86400000]) {
    const profile = { ...normalizeGitHubProfile('123', githubUser), fetchedAt: Date.now() - age };
    let requests = 0;
    const applied = [];
    await loadGitHubAccountProfile('123', p => applied.push(p), new AbortController().signal,
      { getItem: () => JSON.stringify(profile), setItem: () => assert.fail('must retain cache') },
      async () => { requests++; throw new Error('offline'); });
    assert.equal(applied[0].login, 'example');
    assert.equal(requests, age ? 1 : 0);
  }
});

test('GitHub profile ignores corrupt/foreign cache and stale account responses', async () => {
  for (const raw of ['{', JSON.stringify({ userId: '456', fetchedAt: Date.now(), login: 'another' })]) {
    const controller = new AbortController();
    await loadGitHubAccountProfile('123', () => assert.fail('stale profile applied'), controller.signal,
      { getItem: () => raw, setItem: () => assert.fail('stale profile cached') },
      async () => {
        controller.abort();
        return { ok: true, json: async () => githubUser };
      });
  }
});
