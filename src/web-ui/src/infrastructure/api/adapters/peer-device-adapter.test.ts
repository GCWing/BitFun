import { describe, expect, it, vi } from 'vitest';
import {
  PeerDeviceTransportAdapter,
  peerInvokePriorityFor,
} from './peer-device-adapter';

describe('peerInvokePriorityFor', () => {
  it('ranks session hydrate commands high', () => {
    expect(peerInvokePriorityFor('restore_session_view')).toBe('high');
    expect(peerInvokePriorityFor('list_persisted_sessions_page')).toBe('high');
    expect(peerInvokePriorityFor('initialize_workspace_startup_state')).toBe('high');
    expect(peerInvokePriorityFor('start_dialog_turn')).toBe('high');
    expect(peerInvokePriorityFor('reload_config')).toBe('high');
  });

  it('ranks interactive peer directory browsing high', () => {
    expect(peerInvokePriorityFor('get_directory_children')).toBe('high');
    expect(peerInvokePriorityFor('get_directory_children_paginated')).toBe('high');
    expect(peerInvokePriorityFor('list_files')).toBe('high');
    expect(peerInvokePriorityFor('check_path_exists')).toBe('high');
    expect(peerInvokePriorityFor('create_directory')).toBe('high');
    expect(peerInvokePriorityFor('get_system_info')).toBe('high');
  });

  it('ranks git/ssh/editor/fs/search noise low', () => {
    expect(peerInvokePriorityFor('git_is_repository')).toBe('low');
    expect(peerInvokePriorityFor('ssh_is_connected')).toBe('low');
    expect(peerInvokePriorityFor('get_file_metadata')).toBe('low');
    expect(peerInvokePriorityFor('lsp_detect_project')).toBe('low');
    expect(peerInvokePriorityFor('search_get_repo_status')).toBe('low');
    expect(peerInvokePriorityFor('load_canvas_artifact')).toBe('low');
    expect(peerInvokePriorityFor('get_file_tree')).toBe('low');
  });
});

describe('PeerDeviceTransportAdapter queue', () => {
  it('lets high-priority HostInvoke jump ahead of queued low-priority work', async () => {
    const started: string[] = [];
    const gate = createDeferred<void>();

    const deviceRpc = vi.fn(async (_target: string, commandJson: string) => {
      const parsed = JSON.parse(commandJson) as { command: string };
      started.push(parsed.command);
      if (parsed.command === 'git_is_repository') {
        await gate.promise;
      }
      return JSON.stringify({
        resp: 'host_invoke_result',
        ok: true,
        value: parsed.command === 'git_is_repository' ? true : { ok: true },
      });
    });

    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc, {}, 1);
    await adapter.connect();

    const low1 = adapter.request('git_is_repository', { request: { repositoryPath: '/a' } });
    const low2 = adapter.request('ssh_is_connected', { connectionId: 'ssh-x' });
    // Allow the first low request to claim the single concurrency slot.
    await Promise.resolve();
    expect(started).toEqual(['git_is_repository']);

    const high = adapter.request('restore_session_view', {
      request: { sessionId: 's1' },
    });
    await Promise.resolve();
    expect(adapter.getQueueDepthsForTest()).toEqual({
      high: 1,
      normal: 0,
      low: 1,
    });

    gate.resolve();
    await Promise.all([low1, high, low2]);

    expect(started).toEqual([
      'git_is_repository',
      'restore_session_view',
      'ssh_is_connected',
    ]);
  });
});

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}
