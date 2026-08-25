import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  PEER_MUTATION_REQUEST_TIMEOUT_MS,
  PEER_READ_MAX_RETRIES,
  PEER_READ_REQUEST_TIMEOUT_MS,
  PEER_RETRY_BASE_DELAY_MS,
  PeerDeviceTransportAdapter,
  PeerProductCommandError,
  PeerTransportClosedError,
  isPeerLocalOnlyCommand,
  isPeerRetryableIdempotentMutation,
  isPeerRetryableReadCommand,
  peerInvokePriorityFor,
} from './peer-device-adapter';
import {
  getTransportSurfaceId,
  resetTransportAdapter,
  setTransportAdapter,
} from './index';
import { TauriTransportAdapter } from './tauri-adapter';
import { isSurfaceChangedError } from '@/infrastructure/peer-device/deviceSurface';

describe('isPeerLocalOnlyCommand', () => {
  it('keeps file-search streaming on the negotiated response path', () => {
    const adapter = new PeerDeviceTransportAdapter('peer-1', vi.fn());
    expect(adapter.supportsSearchStreamEvents()).toBe(false);
    expect(isPeerLocalOnlyCommand('search_filenames')).toBe(false);
  });

  it('keeps local speech capture and model events on the controller device', () => {
    expect(isPeerLocalOnlyCommand('speech_list_models')).toBe(true);
    expect(isPeerLocalOnlyCommand('speech_start_input_session')).toBe(true);
    expect(isPeerLocalOnlyCommand('speech_append_audio_chunk')).toBe(true);
    expect(isPeerLocalOnlyCommand('speech_finish_input_session')).toBe(true);
  });

  it('keeps sleep-prevention controls on the controller computer', () => {
    expect(isPeerLocalOnlyCommand('get_prevent_sleep_enabled')).toBe(true);
    expect(isPeerLocalOnlyCommand('set_prevent_sleep_enabled')).toBe(true);
  });

  it('keeps native main-window geometry control on the controller computer', () => {
    expect(isPeerLocalOnlyCommand('set_main_window_transient_geometry')).toBe(true);
  });

  it('keeps controller app-shell locale on the controller device', () => {
    expect(isPeerLocalOnlyCommand('i18n_get_current_language')).toBe(true);
    expect(isPeerLocalOnlyCommand('i18n_set_language')).toBe(true);
    expect(isPeerLocalOnlyCommand('i18n_get_supported_languages')).toBe(true);
    expect(isPeerLocalOnlyCommand('i18n_get_config')).toBe(true);
    expect(isPeerLocalOnlyCommand('i18n_set_config')).toBe(true);
  });

  it('keeps announcement scheduler and state on the controller device', () => {
    expect(isPeerLocalOnlyCommand('get_pending_announcements')).toBe(true);
    expect(isPeerLocalOnlyCommand('get_announcement_tips')).toBe(true);
    expect(isPeerLocalOnlyCommand('mark_announcement_seen')).toBe(true);
    expect(isPeerLocalOnlyCommand('dismiss_announcement')).toBe(true);
    expect(isPeerLocalOnlyCommand('never_show_announcement')).toBe(true);
    expect(isPeerLocalOnlyCommand('trigger_announcement')).toBe(true);
  });

  it('keeps companion-pet import and preview on the controller device', () => {
    expect(isPeerLocalOnlyCommand('list_agent_companion_pets')).toBe(true);
    expect(isPeerLocalOnlyCommand('import_agent_companion_pet_package')).toBe(true);
    expect(isPeerLocalOnlyCommand('delete_agent_companion_pet_package')).toBe(true);
  });

  it('keeps insights generation, progress and report on the controller device', () => {
    expect(isPeerLocalOnlyCommand('generate_insights')).toBe(true);
    expect(isPeerLocalOnlyCommand('get_latest_insights')).toBe(true);
    expect(isPeerLocalOnlyCommand('load_insights_report')).toBe(true);
    expect(isPeerLocalOnlyCommand('has_insights_data')).toBe(true);
    expect(isPeerLocalOnlyCommand('cancel_insights_generation')).toBe(true);
  });

  it('keeps IDE control result reporting on the controller device', () => {
    expect(isPeerLocalOnlyCommand('report_ide_control_result')).toBe(true);
  });

  it('keeps controller webview/devtools/desktop-pet/diagnostics on the controller device', () => {
    // These previously hit the local Tauri host via dynamic invoke(); routing
    // them to a peer would regress (peer host does not implement them, and
    // they drive the controller's own embedded surfaces).
    expect(isPeerLocalOnlyCommand('browser_webview_create')).toBe(true);
    expect(isPeerLocalOnlyCommand('browser_webview_eval')).toBe(true);
    expect(isPeerLocalOnlyCommand('browser_webview_navigate')).toBe(true);
    expect(isPeerLocalOnlyCommand('browser_webview_reload')).toBe(true);
    expect(isPeerLocalOnlyCommand('browser_webview_set_bounds')).toBe(true);
    expect(isPeerLocalOnlyCommand('debug_devtools_available')).toBe(true);
    expect(isPeerLocalOnlyCommand('debug_open_devtools')).toBe(true);
    expect(isPeerLocalOnlyCommand('resize_agent_companion_desktop_pet')).toBe(true);
    expect(isPeerLocalOnlyCommand('show_agent_companion_desktop_pet')).toBe(true);
    expect(isPeerLocalOnlyCommand('hide_agent_companion_desktop_pet')).toBe(true);
    expect(isPeerLocalOnlyCommand('append_flow_chat_diagnostics')).toBe(true);
  });

  it('routes Browser Control and Computer Use to the host that runs the Tool', () => {
    // Browser Control / Computer Use run the agent Tool, so they must reach the
    // host that executes it: Desktop Peer B bridges them to its own webview
    // (reads B's browser/OS), CLI Peer refuses them in deny.rs, and the UI
    // gates the section on host type. They must NOT be LOCAL_ONLY, otherwise
    // config is written to the peer while status/launch/repair run on the
    // controller (split across hosts). See PR #2428 review #4 issue #1.
    expect(isPeerLocalOnlyCommand('browser_control_launch')).toBe(false);
    expect(isPeerLocalOnlyCommand('browser_control_list_browsers')).toBe(false);
    expect(isPeerLocalOnlyCommand('browser_control_get_status')).toBe(false);
    expect(isPeerLocalOnlyCommand('browser_control_restart_with_cdp')).toBe(false);
    expect(isPeerLocalOnlyCommand('browser_control_enable_default_cdp')).toBe(false);
    expect(isPeerLocalOnlyCommand('computer_use_get_status')).toBe(false);
    expect(isPeerLocalOnlyCommand('computer_use_request_permissions')).toBe(false);
    expect(isPeerLocalOnlyCommand('computer_use_open_system_settings')).toBe(false);
    // But the embedded browser_webview_* surface stays controller-local.
    expect(isPeerLocalOnlyCommand('browser_webview_create')).toBe(true);
  });

  it('keeps file-tree path checks routed to the peer surface', () => {
    // check_path_exists is the one CLI-Peer-supported routed command: the path
    // comes from the rendered surface's file tree, so it must stay peer-routed.
    expect(isPeerLocalOnlyCommand('check_path_exists')).toBe(false);
    expect(peerInvokePriorityFor('check_path_exists')).toBe('high');
  });
});

describe('peerInvokePriorityFor', () => {
  it('ranks session hydrate commands high', () => {
    expect(peerInvokePriorityFor('restore_session_view')).toBe('high');
    expect(peerInvokePriorityFor('load_session_turn_window')).toBe('high');
    expect(peerInvokePriorityFor('list_persisted_sessions_page')).toBe('high');
    expect(peerInvokePriorityFor('initialize_workspace_startup_state')).toBe('high');
    expect(peerInvokePriorityFor('start_dialog_turn')).toBe('high');
    expect(peerInvokePriorityFor('rollback_session_to_turn')).toBe('high');
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

  it('ranks per-tool cancel high so Interrupt is not queued behind normal work', () => {
    // A long-running shell keeps producing side effects until the cancel
    // reaches the host; it must take the reserved high-priority slot, not the
    // normal queue that saturated reads/mutations can block. See PR #2428 #2.
    expect(peerInvokePriorityFor('cancel_tool')).toBe('high');
    expect(peerInvokePriorityFor('cancel_dialog_turn')).toBe('high');
  });

  it('retries only idempotent Peer reads', () => {
    expect(isPeerRetryableReadCommand('list_persisted_sessions_page')).toBe(true);
    expect(isPeerRetryableReadCommand('get_opened_workspaces')).toBe(true);
    expect(isPeerRetryableReadCommand('restore_session_view')).toBe(true);
    expect(isPeerRetryableReadCommand('load_session_turn_window')).toBe(true);
    expect(isPeerRetryableReadCommand('start_dialog_turn')).toBe(false);
    expect(isPeerRetryableReadCommand('delete_session')).toBe(false);
    expect(isPeerRetryableReadCommand('respond_permission')).toBe(false);
  });

  it('does not retry side-effecting announcement get_* commands', () => {
    // These run the scheduler (mutate app_open_count + persist) and must never
    // be auto-retried by the peer read path, where retries would multiply the
    // side effect.
    expect(isPeerRetryableReadCommand('get_pending_announcements')).toBe(false);
    expect(isPeerRetryableReadCommand('get_announcement_tips')).toBe(false);
  });

  it('retries Browser Control / Computer Use status reads but not their mutations', () => {
    // The pure reads start with `browser_`/`computer_`, not `get_`/`read_`/
    // `list_`, so they are listed explicitly; the side-effecting commands in
    // the same family stay non-retryable mutations. See PR #2428 round 5 #3.
    expect(isPeerRetryableReadCommand('browser_control_get_status')).toBe(true);
    expect(isPeerRetryableReadCommand('browser_control_list_browsers')).toBe(true);
    expect(isPeerRetryableReadCommand('computer_use_get_status')).toBe(true);
    expect(isPeerRetryableReadCommand('browser_control_launch')).toBe(false);
    expect(isPeerRetryableReadCommand('browser_control_restart_with_cdp')).toBe(false);
    expect(isPeerRetryableReadCommand('browser_control_enable_default_cdp')).toBe(false);
    expect(isPeerRetryableReadCommand('computer_use_request_permissions')).toBe(false);
    expect(isPeerRetryableReadCommand('computer_use_open_system_settings')).toBe(false);
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

  it('dispatches cancel_tool immediately when the non-high slot is busy', async () => {
    // The Terminal Interrupt button calls cancel_tool; a long-running shell on
    // the peer keeps producing side effects until the cancel lands. cancel_tool
    // is high-priority, so it takes the reserved high slot and dispatches even
    // while a normal-priority mutation occupies the single non-high slot.
    const started: string[] = [];
    const normalGate = createDeferred<void>();

    const deviceRpc = vi.fn(async (_target: string, commandJson: string) => {
      const parsed = JSON.parse(commandJson) as { command: string };
      started.push(parsed.command);
      if (parsed.command === 'set_config') {
        await normalGate.promise;
      }
      return JSON.stringify({
        resp: 'host_invoke_result',
        ok: true,
        value: parsed.command === 'set_config' ? {} : { ok: true },
      });
    });

    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc, {}, 2);
    await adapter.connect();

    // One normal-priority mutation occupies the single non-high slot
    // (maxConcurrent 2 → one slot reserved for high).
    const normal1 = adapter.request('set_config', { request: { path: 'a' } });
    await Promise.resolve();
    await Promise.resolve();
    expect(started).toEqual(['set_config']);
    expect(adapter.getActiveCountsForTest()).toEqual({
      total: 1,
      high: 0,
      normal: 1,
      low: 0,
    });

    // Interrupt fires while the normal slot is busy. It must start on the
    // reserved high slot without waiting for the mutation to finish.
    const cancel = adapter.request('cancel_tool', { request: { toolUseId: 'tu-1' } });
    await Promise.resolve();
    await Promise.resolve();
    expect(started).toEqual(['set_config', 'cancel_tool']);
    expect(adapter.getActiveCountsForTest().high).toBe(1);

    await cancel;
    normalGate.resolve();
    await normal1;
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

  it('rejects targeted rollback when the Peer host lacks the negotiated capability', async () => {
    const deviceRpc = vi.fn();
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);

    await expect(adapter.request('rollback_session_to_turn', {
      request: {
        workspacePath: '/repo',
        sessionId: 'session-1',
        targetTurnId: 'turn-7',
      },
    })).rejects.toEqual(expect.objectContaining<Partial<PeerProductCommandError>>({
      name: 'PeerProductCommandError',
      message: 'The connected Peer host does not support targeted Session rollback',
    }));
    expect(deviceRpc).not.toHaveBeenCalled();
  });

  it('forwards targeted rollback after capability negotiation', async () => {
    const deviceRpc = vi.fn().mockResolvedValue(JSON.stringify({
      resp: 'host_invoke_result',
      ok: true,
      value: { status: 'completed', sessionId: 'session-1' },
    }));
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc, {
      supportsTargetedSessionRollback: true,
    });

    await expect(adapter.request('rollback_session_to_turn', {
      request: {
        workspacePath: '/repo',
        sessionId: 'session-1',
        targetTurnId: 'turn-7',
      },
    })).resolves.toEqual({ status: 'completed', sessionId: 'session-1' });
    expect(deviceRpc).toHaveBeenCalledTimes(1);
  });

  it('rejects usage statistics before RPC when the Peer host lacks the capability', async () => {
    const deviceRpc = vi.fn();
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);

    await expect(adapter.request('get_token_usage_statistics', {
      request: { timeRange: 'today', granularity: 'hour' },
    })).rejects.toEqual(expect.objectContaining<Partial<PeerProductCommandError>>({
      name: 'PeerProductCommandError',
      message: expect.stringContaining('token_usage_statistics_unsupported'),
    }));
    expect(deviceRpc).not.toHaveBeenCalled();
  });

  it('forwards usage statistics after capability negotiation', async () => {
    const statistics = { totalRequests: 3, totalTokens: 120 };
    const deviceRpc = vi.fn().mockResolvedValue(JSON.stringify({
      resp: 'host_invoke_result',
      ok: true,
      value: statistics,
    }));
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);
    adapter.setHostCapabilities({
      supportsIdempotentDialogSubmit: false,
      supportsTargetedSessionRollback: false,
      supportsTokenUsageStatistics: true,
    });

    await expect(adapter.request('get_token_usage_statistics', {
      request: { timeRange: 'today', granularity: 'hour' },
    })).resolves.toEqual(statistics);
    expect(deviceRpc).toHaveBeenCalledTimes(1);
  });

  it('preserves a session conflict returned by the Peer Host without replaying it', async () => {
    const deviceRpc = vi.fn().mockResolvedValue(JSON.stringify({
      resp: 'host_invoke_result',
      ok: false,
      error: 'session_in_use: Session is already open for writing: session-1',
    }));
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);

    await expect(adapter.request('ensure_coordinator_session', {
      request: { sessionId: 'session-1', workspacePath: '/repo' },
    })).rejects.toEqual(expect.objectContaining<Partial<PeerProductCommandError>>({
      name: 'PeerProductCommandError',
      message: 'session_in_use: Session is already open for writing: session-1',
    }));
    expect(deviceRpc).toHaveBeenCalledTimes(1);
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
        (PEER_READ_REQUEST_TIMEOUT_MS * (PEER_READ_MAX_RETRIES + 1))
          + (PEER_RETRY_BASE_DELAY_MS * ((2 ** PEER_READ_MAX_RETRIES) - 1)),
      );
      await rejection;
      expect(deviceRpc).toHaveBeenCalledTimes(PEER_READ_MAX_RETRIES + 1);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('PeerDeviceTransportAdapter disposal', () => {
  it('settles queued requests instead of dropping them', async () => {
    const gate = createDeferred<string>();
    const deviceRpc = vi.fn(() => gate.promise);
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc, {}, 1);
    await adapter.connect();

    const inFlight = captureOutcome(adapter.request('get_opened_workspaces', { request: {} }));
    const queued = captureOutcome(adapter.request('list_persisted_sessions_page', {
      request: { workspacePath: '/repo' },
    }));
    expect(adapter.getQueueDepthsForTest().high).toBe(1);

    await adapter.disconnect();
    await flushMicrotasks();

    // Both the waiting caller and the one already on the wire must hear back;
    // a silent drop is what left session/history loads spinning forever.
    expect(queued.settled).toBe(true);
    expect(isSurfaceChangedError(queued.value)).toBe(true);
    expect(queued.value).toBeInstanceOf(PeerTransportClosedError);
    expect(inFlight.settled).toBe(true);
    expect(isSurfaceChangedError(inFlight.value)).toBe(true);
    expect(adapter.getQueueDepthsForTest()).toEqual({ high: 0, normal: 0, low: 0 });

    gate.resolve(hostInvokeOk([]));
  });

  it('keeps concurrency accounting correct across disposal', async () => {
    const gate = createDeferred<string>();
    const deviceRpc = vi.fn(() => gate.promise);
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc, {}, 2);
    await adapter.connect();

    const inFlight = captureOutcome(adapter.request('get_opened_workspaces', { request: {} }));
    expect(adapter.getActiveCountsForTest()).toMatchObject({ total: 1, high: 1 });

    await adapter.disconnect();

    // The request still occupies its transport slot: rewriting the count to
    // zero underneath it made its later settle decrement a fresh count, so the
    // limiter believed it had a free slot it did not have.
    expect(adapter.getActiveCountsForTest()).toMatchObject({ total: 1, high: 1 });

    gate.resolve(hostInvokeOk([]));
    await flushMicrotasks();

    expect(adapter.getActiveCountsForTest()).toMatchObject({ total: 0, high: 0 });
    expect(inFlight.settled).toBe(true);
  });

  it('refuses to reopen the peer data plane after disposal', async () => {
    const deviceRpc = vi.fn().mockResolvedValue(hostInvokeOk(true));
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);
    await adapter.connect();
    await adapter.disconnect();

    expect(adapter.isDisposed()).toBe(true);
    expect(adapter.isConnected()).toBe(false);
    await expect(adapter.request('get_opened_workspaces', { request: {} }))
      .rejects.toBeInstanceOf(PeerTransportClosedError);
    expect(deviceRpc).not.toHaveBeenCalled();
  });
});

describe('surface-aware transport registry', () => {
  afterEach(async () => {
    await resetTransportAdapter();
  });

  it('records the surface each registered adapter serves', () => {
    const adapter = new PeerDeviceTransportAdapter('peer-1', vi.fn());
    setTransportAdapter(adapter);
    expect(getTransportSurfaceId()).toBe('peer-1');

    setTransportAdapter(new TauriTransportAdapter());
    expect(getTransportSurfaceId()).toBe('local');
  });

  it('rejects a request that lands after its surface stopped being rendered', async () => {
    const gate = createDeferred<string>();
    const onHostInvokeTransportFailure = vi.fn();
    const deviceRpc = vi.fn(() => gate.promise);
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc, {
      onHostInvokeTransportFailure,
    });
    setTransportAdapter(adapter);

    const pending = captureOutcome(adapter.request('list_persisted_sessions_page', {
      request: { workspacePath: '/repo' },
    }));

    setTransportAdapter(new TauriTransportAdapter());
    await flushMicrotasks();

    // The activation boundary settles the caller; it does not wait for a weak
    // relay request to answer or exhaust its retry budget.
    expect(pending.settled).toBe(true);
    expect(isSurfaceChangedError(pending.value)).toBe(true);
    expect(onHostInvokeTransportFailure).not.toHaveBeenCalled();

    gate.resolve(hostInvokeOk([{ id: 'peer-session' }]));
    await flushMicrotasks();

    expect(pending.settled).toBe(true);
    expect(isSurfaceChangedError(pending.value)).toBe(true);
    // A switch is not a weak link; counting it as one would degrade a healthy peer.
    expect(onHostInvokeTransportFailure).not.toHaveBeenCalled();
  });

  it('invalidates late work when the same adapter is rebound for a new activation', async () => {
    const gate = createDeferred<string>();
    const adapter = new PeerDeviceTransportAdapter('peer-1', vi.fn(() => gate.promise));
    await adapter.connect();
    setTransportAdapter(adapter);

    const pending = captureOutcome(adapter.request('list_persisted_sessions_page', {
      request: { workspacePath: '/repo' },
    }));

    // The surface controller uses a same-adapter rebind to abort A#1 before
    // the next target can commit. A#1's answer must not survive into A#2.
    setTransportAdapter(adapter);
    gate.resolve(hostInvokeOk([{ id: 'stale-session' }]));
    await flushMicrotasks();

    expect(pending.settled).toBe(true);
    expect(isSurfaceChangedError(pending.value)).toBe(true);
  });

  it('rejects requests issued through a retired adapter without calling the peer', async () => {
    const deviceRpc = vi.fn().mockResolvedValue(hostInvokeOk([]));
    const adapter = new PeerDeviceTransportAdapter('peer-1', deviceRpc);
    setTransportAdapter(adapter);
    setTransportAdapter(new TauriTransportAdapter());

    const error = await adapter
      .request('list_persisted_sessions_page', { request: { workspacePath: '/repo' } })
      .then(() => undefined, (rejection: unknown) => rejection);

    expect(isSurfaceChangedError(error)).toBe(true);
    expect(deviceRpc).not.toHaveBeenCalled();
    // The attachment survives the switch, so rendering the peer again revives it.
    setTransportAdapter(adapter);
    await expect(adapter.request('list_persisted_sessions_page', {
      request: { workspacePath: '/repo' },
    })).resolves.toEqual([]);
  });
});

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

function hostInvokeOk(value: unknown): string {
  return JSON.stringify({ resp: 'host_invoke_result', ok: true, value });
}

/**
 * Observe a request without awaiting it, so a promise that never settles fails
 * the assertion instead of hanging the test until its timeout.
 */
function captureOutcome<T>(request: Promise<T>) {
  const outcome: { settled: boolean; value: unknown } = { settled: false, value: undefined };
  void request.then(
    (value) => {
      outcome.settled = true;
      outcome.value = value;
    },
    (error: unknown) => {
      outcome.settled = true;
      outcome.value = error;
    },
  );
  return outcome;
}

function flushMicrotasks(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}
