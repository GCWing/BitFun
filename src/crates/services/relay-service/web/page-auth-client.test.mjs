import assert from 'node:assert/strict';
import test from 'node:test';
import vm from 'node:vm';
import { readFile } from 'node:fs/promises';

const script = await readFile(new URL('../src/routes/page_auth_client.bundle.js', import.meta.url), 'utf8');

async function runLogin({ state, status = 'approved', authorizationUrl = 'https://github.com/login/oauth/authorize?state=test' } = {}) {
  let submit;
  let redirect;
  let opened;
  let closed = false;
  const calls = [];
  const button = { disabled: false, textContent: '' };
  const error = { hidden: true, textContent: '' };
  const form = { dataset: { pageLoginState: state }, addEventListener: (_, callback) => { submit = callback; } };
  const elements = { '[data-page-login-form]': form, '[data-page-login-submit]': button, '[data-page-login-error]': error };
  const context = {
    document: { querySelector: (selector) => elements[selector] },
    navigator: { language: 'en' },
    URL, Error, Date,
    setTimeout: (callback) => callback(),
    window: {
      location: { pathname: state ? '/v/1.0.0/api/page-auth/login' : '/v/1.0.0/p/alice/demo', search: '?q=1', replace: (value) => { redirect = value; } },
      open: () => ({ opener: {}, location: { replace: (value) => { opened = value; } }, close: () => { closed = true; } }),
    },
    fetch: async (url, options) => {
      const body = JSON.parse(options.body);
      calls.push({ url, body });
      if (url.endsWith('/github/start')) return { ok: true, json: async () => ({ transactionId: 'txn', transactionSecret: 'secret', authorizationUrl, expiresAt: Date.now() / 1000 + 60, pollIntervalSeconds: 3 }) };
      if (url.endsWith('/github/poll')) return { ok: true, json: async () => ({ status, tokens: status === 'approved' ? { accessToken: 'verified-account-token' } : undefined }) };
      if (url.endsWith('/page-auth/login')) return { ok: true, json: async () => ({ redirect_to: state ? 'https://pages.example/callback?code=one-time' : '/p/alice/demo?q=1' }) };
      throw new Error(`Unexpected URL ${url}`);
    },
  };
  vm.runInNewContext(script, context);
  await submit({ preventDefault() {} });
  return { calls, redirect, opened, closed, error, button };
}

test('GitHub exchange preserves version prefix and submits only verified identity for Page access', async () => {
  const result = await runLogin();
  assert.equal(result.calls.length, 3);
  assert.equal(result.calls[1].url, '/v/1.0.0/api/auth/github/poll');
  assert.deepEqual(result.calls[1].body, { transactionId: 'txn', transactionSecret: 'secret' });
  assert.deepEqual(result.calls[2].body, { access_token: 'verified-account-token', return_to: '/p/alice/demo?q=1', path_prefix: '/v/1.0.0' });
  assert.equal(result.redirect, '/v/1.0.0/p/alice/demo?q=1');
  assert.equal(result.closed, true);
  assert.equal(result.button.disabled, false);
});

test('isolated Page sign-in retains the one-time login state and callback origin', async () => {
  const result = await runLogin({ state: 'login-state' });
  assert.deepEqual(result.calls[2].body, { access_token: 'verified-account-token', state: 'login-state' });
  assert.equal(result.redirect, 'https://pages.example/callback?code=one-time');
});

test('failed authorization or an untrusted OAuth URL never submits Page access', async () => {
  for (const options of [{ status: 'expired' }, { authorizationUrl: 'https://attacker.example/login' }]) {
    const result = await runLogin(options);
    assert.equal(result.calls.some((call) => call.url.endsWith('/page-auth/login')), false);
    assert.equal(result.redirect, undefined);
    assert.equal(result.error.hidden, false);
    assert.equal(result.closed, true);
  }
});
