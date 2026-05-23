import { describe, expect, it, vi } from 'vitest';

vi.mock('./ConfigManager', () => ({
  configManager: {},
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

import { getProviderDisplayName } from './modelConfigs';

describe('modelConfigs', () => {
  it('preserves custom provider names even when the base URL matches a known provider', () => {
    expect(getProviderDisplayName({
      name: 'My Zhipu Proxy',
      base_url: 'https://open.bigmodel.cn/api/paas/v4',
      model_name: 'glm-5',
    })).toBe('My Zhipu Proxy');
  });

  it('keeps legacy URL inference when a provider name is missing', () => {
    expect(getProviderDisplayName({
      base_url: 'https://open.bigmodel.cn/api/paas/v4',
      model_name: 'glm-5',
    })).toBe('settings/ai-model:providers.zhipu.name');
  });
});
