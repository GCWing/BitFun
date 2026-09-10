import { describe, expect, it } from 'vitest';
import type { SessionResponse } from '../types/session';
import { isSessionRunning } from '@/app/scenes/shell/hooks/shellEntryTypes';
import { TerminalOriginCache, isTerminalPathInside, terminalBelongsToWorkspace } from './terminalWorkspaceScope';

const terminal = (overrides: Partial<SessionResponse> = {}): SessionResponse => ({
  id: 'terminal-1', name: 'Development', shellType: 'Bash', cwd: '/repo/src', initialCwd: '/repo',
  source: 'manual', status: 'Running', cols: 80, rows: 24, ...overrides,
});

describe('workspace terminal projection', () => {
  it('keeps a terminal in its creation workspace after cd', () => {
    expect(terminalBelongsToWorkspace(terminal({ cwd: '/another-project' }), { rootPath: '/repo', isRemote: false })).toBe(true);
    expect(terminalBelongsToWorkspace(terminal({ cwd: '/another-project' }), { rootPath: '/another-project', isRemote: false })).toBe(false);
  });
  it('uses path boundaries and the closest opened project', () => {
    expect(isTerminalPathInside('/repository/src', '/repo')).toBe(false);
    const parent = { rootPath: '/repo', isRemote: false };
    const child = { rootPath: '/repo/packages/app', isRemote: false };
    const session = terminal({ initialCwd: '/repo/packages/app/src' });
    expect(terminalBelongsToWorkspace(session, parent, [parent, child])).toBe(false);
    expect(terminalBelongsToWorkspace(session, child, [parent, child])).toBe(true);
  });
  it('uses the target path semantics on every controller OS', () => {
    expect(isTerminalPathInside('C:\\Repo\\src', 'c:/repo/')).toBe(true);
    expect(isTerminalPathInside('/Repo/src', '/repo', true)).toBe(false);
    expect(isTerminalPathInside('/repo/a\\b', '/repo/a', true)).toBe(false);
    expect(isTerminalPathInside('/repo/src', '/', true)).toBe(true);
  });
  it('isolates local workspaces and two SSH connections with identical paths', () => {
    const remote = terminal({ shellType: 'Remote', connectionId: 'ssh-a' });
    expect(terminalBelongsToWorkspace(remote, { rootPath: '/repo', isRemote: false })).toBe(false);
    expect(terminalBelongsToWorkspace(remote, { rootPath: '/repo', isRemote: true, connectionId: 'ssh-b' })).toBe(false);
    expect(terminalBelongsToWorkspace(remote, { rootPath: '/repo', isRemote: true, connectionId: 'ssh-a' })).toBe(true);
    expect(terminalBelongsToWorkspace(remote, { rootPath: '/repo', isRemote: true })).toBe(false);
  });
  it('accepts old host payloads and anchors their first observation per device', () => {
    const origins = new TerminalOriginCache();
    const old = terminal({ initialCwd: undefined });
    expect(origins.project('local', old).initialCwd).toBe('/repo/src');
    expect(origins.project('local', { ...old, cwd: '/different' }).initialCwd).toBe('/repo/src');
    expect(origins.project('peer-b', { ...old, cwd: '/different' }).initialCwd).toBe('/different');
    expect(origins.project('local', { ...old, initialCwd: '/authoritative' }).initialCwd).toBe('/authoritative');
  });
  it('does not report an exited or unknown session as running', () => {
    for (const status of ['Exited { exit_code: Some(0) }', 'Stopped', 'Terminating', 'Error', 'unknown']) {
      expect(isSessionRunning(terminal({ status }))).toBe(false);
    }
    expect(isSessionRunning(terminal())).toBe(true);
    expect(isSessionRunning(terminal({ status: 'Active' }))).toBe(true);
    expect(isSessionRunning(terminal({ status: 'Starting' }))).toBe(true);
    expect(isSessionRunning(terminal({ status: 'Restoring' }))).toBe(true);
  });
});
