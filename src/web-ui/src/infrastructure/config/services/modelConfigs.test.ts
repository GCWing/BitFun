import { beforeEach, describe, expect, it, vi } from 'vitest';

const configManagerMock = vi.hoisted(() => ({
  getConfig: vi.fn(),
  setConfig: vi.fn(),
}));

vi.mock('./ConfigManager', () => ({
  configManager: configManagerMock,
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

describe('modelConfigs', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it('does not read ai.models when imported for display helpers', async () => {
    await import('./modelConfigs');
    await Promise.resolve();

    expect(configManagerMock.getConfig).not.toHaveBeenCalled();
  });

  it('preserves custom provider names even when the base URL matches a known provider', async () => {
    const { getProviderDisplayName } = await import('./modelConfigs');

    expect(getProviderDisplayName({
      name: 'My Zhipu Proxy',
      base_url: 'https://open.bigmodel.cn/api/paas/v4',
      model_name: 'glm-5',
    })).toBe('My Zhipu Proxy');
  });

  it('keeps legacy URL inference when a provider name is missing', async () => {
    const { getProviderDisplayName } = await import('./modelConfigs');

    expect(getProviderDisplayName({
      base_url: 'https://open.bigmodel.cn/api/paas/v4',
      model_name: 'glm-5',
    })).toBe('settings/ai-model:providers.zhipu.name');
  });

  it('loads ai.models only when the model manager is actually used', async () => {
    configManagerMock.getConfig.mockResolvedValueOnce([
      {
        id: 'model-1',
        name: 'Provider',
        base_url: 'https://example.test',
        api_key: '',
        model_name: 'model',
        provider: 'openai',
      },
    ]);
    const { modelConfigManager } = await import('./modelConfigs');
    const listener = vi.fn();

    modelConfigManager.addListener(listener);
    await Promise.resolve();
    await Promise.resolve();

    expect(configManagerMock.getConfig).toHaveBeenCalledTimes(1);
    expect(configManagerMock.getConfig).toHaveBeenCalledWith('ai.models');
    expect(listener).toHaveBeenCalledWith([
      expect.objectContaining({
        id: 'model-1',
        modelName: 'model',
      }),
    ]);
  });
});
