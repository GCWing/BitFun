import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  PeerConnectionManager,
  type PeerConnectionState,
} from './PeerConnectionManager';

const KEEPALIVE_MS = 20_000;
const RECONNECT_BASE_MS = 1_000;
const RECONNECT_MAX_MS = 4_000;

describe('PeerConnectionManager attach', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('negotiates capabilities and attaches control in one handshake', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);

    const connection = await manager.connect('peer-1', 'Studio');

    expect(rpc.commands()).toEqual(['peer_mode_ping', 'peer_control_attach']);
    expect(rpc.args('peer_control_attach')).toEqual({ controller_device_id: 'controller-1' });
    expect(connection.surfaceId).toBe('peer-1');
    expect(connection.getState()).toMatchObject({
      deviceName: 'Studio',
      health: 'ready',
      capabilities: {
        idempotentDialogSubmit: true,
        targetedSessionRollback: false,
        tokenUsageStatistics: true,
      },
    });
    expect(manager.get('peer-1')).toBe(connection);
  });

  it('reports new capability fields as null (unknown) when the host omits them', async () => {
    // An older host's peer_mode_ping does not advertise cancel_tool /
    // tool_catalog; coercing those to `false` would hide a working capability
    // on an older Desktop (Interrupt button / tool list). They must parse to
    // `null` so consumers stay optimistic. See PR #2428 #4.
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);

    const connection = await manager.connect('peer-1', 'Studio');
    const caps = connection.getState().capabilities;
    expect(caps.cancelTool).toBeNull();
    expect(caps.toolCatalog).toBeNull();
  });

  it('parses advertised new capability fields as true', async () => {
    const manager = new PeerConnectionManager({
      deviceRpc: async () => JSON.stringify({
        resp: 'host_invoke_result',
        ok: true,
        value: {
          capabilities: {
            cancel_tool: true,
            tool_catalog: true,
          },
        },
      }),
      getControllerDeviceId: async () => 'controller-1',
    });

    const connection = await manager.connect('peer-1', 'Studio');
    const caps = connection.getState().capabilities;
    expect(caps.cancelTool).toBe(true);
    expect(caps.toolCatalog).toBe(true);
  });

  it('leaves nothing attached when the handshake fails', async () => {
    const rpc = createRpc({ failCommands: new Set(['peer_control_attach']) });
    const manager = createManager(rpc.deviceRpc);

    await expect(manager.connect('peer-1', 'Studio')).rejects.toThrow('peer refused');

    expect(manager.get('peer-1')).toBeUndefined();
    expect(manager.list()).toEqual([]);
  });

  it('reuses one attachment for concurrent callers', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);

    const [first, second] = await Promise.all([
      manager.connect('peer-1', 'Studio'),
      manager.connect('peer-1', 'Studio'),
    ]);

    expect(first).toBe(second);
    expect(rpc.commands().filter(c => c === 'peer_control_attach')).toHaveLength(1);
  });

  it('orders detach after a handshake that was disposed in flight', async () => {
    const ping = deferred<string>();
    const commands: string[] = [];
    const deviceRpc = vi.fn(async (_target: string, commandJson: string): Promise<string> => {
      const command = (JSON.parse(commandJson) as { command?: string }).command ?? 'unknown';
      commands.push(command);
      if (command === 'peer_mode_ping') {
        return ping.promise;
      }
      return JSON.stringify({ resp: 'host_invoke_result', ok: true, value: null });
    });
    const manager = createManager(deviceRpc);

    const connecting = manager.connect('peer-1', 'Studio');
    await vi.waitFor(() => expect(commands).toEqual(['peer_mode_ping']));
    const disposing = manager.dispose('peer-1');
    ping.resolve(JSON.stringify({
      resp: 'host_invoke_result',
      ok: true,
      value: { capabilities: {} },
    }));

    await expect(connecting).rejects.toThrow('disposed during attachment');
    await disposing;
    expect(commands).toEqual(['peer_mode_ping', 'peer_control_detach']);
    expect(manager.get('peer-1')).toBeUndefined();
  });
});

describe('PeerConnectionManager health', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('degrades on a single missed ping instead of tearing the peer down', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);
    const seen = observe(manager);
    await manager.connect('peer-1', 'Studio');

    rpc.failNext(1);
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS);

    expect(manager.get('peer-1')?.getState()).toMatchObject({
      health: 'degraded',
      consecutiveFailures: 1,
    });
    expect(seen.healthTimeline()).toEqual(['connecting', 'ready', 'degraded']);
  });

  it('reconnects with exponential backoff while degraded', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc, { maxKeepaliveFailures: 4 });
    await manager.connect('peer-1', 'Studio');

    rpc.failNext(2);
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS);
    const afterFirstFailure = rpc.commands().length;

    // 1s, then 2s: nothing is retried before that attempt's delay elapses.
    await vi.advanceTimersByTimeAsync(RECONNECT_BASE_MS - 1);
    expect(rpc.commands().length).toBe(afterFirstFailure);
    await vi.advanceTimersByTimeAsync(1);
    expect(rpc.commands().length).toBe(afterFirstFailure + 1);

    await vi.advanceTimersByTimeAsync(RECONNECT_BASE_MS * 2 - 1);
    expect(rpc.commands().length).toBe(afterFirstFailure + 1);
    await vi.advanceTimersByTimeAsync(1);

    // The third attempt answers, so the peer recovers and the delay resets.
    expect(manager.get('peer-1')?.getState()).toMatchObject({
      health: 'ready',
      consecutiveFailures: 0,
    });
    // A recovered peer re-attaches: the host may have pruned us during the gap.
    expect(rpc.commands().filter(c => c === 'peer_control_attach')).toHaveLength(2);
  });

  it('caps the reconnect delay', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc, { maxKeepaliveFailures: 6 });
    await manager.connect('peer-1', 'Studio');

    rpc.failNext(5);
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS);
    // Failures 1..4 back off 1s, 2s, 4s, 4s — the fourth is already capped.
    await vi.advanceTimersByTimeAsync(RECONNECT_BASE_MS + RECONNECT_BASE_MS * 2 + RECONNECT_MAX_MS);
    const beforeCappedAttempt = rpc.commands().length;

    await vi.advanceTimersByTimeAsync(RECONNECT_MAX_MS);

    expect(rpc.commands().length).toBe(beforeCappedAttempt + 1);
    expect(manager.get('peer-1')?.getState().consecutiveFailures).toBe(5);
  });

  it('loses a peer after repeated failures and stops retrying', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);
    await manager.connect('peer-1', 'Studio');

    rpc.failAll();
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS + RECONNECT_BASE_MS + RECONNECT_BASE_MS * 2);

    expect(manager.get('peer-1')?.getState()).toMatchObject({
      health: 'lost',
      lostReason: 'keepalive',
      consecutiveFailures: 3,
    });

    const afterLoss = rpc.commands().length;
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS * 5);
    expect(rpc.commands().length).toBe(afterLoss);
  });

  it('loses a peer that dropped out of account presence', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);
    await manager.connect('peer-1', 'Studio');

    manager.reportPresence(['peer-2']);

    expect(manager.get('peer-1')?.getState()).toMatchObject({
      health: 'lost',
      lostReason: 'presence',
    });

    const afterLoss = rpc.commands().length;
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS * 3);
    expect(rpc.commands().length).toBe(afterLoss);
  });

  it('treats successful product traffic as proof the link is alive', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);
    const connection = await manager.connect('peer-1', 'Studio');

    rpc.failNext(1);
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS);
    expect(connection.getState().health).toBe('degraded');

    await connection.adapter.request('get_opened_workspaces', { request: {} });

    expect(connection.getState()).toMatchObject({ health: 'ready', consecutiveFailures: 0 });
    await vi.waitFor(() => {
      expect(rpc.commands().filter(c => c === 'peer_control_attach')).toHaveLength(2);
    });
  });

  it('degrades after a product transport failure instead of waiting for keepalive', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);
    const connection = await manager.connect('peer-1', 'Studio');

    rpc.failNext(1);
    await expect(connection.adapter.request('set_config', {
      request: { path: 'ui.theme', value: 'dark' },
    })).rejects.toThrow('relay unavailable');

    expect(connection.getState()).toMatchObject({
      health: 'degraded',
      consecutiveFailures: 1,
    });
  });
});

describe('PeerConnectionManager disposal', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('detaches, disposes the transport, and drops the entry', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);
    const connection = await manager.connect('peer-1', 'Studio');

    await manager.dispose('peer-1');

    expect(rpc.commands()).toContain('peer_control_detach');
    expect(connection.adapter.isDisposed()).toBe(true);
    expect(manager.get('peer-1')).toBeUndefined();
    expect(manager.list()).toEqual([]);

    const afterDispose = rpc.commands().length;
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS * 3);
    expect(rpc.commands().length).toBe(afterDispose);
  });

  it('skips the detach RPC for a peer that is already gone', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);
    await manager.connect('peer-1', 'Studio');

    await manager.dispose('peer-1', { notifyPeer: false });

    expect(rpc.commands()).not.toContain('peer_control_detach');
  });

  it('settles transport work still waiting on a disposed peer', async () => {
    const rpc = createRpc();
    const manager = createManager(rpc.deviceRpc);
    const connection = await manager.connect('peer-1', 'Studio');

    rpc.hangNext();
    const pending = connection.adapter.request('list_persisted_sessions_page', {
      request: { workspacePath: '/repo' },
    });
    const settled = pending.then(() => 'resolved', () => 'rejected');

    await manager.dispose('peer-1');

    await expect(settled).resolves.toBe('rejected');
  });

  it('orders detach after a recovery re-attach already in flight', async () => {
    const reattach = deferred<string>();
    const commands: string[] = [];
    let failNextPing = false;
    let attachCount = 0;
    const deviceRpc = vi.fn(async (_target: string, commandJson: string): Promise<string> => {
      const command = (JSON.parse(commandJson) as { command?: string }).command ?? 'unknown';
      commands.push(command);
      if (command === 'peer_mode_ping' && failNextPing) {
        failNextPing = false;
        throw new Error('relay unavailable');
      }
      if (command === 'peer_control_attach' && ++attachCount === 2) {
        return reattach.promise;
      }
      if (command === 'peer_mode_ping') {
        return JSON.stringify({
          resp: 'host_invoke_result',
          ok: true,
          value: { capabilities: {} },
        });
      }
      return JSON.stringify({ resp: 'host_invoke_result', ok: true, value: null });
    });
    const manager = createManager(deviceRpc);
    const connection = await manager.connect('peer-1', 'Studio');

    failNextPing = true;
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS);
    await connection.adapter.request('get_opened_workspaces', { request: {} });
    await vi.waitFor(() => {
      expect(commands.filter(command => command === 'peer_control_attach')).toHaveLength(2);
    });

    const disposing = manager.dispose('peer-1');
    await Promise.resolve();
    expect(commands).not.toContain('peer_control_detach');

    reattach.resolve(JSON.stringify({ resp: 'host_invoke_result', ok: true, value: null }));
    await disposing;
    expect(commands.at(-1)).toBe('peer_control_detach');
  });

  it('publishes a snapshot when a ready peer reports changed capabilities', async () => {
    // A peer that stays `ready` but whose host changes capabilities mid-session
    // (e.g. a restart on a different build) must push a fresh snapshot, or the
    // UI keeps gating on stale capabilities. See PR #2428 #6.
    let capabilities: Record<string, unknown> = { cancel_tool: false, tool_catalog: false };
    const manager = new PeerConnectionManager({
      deviceRpc: async (_target, commandJson) => {
        const parsed = JSON.parse(commandJson) as { command?: string };
        if (parsed.command === 'peer_mode_ping') {
          return JSON.stringify({
            resp: 'host_invoke_result',
            ok: true,
            value: { capabilities },
          });
        }
        return JSON.stringify({ resp: 'host_invoke_result', ok: true, value: null });
      },
      getControllerDeviceId: async () => 'controller-1',
      keepaliveIntervalMs: KEEPALIVE_MS,
    });

    const connection = await manager.connect('peer-1', 'Studio');
    expect(connection.getState().capabilities.cancelTool).toBe(false);

    const snapshots: number[] = [];
    manager.subscribe(states => snapshots.push(states.length));

    // Host restarts advertising cancel_tool now available, without going degraded.
    capabilities = { cancel_tool: true, tool_catalog: false };
    await vi.advanceTimersByTimeAsync(KEEPALIVE_MS);

    await vi.waitFor(() => {
      expect(connection.getState().capabilities.cancelTool).toBe(true);
    });
    // A snapshot was published for the capability change while staying ready.
    expect(snapshots.length).toBeGreaterThan(0);
  });
});

function createManager(
  deviceRpc: ReturnType<typeof createRpc>['deviceRpc'],
  overrides: { maxKeepaliveFailures?: number } = {},
): PeerConnectionManager {
  return new PeerConnectionManager({
    deviceRpc,
    getControllerDeviceId: async () => 'controller-1',
    keepaliveIntervalMs: KEEPALIVE_MS,
    reconnectBaseDelayMs: RECONNECT_BASE_MS,
    reconnectMaxDelayMs: RECONNECT_MAX_MS,
    maxKeepaliveFailures: overrides.maxKeepaliveFailures ?? 3,
  });
}

interface RpcCall {
  command: string;
  args: Record<string, unknown>;
}

function createRpc(options: { failCommands?: Set<string> } = {}) {
  const calls: RpcCall[] = [];
  let remainingFailures = 0;
  let hangNext = false;

  const deviceRpc = vi.fn(async (_target: string, commandJson: string): Promise<string> => {
    const parsed = JSON.parse(commandJson) as {
      command?: string;
      args?: Record<string, unknown>;
    };
    const command = parsed.command ?? 'unknown';
    calls.push({ command, args: parsed.args ?? {} });

    if (hangNext) {
      hangNext = false;
      return new Promise<string>(() => {
        // Models a relay request that never settles.
      });
    }
    if (options.failCommands?.has(command)) {
      throw new Error('peer refused');
    }
    if (remainingFailures > 0) {
      remainingFailures -= 1;
      throw new Error('relay unavailable');
    }
    if (command === 'peer_mode_ping') {
      return JSON.stringify({
        resp: 'host_invoke_result',
        ok: true,
        value: {
          capabilities: {
            idempotent_dialog_submit: true,
            token_usage_statistics: true,
          },
        },
      });
    }
    return JSON.stringify({ resp: 'host_invoke_result', ok: true, value: null });
  });

  return {
    deviceRpc,
    commands: () => calls.map(call => call.command),
    args: (command: string) => calls.find(call => call.command === command)?.args,
    failNext: (count: number) => {
      remainingFailures = count;
    },
    failAll: () => {
      remainingFailures = Number.MAX_SAFE_INTEGER;
    },
    hangNext: () => {
      hangNext = true;
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(res => {
    resolve = res;
  });
  return { promise, resolve };
}

function observe(manager: PeerConnectionManager) {
  const updates: PeerConnectionState[][] = [];
  manager.subscribe(states => updates.push(states.map(state => ({ ...state }))));
  return {
    healthTimeline: () => updates
      .map(states => states[0]?.health)
      .filter((health, index, all): health is PeerConnectionState['health'] =>
        health !== undefined && health !== all[index - 1]),
  };
}

describe('PeerConnectionManager presence recovery', () => {
  it('clears a presence-lost attachment once the device is reachable again', async () => {
    // `lost` is terminal and `connect` refuses a lost entry, so one presence
    // blip during a burst of switching used to strand a healthy device for the
    // rest of the session.
    const manager = new PeerConnectionManager({
      deviceRpc: async () => JSON.stringify({
        resp: 'host_invoke_result',
        ok: true,
        value: { capabilities: {} },
      }),
      getControllerDeviceId: async () => 'controller-1',
    });

    await manager.connect('device-b', 'B');
    manager.reportPresence([]);
    expect(manager.get('device-b')?.getState().health).toBe('lost');

    manager.reportPresence(['device-b']);

    expect(manager.has('device-b')).toBe(false);
    await expect(manager.connect('device-b', 'B')).resolves.toBeDefined();
  });

});
