import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import ts from 'typescript';

const source = await readFile(new URL('../src/services/workspaceIdentity.ts', import.meta.url), 'utf8');
const code = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
}).outputText;
const { mergeWorkspaceSessions, sessionMatchesWorkspace, workspaceIdentityKey } = await import(
  `data:text/javascript;base64,${Buffer.from(code).toString('base64')}`
);

const a = { path: '/projects/herdr', remote_connection_id: 'ssh-a', remote_ssh_host: 'host-a' };
const b = { path: a.path, remote_connection_id: 'ssh-b', remote_ssh_host: 'host-b' };
const local = { path: a.path };
const row = (id, identity) => ({
  session_id: id, name: id, agent_type: 'agentic', created_at: '1', updated_at: '1',
  message_count: 0, workspace_path: a.path, workspace_identity: identity,
});

test('same-path SSH and local session caches retain distinct ownership after reload', () => {
  let sessions = mergeWorkspaceSessions([], [row('a')], a, true);
  sessions = mergeWorkspaceSessions(sessions, [row('b')], b, true);
  sessions = mergeWorkspaceSessions(sessions, [row('local')], local, true);
  sessions = JSON.parse(JSON.stringify(sessions));
  for (const [workspace, id] of [[a, 'a'], [b, 'b'], [local, 'local']]) {
    assert.deepEqual(sessions.filter(s => sessionMatchesWorkspace(s, workspace, [a, b, local]))
      .map(s => s.session_id), [id]);
  }
  const refreshed = mergeWorkspaceSessions(sessions, [], a, true);
  assert.deepEqual(refreshed.map(s => s.session_id), ['b', 'local']);
});

test('legacy rows remain readable and are retained without guessing a remote host', () => {
  const legacy = JSON.parse(JSON.stringify(row('legacy')));
  assert.equal(sessionMatchesWorkspace(legacy, local), true);
  assert.equal(sessionMatchesWorkspace(legacy, a), false);
  assert.equal(sessionMatchesWorkspace(legacy, local, [a, local]), false);
  assert.deepEqual(mergeWorkspaceSessions([legacy], [], a, true), [legacy]);
  const repaired = mergeWorkspaceSessions([legacy], [legacy], b, true);
  assert.equal(repaired.length, 1);
  assert.equal(sessionMatchesWorkspace(repaired[0], b), true);
  const unscopedRefresh = mergeWorkspaceSessions(repaired, [legacy], undefined, false);
  assert.equal(sessionMatchesWorkspace(unscopedRefresh[0], b), true);
});

test('workspace identity keys cannot collide through delimiter-shaped paths and ids', () => {
  assert.notEqual(workspaceIdentityKey(a), workspaceIdentityKey(b));
  assert.notEqual(workspaceIdentityKey(a), workspaceIdentityKey(local));
  assert.notEqual(workspaceIdentityKey({ path: 'c:d', remote_connection_id: 'a', remote_ssh_host: 'b' }),
    workspaceIdentityKey({ path: 'd', remote_connection_id: 'a:b', remote_ssh_host: 'c' }));
});

const { projectWorkspaceCatalog } = await import(
  `data:text/javascript;base64,${Buffer.from(code).toString('base64')}`
);
const named = (identity, name) => ({ ...identity, name, last_opened: '' });
const assistant = { path: '/assistant/workspace', name: 'Mina', workspace_kind: 'assistant', last_opened: '' };

test('opened catalog excludes closed history and preserves the assistant identity name', () => {
  const response = {
    workspaces: [named(a, 'Closed SSH'), named(local, 'Project'), named({ path: '/old-worktree' }, 'Closed worktree')],
    opened_workspaces: [assistant, named(local, 'Project')],
  };
  const catalog = projectWorkspaceCatalog(JSON.parse(JSON.stringify(response)));
  assert.equal(catalog.source, 'opened');
  assert.deepEqual(catalog.workspaces.map(w => w.name), ['Mina', 'Project']);
  assert.equal(catalog.workspaces[0].workspace_kind, 'assistant');
  // A successful empty refresh must stay empty, even with retained history.
  assert.deepEqual(projectWorkspaceCatalog({ ...response, opened_workspaces: [] }).workspaces, []);
  assert.deepEqual(response.workspaces.map(w => w.name), ['Closed SSH', 'Project', 'Closed worktree']);
});

test('catalog preserves same-path SSH identities and never substitutes a local assistant name', () => {
  const response = { workspaces: [], opened_workspaces: [named(a, 'A'), named(b, 'B'), named(local, 'Local'), named(a, 'duplicate')] };
  assert.deepEqual(projectWorkspaceCatalog(response).workspaces.map(w => w.name), ['A', 'B', 'Local']);
});

test('legacy hosts advertise recent-history fallback and resolve assistants without guessing paths', () => {
  const legacy = JSON.parse(JSON.stringify({ workspaces: [
    named({ path: assistant.path }, 'workspace'),
    named({ path: '/ordinary/workspace' }, 'workspace'),
    named({ path: assistant.path, remote_connection_id: 'ssh-a', remote_ssh_host: 'host-a' }, 'Remote workspace'),
  ] }));
  const catalog = projectWorkspaceCatalog(legacy, [assistant]);
  assert.equal(catalog.source, 'recent');
  assert.deepEqual(catalog.workspaces.map(w => w.name), ['Mina', 'workspace', 'Remote workspace']);
  assert.equal(catalog.workspaces[0].workspace_kind, 'assistant');
});

async function moduleUrl(relative, imports = {}) {
  let transformed = ts.transpileModule(await readFile(new URL(relative, import.meta.url), 'utf8'), {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2020 },
  }).outputText;
  transformed = transformed.replace(/from (['"])([^'"]+)\1/g, (_, quote, specifier) => {
    assert.ok(imports[specifier], `unexpected dependency ${specifier}`);
    return `from ${JSON.stringify(imports[specifier])}`;
  });
  return `data:text/javascript;base64,${Buffer.from(transformed).toString('base64')}`;
}
const agentContract = await moduleUrl('../../shared/agent-harness/contract.generated.ts');
const agentWire = await moduleUrl('../../shared/agent-harness/wire.ts', { './contract.generated': agentContract });
const controlIdentity = await moduleUrl('../src/services/controlClientIdentity.ts');
const managerUrl = await moduleUrl('../src/services/RemoteSessionManager.ts', {
  '../../../shared/agent-harness/wire': agentWire,
  './controlClientIdentity': controlIdentity,
  './workspaceIdentity': `data:text/javascript;base64,${Buffer.from(code).toString('base64')}`,
});
const { RemoteSessionManager, RemoteControlTargetChangedError } = await import(managerUrl);
function catalogClient(handler) {
  const client = {
    controlTargetEpoch: 1,
    targetDeviceId: 'desktop-a',
    getControlTargetSnapshot: () => ({ epoch: client.controlTargetEpoch, deviceId: client.targetDeviceId }),
    isControlTargetCurrent: snapshot => snapshot.epoch === client.controlTargetEpoch && snapshot.deviceId === client.targetDeviceId,
    sendDeviceRpc: handler,
  };
  return client;
}

test('manager consumes the advertised opened catalog without issuing a legacy assistant read', async () => {
  const calls = [];
  const manager = new RemoteSessionManager(catalogClient(async (device, cmd) => {
    calls.push(cmd.cmd);
    return { resp: 'recent_workspaces', workspaces: [named(a, 'Closed')], opened_workspaces: [assistant] };
  }));
  const catalog = await manager.listWorkspaceCatalog();
  assert.deepEqual(catalog.workspaces, [assistant]);
  assert.deepEqual(calls, ['list_recent_workspaces']);
});

test('legacy catalog reads stay on one device generation across both requests', async () => {
  const calls = [];
  let completeAssistants;
  const client = catalogClient(async (device, cmd) => {
    calls.push([device, cmd.cmd]);
    if (cmd.cmd === 'list_recent_workspaces') return { resp: 'recent_workspaces', workspaces: [] };
    return new Promise(resolve => { completeAssistants = resolve; });
  });
  const pending = new RemoteSessionManager(client).listWorkspaceCatalog();
  while (!completeAssistants) await new Promise(resolve => setImmediate(resolve));
  client.controlTargetEpoch = 2;
  client.targetDeviceId = 'desktop-b';
  completeAssistants({ resp: 'assistant_list', assistants: [assistant] });
  await assert.rejects(pending, RemoteControlTargetChangedError);
  assert.deepEqual(calls, [['desktop-a', 'list_recent_workspaces'], ['desktop-a', 'list_assistants']]);
});
