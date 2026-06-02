import { beforeEach, describe, expect, it, vi } from 'vitest';
import { isExpectedTauriRequestError, TauriTransportAdapter } from './tauri-adapter';

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

describe('Tauri adapter expected errors', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
});
