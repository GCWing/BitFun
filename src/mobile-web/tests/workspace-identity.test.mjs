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
