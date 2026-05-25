import { beforeEach, describe, expect, it, vi } from 'vitest';
import { configManager } from './ConfigManager';

const configApiMocks = vi.hoisted(() => ({
  getConfig: vi.fn(),
  getConfigs: vi.fn(),
  setConfig: vi.fn(),
  resetConfig: vi.fn(),
  exportConfig: vi.fn(),
  importConfig: vi.fn(),
}));

vi.mock('@/infrastructure/api', () => ({
  configAPI: configApiMocks,
}));

vi.mock('@/infrastructure/api/service-api/ConfigAPI', () => ({
  configAPI: configApiMocks,
}));

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('ConfigManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    configManager.clearCache();
  });

  it('deduplicates concurrent reads for the same config path', async () => {
    const deferred = createDeferred<string>();
    configApiMocks.getConfig.mockReturnValueOnce(deferred.promise);

    const first = configManager.getConfig<string>('app.logging.level');
    const second = configManager.getConfig<string>('app.logging.level');

    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
    expect(configApiMocks.getConfig).toHaveBeenCalledWith('app.logging.level');

    deferred.resolve('debug');

    await expect(Promise.all([first, second])).resolves.toEqual(['debug', 'debug']);
    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
  });

  it('reloads startup config paths through one batch call', async () => {
    configApiMocks.getConfigs.mockResolvedValueOnce({
      'ai.models': [],
      'ai.agent_models': { coder: 'gpt-5' },
      'ai.func_agent_models': { title: 'gpt-5-mini' },
      'ai.default_models': { chat: 'gpt-5' },
    });

    await configManager.reload();

    expect(configApiMocks.getConfigs).toHaveBeenCalledTimes(1);
    expect(configApiMocks.getConfigs).toHaveBeenCalledWith([
      'ai.models',
      'ai.agent_models',
      'ai.func_agent_models',
      'ai.default_models',
    ]);
    expect(configApiMocks.getConfig).not.toHaveBeenCalled();
    expect(configManager.get('ai.default_models')).toEqual({ chat: 'gpt-5' });
  });

  it('migrates legacy models with the same base URL into one provider instance', async () => {
    const legacyModels = [
      {
        id: 'model-a',
        name: 'First provider',
        base_url: 'https://open.bigmodel.cn/api/paas/v4',
        model_name: 'glm-5',
      },
      {
        id: 'model-b',
        name: 'Second provider',
        base_url: 'https://open.bigmodel.cn/api/paas/v4/',
        model_name: 'glm-4.7',
      },
      {
        id: 'model-c',
        name: 'Other provider',
        base_url: 'https://api.deepseek.com/v1',
        model_name: 'deepseek-v4',
      },
    ];
    configApiMocks.getConfig.mockResolvedValueOnce(legacyModels);

    const migrated = await configManager.getConfig<any[]>('ai.models');

    const firstProviderId = migrated[0].metadata.provider_instance_id;
    expect(firstProviderId).toMatch(/^provider_legacy_/);
    expect(migrated[1].metadata.provider_instance_id).toBe(firstProviderId);
    expect(migrated[2].metadata.provider_instance_id).not.toBe(firstProviderId);
    expect(configApiMocks.setConfig).toHaveBeenCalledWith('ai.models', migrated);
  });
});
