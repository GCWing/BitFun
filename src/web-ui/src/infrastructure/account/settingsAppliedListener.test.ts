import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  listen: vi.fn(),
  reloadConfig: vi.fn(),
  applyExternalReload: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/ApiClient', () => ({
  api: { listen: mocks.listen },
}));

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: { reloadConfig: mocks.reloadConfig },
}));

vi.mock('@/infrastructure/config/services/ConfigManager', () => ({
  configManager: { applyExternalReload: mocks.applyExternalReload },
}));

vi.mock('@/shared/utils/logger', () => ({
  createLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

describe('settingsAppliedListener', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    mocks.listen.mockReturnValue(() => undefined);
    mocks.reloadConfig.mockResolvedValue(undefined);
    mocks.applyExternalReload.mockResolvedValue(undefined);
  });

  it('refreshes the config cache when the backend applies cloud settings', async () => {
    const { ensureSettingsAppliedListener } = await import('./settingsAppliedListener');
    ensureSettingsAppliedListener();

    expect(mocks.listen).toHaveBeenCalledTimes(1);
    const [event, handler] = mocks.listen.mock.calls[0];
    expect(event).toBe('account://settings-applied');

    handler({ applied: true });

    await vi.waitFor(() => {
      expect(mocks.reloadConfig).toHaveBeenCalledTimes(1);
      expect(mocks.applyExternalReload).toHaveBeenCalledTimes(1);
    });
  });

  it('registers the listener only once', async () => {
    const { ensureSettingsAppliedListener } = await import('./settingsAppliedListener');
    ensureSettingsAppliedListener();
    ensureSettingsAppliedListener();

    expect(mocks.listen).toHaveBeenCalledTimes(1);
  });
});
