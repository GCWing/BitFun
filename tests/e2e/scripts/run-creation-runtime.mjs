/** Native packaged-asset regression, with no dev server, model account or user profile. */
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { cp, mkdir, mkdtemp, open, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const suspendedPaint = process.argv.includes('--suspended-paint');
const root = await mkdtemp(path.join(tmpdir(), 'openbitfun-creation-e2e-'));
const userRoot = path.join(root, 'user');
const productHome = path.join(root, 'home');
const workbench = path.join(userRoot, 'data/frontend-workbench');
const revision = 'creative-native-test';
const overlay = path.join(workbench, 'revisions', revision);
const bundleManifest = await readFile(path.join(repo, 'dist/frontend-revision.json'), 'utf8');
const bundle = JSON.parse(bundleManifest);
const frontendSnapshot = path.join(root, 'frontend');
await cp(path.join(repo, 'dist'), frontendSnapshot, { recursive: true });
assert.equal(await readFile(path.join(repo, 'dist/frontend-revision.json'), 'utf8'), bundleManifest,
  'The build changed while creating the test snapshot. Finish the build and rerun.');
assert.equal(await readFile(path.join(frontendSnapshot, 'frontend-revision.json'), 'utf8'), bundleManifest);
if (suspendedPaint) {
  // Reproduce WebKit's occluded-window startup: timers and DOM commits run,
  // but animation frames do not arrive until the window becomes visible again.
  const indexPath = path.join(frontendSnapshot, 'index.html');
  const index = await readFile(indexPath, 'utf8');
  await writeFile(indexPath, index.replace('<head>', `<head><script>
    (() => {
      const request = window.requestAnimationFrame.bind(window);
      const cancel = window.cancelAnimationFrame.bind(window);
      const pending = new Map(); let next = 1;
      window.requestAnimationFrame = callback => { const id = next++; pending.set(id, callback); return id; };
      window.cancelAnimationFrame = id => pending.delete(id);
      window.__openbitfunResumeTestPaint = () => {
        window.requestAnimationFrame = request; window.cancelAnimationFrame = cancel;
        for (const callback of pending.values()) request(callback);
        pending.clear();
      };
    })();
  </script>`));
}
await mkdir(overlay, { recursive: true });
await mkdir(productHome, { recursive: true });
await writeFile(path.join(overlay, '.creation-overlay.json'), JSON.stringify({ apiVersion: 1 }));
await writeFile(path.join(overlay, 'openbitfun-creation.css'), '[data-testid="creation-native-output"] { white-space: pre-wrap; }');
await writeFile(path.join(overlay, 'openbitfun-creation.js'), `
export default function activate(ui) {
  const root = ui.mount('sidebar-footer');
  const output = document.createElement('pre');
  output.dataset.testid = 'creation-native-output';
  const render = () => { output.textContent = JSON.stringify({ counter: ui.state.get('test.counter', 0), snapshot: ui.inspect() }); };
  ui.events.on('state.changed', render);
  ui.commands.register({ id: 'test.increment', description: 'Increment a persistent counter', parameters: { amount: { type: 'integer', required: true } } },
    ({ amount }) => ui.state.set('test.counter', ui.state.get('test.counter', 0) + amount));
  ui.commands.register({ id: 'test.create', description: 'Create and open an isolated MiniApp' }, async () => {
    const created = await ui.control.execute('feature.miniapps', 'create-app', {
      name: 'Creation native test', html: '<div id="creation-content">Loading</div>',
      uiJs: 'document.getElementById("creation-content").textContent = "Creation MiniApp Ready";',
    });
    await ui.state.set('test.app', created.app);
    await ui.openMiniApp(created.app.appId);
    return created.app;
  });
  ui.commands.register({ id: 'test.update', description: 'Patch the installed app' }, async () => {
    const app = ui.state.get('test.app');
    const updated = await ui.control.execute('feature.miniapps', 'update-app', {
      appId: app.appId, expectedVersion: app.version, css: '#creation-content { padding: 17px; }',
    });
    if (updated.app.html !== app.html || updated.app.uiJs !== app.uiJs) throw new Error('Partial update lost source');
    await ui.state.set('test.app', updated.app);
    return updated.app;
  });
  ui.commands.register({ id: 'test.delete', description: 'Remove the isolated app' }, async () => {
    const app = ui.state.get('test.app');
    await ui.control.execute('feature.miniapps', 'delete-app', { appId: app.appId, expectedVersion: app.version });
    const listed = await ui.control.execute('feature.miniapps', 'list-apps');
    if (listed.apps.some(item => item.id === app.appId)) throw new Error('Deleted app remains installed');
    return { deleted: true };
  });
  const result = document.createElement('pre'); result.dataset.testid = 'creation-native-result';
  for (const name of ['increment', 'create', 'update', 'delete']) {
    const button = document.createElement('button'); button.textContent = name;
    button.dataset.testid = 'creation-native-' + name;
    button.addEventListener('click', async () => {
      result.textContent = '';
      try { result.textContent = JSON.stringify({ ok: true, result: await ui.commands.invoke('test.' + name, name === 'increment' ? { amount: 2 } : {}) }); }
      catch (error) { result.textContent = JSON.stringify({ ok: false, error: String(error) }); }
    }, { signal: ui.signal });
    root.append(button);
  }
  root.append(output, result); render();
}
`);
await writeFile(path.join(workbench, 'state.json'), JSON.stringify({
  schemaVersion: 2, bundledRevision: bundle.revision, activeRevision: revision,
  previousRevision: bundle.revision, pending: null, lastOutcome: null,
}));

const probe = createServer(); probe.listen(0, '127.0.0.1'); await once(probe, 'listening');
const port = probe.address().port; await new Promise(resolve => probe.close(resolve));
const endpoint = 'http://127.0.0.1:' + port;
const log = await open(path.join(root, 'desktop.log'), 'w');
const app = spawn(path.join(repo, 'target/debug/openbitfun-desktop' + (process.platform === 'win32' ? '.exe' : '')), [], {
  cwd: repo, windowsHide: true, stdio: ['ignore', log.fd, log.fd],
  env: { ...process.env, OPENBITFUN_USER_ROOT: userRoot, OPENBITFUN_E2E_USER_ROOT: userRoot,
    OPENBITFUN_HOME: productHome, OPENBITFUN_E2E_HOME: productHome, OPENBITFUN_E2E_STORAGE_GUARD: '1',
    OPENBITFUN_E2E_PACKAGED_FRONTEND: '1', OPENBITFUN_E2E_FRONTEND_DIR: frontendSnapshot,
    OPENBITFUN_E2E_LOG_DIR: path.join(root, 'logs'),
    OPENBITFUN_WEBDRIVER_PORT: String(port), OPENBITFUN_WEBDRIVER_LABEL: 'main' },
});
let exitError;
app.on('error', error => { exitError = error; });
let session;
async function request(route, body) {
  const response = await fetch(endpoint + route, body === undefined ? {} : {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body),
  });
  const result = await response.json();
  if (!response.ok || result.value?.error) throw new Error(JSON.stringify(result));
  return result.value;
}
async function until(read, description, timeout = 45_000) {
  const started = Date.now(); let last;
  while (Date.now() - started < timeout) {
    if (exitError) throw exitError;
    if (app.exitCode !== null) throw new Error('Desktop exited with ' + app.exitCode);
    try { const result = await read(); if (result) return result; } catch (error) { last = error; }
    await new Promise(resolve => setTimeout(resolve, 250));
  }
  throw new Error('Timed out waiting for ' + description + ': ' + (last ?? 'not ready'));
}
const execute = script => request('/session/' + session + '/execute/sync', { script, args: [] });
const readOutput = () => execute('() => document.querySelector("[data-testid=creation-native-output]")?.textContent');
async function resumePaint() {
  if (!suspendedPaint) return;
  const phase = await execute('() => window.__OPENBITFUN_STARTUP_TRACE__?.snapshot().phases.events.find(event => event.phase === "interactive_shell_ready")');
  assert.equal(phase?.reason, 'startup-overlay-hidden');
  assert.equal(phase?.afterPaint, false);
  await execute('() => { window.__openbitfunResumeTestPaint(); return true; }');
}
async function click(name) {
  await execute('() => document.querySelector("[data-testid=creation-native-' + name + ']").click()');
  const raw = await until(() => execute('() => document.querySelector("[data-testid=creation-native-result]")?.textContent'), name);
  const result = JSON.parse(raw); assert.equal(result.ok, true, JSON.stringify(result));
  return result.result;
}
try {
  await until(async () => (await request('/status')).ready, 'embedded driver');
  session = (await request('/session', {})).sessionId;
  await request('/session/' + session + '/timeouts', { script: 5000 });
  const initial = JSON.parse(await until(readOutput, 'packaged customization activation'));
  const url = await execute('() => location.href');
  assert.match(url, /openbitfun-ui/);
  assert.equal(initial.counter, 0);
  assert.ok(initial.snapshot.commands.some(command => command.id === 'test.increment'));
  await resumePaint();
  assert.equal(await click('increment'), 2);
  assert.equal(JSON.parse(await readOutput()).counter, 2);
  const oldTimeOrigin = await execute('() => performance.timeOrigin');
  await execute('() => { location.reload(); return true; }');
  await until(async () => (await execute('() => performance.timeOrigin')) !== oldTimeOrigin, 'new document');
  const reloaded = await until(async () => {
    const raw = await readOutput(); return raw && JSON.parse(raw).counter === 2 ? JSON.parse(raw) : null;
  }, 'state after reload');
  assert.equal(reloaded.counter, 2);
  await resumePaint();
  const created = await click('create');
  await until(() => execute('() => [...document.querySelectorAll("iframe")].some(frame => { try { return frame.contentDocument?.body?.textContent.includes("Creation MiniApp Ready"); } catch { return false; } })'), 'compiled MiniApp UI');
  const updated = await click('update');
  assert.equal(updated.version, created.version + 1);
  await until(() => execute('() => [...document.querySelectorAll("iframe")].some(frame => { try { const element = frame.contentDocument?.getElementById("creation-content"); return element && frame.contentWindow.getComputedStyle(element).paddingTop === "17px"; } catch { return false; } })'), 'updated MiniApp UI');
  const screenshot = await request('/session/' + session + '/screenshot');
  await writeFile(path.join(root, 'native.png'), Buffer.from(screenshot, 'base64'));
  assert.equal((await click('delete')).deleted, true);
  const report = { passed: true, url, suspendedPaint, checks: ['packaged protocol', 'runtime discovery', 'command/state/event/UI composition', 'reload persistence', 'MiniApp create/open/render/update/delete'], artifactRoot: root };
  await writeFile(path.join(root, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report));
} catch (error) {
  if (session) {
    const diagnostics = await execute('() => ({ url: location.href, ready: document.readyState, text: document.body?.innerText.slice(0, 3000), scripts: [...document.scripts].map(script => script.src), creationStyles: document.querySelectorAll("[data-openbitfun-creation]").length, startup: window.__OPENBITFUN_STARTUP_TRACE__?.snapshot() })').catch(String);
    await writeFile(path.join(root, 'failure.json'), JSON.stringify(diagnostics, null, 2));
    const browserLogs = await request('/session/' + session + '/se/log', { type: 'browser' }).catch(String);
    await writeFile(path.join(root, 'browser-errors.json'), JSON.stringify(browserLogs, null, 2));
    const screenshot = await request('/session/' + session + '/screenshot').catch(() => null);
    if (screenshot) await writeFile(path.join(root, 'failure.png'), Buffer.from(screenshot, 'base64'));
  }
  console.error('Creation native regression failed. Artifacts: ' + root);
  throw error;
} finally {
  if (session) await fetch(endpoint + '/session/' + session, { method: 'DELETE' }).catch(() => {});
  if (app.exitCode === null) { app.kill(); await Promise.race([once(app, 'exit'), new Promise(resolve => setTimeout(resolve, 5000))]); }
  await log.close();
}
