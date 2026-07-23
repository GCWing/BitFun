import { describe, expect, it, vi } from 'vitest';
import {
  PEER_MUTATION_REQUEST_TIMEOUT_MS,
  PEER_READ_REQUEST_TIMEOUT_MS,
  PeerDeviceTransportAdapter,
  isPeerLocalOnlyCommand,
  isPeerRetryableIdempotentMutation,
  isPeerRetryableReadCommand,
  peerInvokePriorityFor,
} from './peer-device-adapter';

describe('peerInvokePriorityFor', () => {
  it('ranks session hydrate commands high', () => {
    expect(peerInvokePriorityFor('restore_session_view')).toBe('high');
    expect(peerInvokePriorityFor('list_persisted_sessions_page')).toBe('high');
    expect(peerInvokePriorityFor('initialize_workspace_startup_state')).toBe('high');
    expect(peerInvokePriorityFor('start_dialog_turn')).toBe('high');
    expect(peerInvokePriorityFor('reload_config')).toBe('high');
    expect(peerInvokePriorityFor('get_config')).toBe('high');
    expect(peerInvokePriorityFor('get_available_modes')).toBe('high');
  });

  it('keeps account finalize and relay deploy on the controller', () => {
    expect(isPeerLocalOnlyCommand('account_finalize_login')).toBe(true);
    expect(isPeerLocalOnlyCommand('account_cancel_pending_login')).toBe(true);
    expect(isPeerLocalOnlyCommand('account_fetch_session_turns')).toBe(true);
    expect(isPeerLocalOnlyCommand('relay_deploy_start')).toBe(true);
    expect(isPeerLocalOnlyCommand('relay_deploy_cancel')).toBe(true);
    expect(isPeerLocalOnlyCommand('create_session')).toBe(false);
  });

  it('ranks interactive peer directory browsing high', () => {
    expect(peerInvokePriorityFor('get_directory_children')).toBe('high');
    expect(peerInvokePriorityFor('get_directory_children_paginated')).toBe('high');
    expect(peerInvokePriorityFor('list_files')).toBe('high');
    expect(peerInvokePriorityFor('check_path_exists')).toBe('high');
    expect(peerInvokePriorityFor('create_directory')).toBe('high');
    expect(peerInvokePriorityFor('get_system_info')).toBe('high');
  });

  it('ranks permission control commands high', () => {
    for (const command of [
      'list_pending_permission_requests',
      'subscribe_permission_requests',
      'respond_permission',
      'respond_permission_batch',
      'list_project_permission_grants',
      'remove_project_permission_grant',
      'clear_project_permission_grants',
      'list_project_permission_audit',
      'get_project_permission_rules',
      'save_project_permission_rules',
    ]) {
      expect(peerInvokePriorityFor(command)).toBe('high');
    }
  });

  it('ranks all terminal commands high', () => {
    expect(peerInvokePriorityFor('terminal_create')).toBe('high');
    expect(peerInvokePriorityFor('terminal_write')).toBe('high');
    expect(peerInvokePriorityFor('terminal_resize')).toBe('high');
    expect(peerInvokePriorityFor('terminal_signal')).toBe('high');
  });

  it('retries only idempotent Peer reads', () => {
    expect(isPeerRetryableReadCommand('list_persisted_sessions_page')).toBe(true);
    expect(isPeerRetryableReadCommand('get_opened_workspaces')).toBe(true);
    expect(isPeerRetryableReadCommand('restore_session_view')).toBe(true);
    expect(isPeerRetryableReadCommand('start_dialog_turn')).toBe(false);
    expect(isPeerRetryableReadCommand('delete_session')).toBe(false);
    expect(isPeerRetryableReadCommand('respond_permission')).toBe(false);
  });

  it('retries only mutations with an explicit host idempotency identity', () => {
    expect(isPeerRetryableIdempotentMutation('start_dialog_turn', {
      request: { sessionId: 'session-1', turnId: 'turn-1' },
    })).toBe(true);
    expect(isPeerRetryableIdempotentMutation('start_acp_dialog_turn', {
      request: { sessionId: 'session-1', turnId: 'turn-1' },
    })).toBe(true);
    expect(isPeerRetryableIdempotentMutation('start_dialog_turn', {
      request: { sessionId: 'session-1' },
    })).toBe(false);
    expect(isPeerRetryableIdempotentMutation('delete_session', {
      request: { sessionId: 'session-1', turnId: 'turn-1' },
    })).toBe(false);
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

  it('reserves one concurrency slot for terminal work', async () => {
    const started: string[] = [];
    const firstLowGate = createDeferred<void>();

    const deviceRpc = vi.fn(async (_target: string, commandJson: string) => {
      const parsed = JSON.parse(commandJson) as { command: string };
      started.push(parsed.command);
      if (parsed.command === 'git_is_repository') {
        await firstLowGate.promise;
      }
      return JSON.stringify({
        resp: 'host_invoke_result',
        ok: true,
        value: true,
      });
    });

    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc, {}, 2);
    await adapter.connect();

    const low1 = adapter.request('git_is_repository', {
      request: { repositoryPath: '/a' },
    });
    const low2 = adapter.request('ssh_is_connected', { connectionId: 'ssh-x' });
    await Promise.resolve();
    expect(started).toEqual(['git_is_repository']);

    const terminal = adapter.request('terminal_write', {
      request: { sessionId: 't1', data: 'pwd\r' },
    });
    await terminal;
    expect(started).toEqual(['git_is_repository', 'terminal_write']);

    firstLowGate.resolve();
    await Promise.all([low1, low2]);
    expect(started).toEqual([
      'git_is_repository',
      'terminal_write',
      'ssh_is_connected',
    ]);
  });

  it('sends split-endpoint file reads as direct peer commands', async () => {
    const deviceRpc = vi.fn(async (_target: string, commandJson: string) => {
      const parsed = JSON.parse(commandJson) as { cmd: string; path: string };
      expect(parsed).toEqual({
        cmd: 'get_file_info',
        path: '/peer/report.bin',
        session_id: null,
      });
      return JSON.stringify({
        resp: 'file_info',
        name: 'report.bin',
        size: 4,
        mime_type: 'application/octet-stream',
      });
    });
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);

    const response = await adapter.requestPeerCommand({
      cmd: 'get_file_info',
      path: '/peer/report.bin',
      session_id: null,
    });

    expect(response.resp).toBe('file_info');
    expect(deviceRpc).toHaveBeenCalledTimes(1);
  });

  it('retries transient failures for read-only HostInvoke requests', async () => {
    vi.useFakeTimers();
    try {
      const deviceRpc = vi.fn()
        .mockRejectedValueOnce(new Error('relay unavailable'))
        .mockRejectedValueOnce(new Error('gateway timeout'))
        .mockResolvedValueOnce(JSON.stringify({
          resp: 'host_invoke_result',
          ok: true,
          value: [{ id: 'workspace-1' }],
        }));
      const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);

      const request = adapter.request('get_opened_workspaces', { request: {} });
      await vi.advanceTimersByTimeAsync(1500);

      await expect(request).resolves.toEqual([{ id: 'workspace-1' }]);
      expect(deviceRpc).toHaveBeenCalledTimes(3);
      expect(deviceRpc).toHaveBeenCalledWith(
        'peer-1',
        expect.any(String),
        PEER_READ_REQUEST_TIMEOUT_MS,
      );
    } finally {
      vi.useRealTimers();
    }
  });

  it('does not replay mutations when the host has not advertised deduplication', async () => {
    const deviceRpc = vi.fn().mockRejectedValue(new Error('outcome unknown'));
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);

    await expect(
      adapter.request('start_dialog_turn', {
        request: { sessionId: 's1', turnId: 'turn-1' },
      }),
    ).rejects.toThrow('outcome unknown');
    expect(deviceRpc).toHaveBeenCalledTimes(1);
    expect(deviceRpc).toHaveBeenCalledWith(
      'peer-1',
      expect.any(String),
      PEER_MUTATION_REQUEST_TIMEOUT_MS,
    );
  });

  it('recovers an idempotent dialog submission after a transient Relay failure', async () => {
    vi.useFakeTimers();
    try {
      const deviceRpc = vi.fn()
        .mockRejectedValueOnce(new Error('error sending request for url'))
        .mockResolvedValueOnce(JSON.stringify({
          resp: 'host_invoke_result',
          ok: true,
          value: { success: true, message: 'Dialog turn started' },
        }));
      const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc, {
        supportsIdempotentDialogSubmit: true,
      });
      const params = {
        request: {
          sessionId: 'session-1',
          turnId: 'turn-1',
          userInput: 'hello',
        },
      };

      const request = adapter.request('start_dialog_turn', params);
      await vi.advanceTimersByTimeAsync(500);

      await expect(request).resolves.toEqual({
        success: true,
        message: 'Dialog turn started',
      });
      expect(deviceRpc).toHaveBeenCalledTimes(2);
      expect(deviceRpc).toHaveBeenNthCalledWith(
        1,
        'peer-1',
        expect.any(String),
        PEER_MUTATION_REQUEST_TIMEOUT_MS,
      );
      expect(deviceRpc.mock.calls[1][1]).toBe(deviceRpc.mock.calls[0][1]);
    } finally {
      vi.useRealTimers();
    }
  });

  it('bounds a hung read and rejects after its retry budget', async () => {
    vi.useFakeTimers();
    try {
      const deviceRpc = vi.fn(
        () => new Promise<string>(() => {
          // Simulate a Tauri/relay request that never settles.
        }),
      );
      const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);

      const request = adapter.request('list_persisted_sessions_page', {
        request: { workspacePath: '/repo' },
      });
      const rejection = expect(request).rejects.toThrow(
        "Peer request 'list_persisted_sessions_page' timed out",
      );

      await vi.advanceTimersByTimeAsync(
        (PEER_READ_REQUEST_TIMEOUT_MS * 3) + 1500,
      );
      await rejection;
      expect(deviceRpc).toHaveBeenCalledTimes(3);
    } finally {
      vi.useRealTimers();
    }
  });
});

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}
