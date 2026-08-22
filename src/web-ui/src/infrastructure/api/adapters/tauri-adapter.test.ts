import { beforeEach, describe, expect, it, vi } from 'vitest';
import { isExpectedTauriRequestError, TauriTransportAdapter } from './tauri-adapter';
import {
  beginRuntimeSessionAttachment,
  resetRuntimeSessionEventGateForTest,
  RUNTIME_EVENT_CURSOR_KEY,
  RUNTIME_EVENT_STREAM_ID_KEY,
} from '@/infrastructure/peer-device/runtimeSessionEventGate';
import { setActiveSurfaceDeviceId } from '@/infrastructure/peer-device/deviceSurfaceRouting';

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

describe('Tauri adapter expected errors', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetRuntimeSessionEventGateForTest();
    setActiveSurfaceDeviceId(null);
  });

  it('classifies optional get_config not found as expected', () => {
    expect(isExpectedTauriRequestError(
      'get_config',
      {
        request: {
          path: 'font',
          skipRetryOnNotFound: true,
        },
      },
      new Error("Config path not found: 'font'")
    )).toBe(true);
  });

  it('does not hide non-optional get_config failures', () => {
    expect(isExpectedTauriRequestError(
      'get_config',
      {
        request: {
          path: 'font',
        },
      },
      new Error("Config path not found: 'font'")
    )).toBe(false);
  });

  it('records adapter init and invoke timings for each request', async () => {
    invokeMock.mockResolvedValueOnce({ ok: true });
    const adapter = new TauriTransportAdapter();
    const timing: {
      adapterInitDurationMs?: number;
      invokeDurationMs?: number;
      transportDurationMs?: number;
    } = {};

    await expect(adapter.request('list_persisted_sessions_page', {
      request: { limit: 5 },
    }, timing)).resolves.toEqual({ ok: true });

    expect(invokeMock).toHaveBeenCalledWith('list_persisted_sessions_page', {
      request: { limit: 5 },
    });
    expect(timing.adapterInitDurationMs).toEqual(expect.any(Number));
    expect(timing.invokeDurationMs).toEqual(expect.any(Number));
    expect(timing.transportDurationMs).toEqual(expect.any(Number));
  });

  it('records invoke timing when a request rejects', async () => {
    invokeMock.mockRejectedValueOnce(new Error("Config path not found: 'font'"));
    const adapter = new TauriTransportAdapter();
    const timing: {
      adapterInitDurationMs?: number;
      invokeDurationMs?: number;
      transportDurationMs?: number;
    } = {};

    await expect(adapter.request('get_config', {
      request: {
        path: 'font',
        skipRetryOnNotFound: true,
      },
    }, timing)).rejects.toThrow("Config path not found: 'font'");

    expect(timing.adapterInitDurationMs).toEqual(expect.any(Number));
    expect(timing.invokeDurationMs).toEqual(expect.any(Number));
    expect(timing.transportDurationMs).toEqual(expect.any(Number));
  });

  it('routes a positioned Tauri event once across adapter instances', async () => {
    const handlers = new Map<string, (event: { payload: unknown }) => void>();
    const underlyingUnlisten = vi.fn();
    listenMock.mockImplementation(async (event: string, handler: (event: { payload: unknown }) => void) => {
      handlers.set(event, handler);
      return underlyingUnlisten;
    });

    const firstAdapter = new TauriTransportAdapter();
    const secondAdapter = new TauriTransportAdapter();
    const first = vi.fn();
    const second = vi.fn();
    const unlistenFirst = firstAdapter.listen('agentic://dialog-turn-completed', first);
    await firstAdapter.waitForListenerRegistrations();
    const unlistenSecond = secondAdapter.listen('agentic://dialog-turn-completed', second);
    await secondAdapter.waitForListenerRegistrations();

    expect(listenMock).toHaveBeenCalledTimes(1);
    handlers.get('agentic://dialog-turn-completed')?.({
      payload: {
        sessionId: 'session-1',
        turnId: 'turn-1',
        success: true,
        [RUNTIME_EVENT_STREAM_ID_KEY]: 'runtime-1',
        [RUNTIME_EVENT_CURSOR_KEY]: 14,
      },
    });

    const expectedPayload = {
      sessionId: 'session-1',
      turnId: 'turn-1',
      success: true,
    };
    expect(first).toHaveBeenCalledOnce();
    expect(first).toHaveBeenCalledWith(expectedPayload);
    expect(second).toHaveBeenCalledOnce();
    expect(second).toHaveBeenCalledWith(expectedPayload);

    unlistenFirst();
    expect(underlyingUnlisten).not.toHaveBeenCalled();
    unlistenSecond();
    expect(underlyingUnlisten).toHaveBeenCalledOnce();
  });

  it('disconnects only the subscriptions owned by that adapter', async () => {
    const handlers = new Map<string, (event: { payload: unknown }) => void>();
    const underlyingUnlisten = vi.fn();
    listenMock.mockImplementation(async (event: string, handler: (event: { payload: unknown }) => void) => {
      handlers.set(event, handler);
      return underlyingUnlisten;
    });

    const firstAdapter = new TauriTransportAdapter();
    const secondAdapter = new TauriTransportAdapter();
    const first = vi.fn();
    const second = vi.fn();
    firstAdapter.listen('account://login-state', first);
    secondAdapter.listen('account://login-state', second);
    await Promise.all([
      firstAdapter.waitForListenerRegistrations(),
      secondAdapter.waitForListenerRegistrations(),
    ]);

    await firstAdapter.disconnect();
    expect(underlyingUnlisten).not.toHaveBeenCalled();

    handlers.get('account://login-state')?.({ payload: { logged_in: true } });
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledOnce();
    expect(second).toHaveBeenCalledWith({ logged_in: true });

    await secondAdapter.disconnect();
    expect(underlyingUnlisten).toHaveBeenCalledOnce();
  });

  it('releases a held event to the subscribers that owned it on arrival', async () => {
    const handlers = new Map<string, (event: { payload: unknown }) => void>();
    listenMock.mockImplementation(async (event: string, handler: (event: { payload: unknown }) => void) => {
      handlers.set(event, handler);
      return vi.fn();
    });

    const firstAdapter = new TauriTransportAdapter();
    const first = vi.fn();
    const unlistenFirst = firstAdapter.listen('agentic://text-chunk', first);
    await firstAdapter.waitForListenerRegistrations();

    const attachment = beginRuntimeSessionAttachment('local', 'session-1');
    handlers.get('agentic://text-chunk')?.({
      payload: {
        sessionId: 'session-1',
        text: 'held',
        [RUNTIME_EVENT_STREAM_ID_KEY]: 'runtime-1',
        [RUNTIME_EVENT_CURSOR_KEY]: 2,
      },
    });
    unlistenFirst();

    const secondAdapter = new TauriTransportAdapter();
    const second = vi.fn();
    const unlistenSecond = secondAdapter.listen('agentic://text-chunk', second);
    await secondAdapter.waitForListenerRegistrations();

    attachment.finish({ streamId: 'runtime-1', cursor: 1 });
    expect(first).toHaveBeenCalledOnce();
    expect(first).toHaveBeenCalledWith({
      sessionId: 'session-1',
      text: 'held',
    });
    expect(second).not.toHaveBeenCalled();

    unlistenSecond();
  });
});
