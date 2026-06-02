import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiClient } from './ApiClient';

const adapterMocks = vi.hoisted(() => ({
  request: vi.fn(),
  listen: vi.fn(),
  connect: vi.fn(),
  disconnect: vi.fn(),
  isConnected: vi.fn(() => true),
}));

const traceMocks = vi.hoisted(() => ({
  estimateJsonBytes: vi.fn(() => 1),
  recordApiCall: vi.fn(),
}));

const loggerMocks = vi.hoisted(() => ({
  debug: vi.fn(),
  warn: vi.fn(),
  error: vi.fn(),
}));

vi.mock('../adapters', () => ({
  getTransportAdapter: () => adapterMocks,
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => loggerMocks,
}));

vi.mock('@/shared/utils/startupTrace', () => ({
  estimateJsonBytes: traceMocks.estimateJsonBytes,
  isRemoteTraceRequest: vi.fn(() => false),
  startupTrace: traceMocks,
}));

describe('ApiClient startup trace classification', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    delete globalThis.__BITFUN_PERF_TRACE_ENABLED__;
  });

  it('does not record optional get_config not found as a startup failure', async () => {
    adapterMocks.request.mockRejectedValueOnce(new Error("Config path not found: 'font'"));
    const client = new ApiClient({ enableLogging: true, retries: 0 });

    await expect(
      client.invoke('get_config', {
        request: {
          path: 'font',
          skipRetryOnNotFound: true,
        },
      })
    ).rejects.toThrow();

    expect(traceMocks.recordApiCall).toHaveBeenCalledWith(expect.objectContaining({
      command: 'get_config',
      target: 'font',
      outcome: 'success',
    }));
    expect(client.getStats()).toMatchObject({
      successfulRequests: 1,
      failedRequests: 0,
    });
    expect(loggerMocks.error).not.toHaveBeenCalled();
  });

  it('does not estimate payload bytes by default', async () => {
    adapterMocks.request.mockResolvedValueOnce({ turns: [] });
    const client = new ApiClient({ enableLogging: false, retries: 0 });

    await client.invoke('restore_session_view', {
      request: {
        sessionId: 'history-1',
        workspacePath: 'D:/workspace/BitFun',
      },
    });

    expect(traceMocks.estimateJsonBytes).not.toHaveBeenCalled();
    expect(traceMocks.recordApiCall).toHaveBeenCalledWith(expect.objectContaining({
      command: 'restore_session_view',
      requestBytes: undefined,
      responseBytes: undefined,
      payloadEstimateDurationMs: undefined,
    }));
  });

  it('uses a bounded response estimate cap for session view restore when perf trace is enabled', async () => {
    globalThis.__BITFUN_PERF_TRACE_ENABLED__ = true;
    adapterMocks.request.mockResolvedValueOnce({ turns: [] });
    const client = new ApiClient({ enableLogging: false, retries: 0 });

    await client.invoke('restore_session_view', {
      request: {
        sessionId: 'history-1',
        workspacePath: 'D:/workspace/BitFun',
      },
    });

    expect(traceMocks.estimateJsonBytes).toHaveBeenCalledWith(
      { turns: [] },
      2 * 1024 * 1024
    );
  });

  it('records request boundary timings and active request pressure', async () => {
    let releaseFirstRequest!: () => void;
    adapterMocks.request
      .mockImplementationOnce((_command, _args, timing) => new Promise<void>(resolve => {
        Object.assign(timing, {
          adapterInitDurationMs: 1,
          invokeDurationMs: 10,
          transportDurationMs: 11,
        });
        releaseFirstRequest = resolve;
      }))
      .mockImplementationOnce((_command, _args, timing) => {
        Object.assign(timing, {
          adapterInitDurationMs: 2,
          invokeDurationMs: 20,
          transportDurationMs: 22,
        });
        return Promise.resolve({ ok: true });
      });
    const client = new ApiClient({ enableLogging: false, retries: 0 });

    const firstRequest = client.invoke('get_config', {
      request: { path: 'app.keybindings' },
    });
    const secondRequest = client.invoke('list_persisted_sessions_page', {
      request: {
        workspacePath: 'D:/workspace/BitFun',
        limit: 5,
      },
    });

    await secondRequest;
    releaseFirstRequest();
    await firstRequest;

    expect(traceMocks.recordApiCall).toHaveBeenCalledWith(expect.objectContaining({
      command: 'list_persisted_sessions_page',
      requestPayloadEstimateDurationMs: undefined,
      responsePayloadEstimateDurationMs: undefined,
      payloadEstimateDurationMs: undefined,
      adapterInitDurationMs: expect.any(Number),
      transportDurationMs: expect.any(Number),
      activeRequestsAtStart: 1,
      activeRequestsAtEnd: 1,
      maxConcurrentRequests: 2,
    }));
  });
});
