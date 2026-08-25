import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import test from 'node:test';

const requireFromWebUi = createRequire(
  new URL('../../../../../../../../../web-ui/package.json', import.meta.url),
);
const { JSDOM, VirtualConsole } = requireFromWebUi('jsdom');

const ASSET_ROOT = new URL('../', import.meta.url);

async function readAsset(name) {
  return readFile(new URL(name, ASSET_ROOT), 'utf8');
}

async function waitFor(predicate, description) {
  const deadline = Date.now() + 2000;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  assert.fail(`timed out waiting for ${description}`);
}

function plain(value) {
  return JSON.parse(JSON.stringify(value));
}

function installBrowserShims(window) {
  window.requestAnimationFrame = (callback) => {
    queueMicrotask(() => callback(Date.now()));
    return 1;
  };
  window.cancelAnimationFrame = () => {};
  window.setInterval = () => 1;
  window.clearInterval = () => {};
  window.document.querySelectorAll('dialog').forEach((dialog) => {
    dialog.showModal = () => {
      dialog.open = true;
      dialog.setAttribute('open', '');
    };
    dialog.close = () => {
      dialog.open = false;
      dialog.removeAttribute('open');
    };
  });
}

function issueKey(number = 2382) {
  return {
    repository: {
      host: 'github.com',
      owner: 'GCWing',
      repository: 'BitFun',
    },
    kind: 'issue',
    number,
  };
}

function taskSnapshot(now) {
  return {
    taskId: 'task-2382-1',
    batchId: 'batch-1',
    identity: { item: issueKey(), attempt: 1 },
    generation: 2,
    revision: 7,
    goalId: 'goal-2382',
    agentId: 'agent-loopx',
    state: 'running',
    phase: 'agent_running',
    workspacePath: 'D:\\BitFun-worktrees\\issue-2382',
    modelId: 'primary',
    grantedScopes: [
      'workspace_read',
      'workspace_write',
      'git_local',
      'github_read',
      'agent_execution',
    ],
    currentTurnId: 'turn-4',
    currentTool: 'cargo test',
    lastOutputAt: now - 5000,
    deadlineAt: now + 120000,
    retryAt: null,
    error: null,
    settlement: {},
    createdAt: now - 60000,
    updatedAt: now - 5000,
  };
}

function controllerSnapshot(now, task) {
  const available = (version, detail) => ({
    status: 'available',
    version,
    detail,
    checkedAt: now - 1000,
  });
  return {
    schemaVersion: 1,
    streamId: 'stream-runtime-1',
    cursor: 2,
    revision: 11,
    executionDomain: 'local_desktop',
    executionSupport: 'supported',
    unsupportedReason: null,
    environment: {
      revision: 4,
      status: 'degraded',
      core: {
        sidecar: available('0.2.13', 'Pinned adapter ready'),
        gitWorktree: available('2.51.0', 'Worktree service ready'),
        agentModel: available('primary', 'Model available'),
      },
      optional: {
        pythonFallback: {
          status: 'unavailable',
          detail: 'Optional fallback is not installed',
          checkedAt: now - 1000,
        },
        openViking: { status: 'unknown', detail: 'Not configured' },
        githubAuth: available('gh', 'Authenticated'),
      },
      checkedAt: now - 1000,
    },
    tasks: [task],
    generatedAt: now,
  };
}

function historyEvents(now) {
  return [
    {
      streamId: 'stream-runtime-1',
      cursor: 1,
      taskId: 'task-2382-1',
      generation: 2,
      revision: 6,
      kind: 'task_created',
      level: 'info',
      source: 'controller',
      phase: 'queued',
      message: 'Task created after intake confirmation',
      important: true,
      details: { attempt: '1' },
      occurredAt: now - 10000,
    },
    {
      streamId: 'stream-runtime-1',
      cursor: 2,
      taskId: 'task-2382-1',
      generation: 2,
      revision: 7,
      kind: 'phase_changed',
      level: 'info',
      source: 'agent',
      phase: 'agent_running',
      message: 'Agent turn is running in the bound worktree',
      important: true,
      toolName: 'cargo test',
      deadlineAt: now + 120000,
      details: {},
      occurredAt: now - 5000,
    },
  ];
}

function intakePreview(now) {
  return {
    fingerprint: 'sha256:runtime-preview',
    target: { targetType: 'item', item: issueKey() },
    repository: issueKey().repository,
    workspace: {
      disposition: 'existing_worktree',
      path: 'D:\\BitFun-worktrees\\issue-2382',
      repositoryVerified: true,
    },
    candidates: [{
      key: issueKey(),
      url: 'https://github.com/GCWing/BitFun/issues/2382',
      title: 'Keep LoopX tasks alive outside the MiniApp tab',
      state: 'open',
      fromRepository: false,
      hasImages: false,
      defaultSelected: true,
    }],
    truncated: false,
    model: { modelId: 'primary', available: true, supportsImages: true },
    permissionScopes: [
      'workspace_read',
      'workspace_write',
      'agent_execution',
      'publish',
    ],
    resolvedAt: now,
    expiresAt: now + 60000,
  };
}

test('thin client boots from host state and completes the confirmed intake flow', async () => {
  const [html, ui] = await Promise.all([
    readAsset('index.html'),
    readAsset('ui.js'),
  ]);
  const virtualConsole = new VirtualConsole();
  const jsdomErrors = [];
  virtualConsole.on('jsdomError', (error) => jsdomErrors.push(error));
  const dom = new JSDOM(html, {
    url: 'https://miniapp.invalid/builtin-bitfun-loopx/',
    runScripts: 'outside-only',
    pretendToBeVisual: true,
    virtualConsole,
  });
  const { window } = dom;
  installBrowserShims(window);

  const now = Date.now();
  const task = taskSnapshot(now);
  const snapshot = controllerSnapshot(now, task);
  const events = historyEvents(now);
  const preview = intakePreview(now);
  const callOrder = [];
  const attachRequests = [];
  const eventRequests = [];
  const resolveRequests = [];
  const createRequests = [];
  const forbiddenAccesses = [];
  let eventListener = null;

  const loopx = {
    onEvent(listener) {
      callOrder.push('onEvent');
      eventListener = listener;
    },
    offEvent(listener) {
      assert.equal(listener, eventListener);
    },
    async attach(request) {
      callOrder.push('attach');
      attachRequests.push(request);
      return { snapshot: structuredClone(snapshot) };
    },
    async eventsSince(request) {
      eventRequests.push(request);
      return {
        status: 'current',
        streamId: snapshot.streamId,
        events: structuredClone(events),
        nextCursor: snapshot.cursor,
        hasMore: false,
      };
    },
    async resolveIntake(request) {
      resolveRequests.push(request);
      return { preview: structuredClone(preview) };
    },
    async createTask(request) {
      createRequests.push(request);
      return {
        outcomes: [{
          item: issueKey(),
          kind: 'opened_existing',
          taskId: task.taskId,
          attempt: 1,
        }],
        snapshotRevision: snapshot.revision,
      };
    },
    async action() {
      assert.fail('the intake smoke test must not dispatch task actions');
    },
  };
  const appTarget = {
    locale: 'en-US',
    loopx,
    onLocaleChange() {},
    onActivate() {},
  };
  window.app = new Proxy(appTarget, {
    get(target, property, receiver) {
      if (['agent', 'call', 'worker'].includes(String(property))) {
        forbiddenAccesses.push(String(property));
      }
      return Reflect.get(target, property, receiver);
    },
  });

  try {
    window.eval(ui);
    await waitFor(() => window.document.querySelectorAll('#log-list .log-row').length === 2, 'initial rendering');

    assert.deepEqual(callOrder.slice(0, 2), ['onEvent', 'attach']);
    assert.deepEqual(plain(attachRequests[0]), {});
    assert.deepEqual(plain(eventRequests), [{
      streamId: snapshot.streamId,
      afterCursor: 0,
      limit: 250,
    }]);
    assert.equal(window.document.querySelector('#loopx-app').getAttribute('aria-busy'), 'false');
    assert.equal(window.document.querySelector('#task-count').textContent, '1');
    assert.match(window.document.querySelector('#task-items').textContent, /GCWing\/BitFun Issue #2382/);
    assert.equal(window.document.querySelector('#environment-status').textContent, 'Degraded');
    assert.match(window.document.querySelector('#core-environment-list').textContent, /0\.2\.13/);
    assert.match(window.document.querySelector('#log-list').textContent, /Agent turn is running/);

    assert.equal(typeof eventListener, 'function');
    eventListener({
      event: {
        streamId: snapshot.streamId,
        cursor: 3,
        taskId: task.taskId,
        generation: 2,
        revision: 7,
        kind: 'log',
        level: 'warning',
        source: 'git',
        phase: 'agent_running',
        message: 'Validation produced one warning',
        important: false,
        details: {},
        occurredAt: now,
      },
    });
    await waitFor(
      () => /Validation produced one warning/.test(window.document.querySelector('#log-list').textContent),
      'stream event rendering',
    );

    const input = window.document.querySelector('#intake-input');
    input.value = 'https://github.com/GCWing/BitFun/issues/2382';
    window.document.querySelector('#intake-form').dispatchEvent(new window.Event('submit', {
      bubbles: true,
      cancelable: true,
    }));
    await waitFor(() => window.document.querySelector('#intake-dialog').open, 'intake dialog');

    assert.deepEqual(plain(resolveRequests), [{
      input: 'https://github.com/GCWing/BitFun/issues/2382',
      modelId: 'auto',
    }]);
    assert.equal(window.document.querySelector('#preview-repository').textContent, 'GCWing/BitFun');
    assert.equal(
      window.document.querySelector('#preview-workspace').textContent,
      'D:\\BitFun-worktrees\\issue-2382',
    );
    assert.match(window.document.querySelector('#candidate-list').textContent, /Keep LoopX tasks alive/);
    assert.equal(window.document.querySelector('input[name="candidate"]').checked, true);
    assert.equal(
      window.document.querySelector('input[name="permission"][value="publish"]').checked,
      false,
    );

    window.document.querySelector('#intake-confirm-form').dispatchEvent(new window.Event('submit', {
      bubbles: true,
      cancelable: true,
    }));
    await waitFor(() => createRequests.length === 1 && !window.document.querySelector('#intake-dialog').open, 'task creation');

    assert.equal(createRequests[0].previewFingerprint, preview.fingerprint);
    assert.deepEqual(plain(createRequests[0].selectedItems), [issueKey()]);
    assert.deepEqual(plain(createRequests[0].grantedScopes), [
      'workspace_read',
      'workspace_write',
      'agent_execution',
    ]);
    assert.equal(createRequests[0].modelId, 'primary');
    assert.equal(createRequests[0].retryTerminal, false);
    assert.ok(createRequests[0].clientRequestId);
    assert.match(window.document.querySelector('#notice').textContent, /existing task.*duplicate/i);
    assert.deepEqual(forbiddenAccesses, []);
    assert.deepEqual(jsdomErrors, []);
  } finally {
    window.close();
  }
});
