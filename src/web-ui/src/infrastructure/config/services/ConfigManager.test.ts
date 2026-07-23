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
    delete globalThis.__BITFUN_BOOTSTRAP_KEYBINDINGS__;
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

  it('reads optional configs without reporting expected missing paths as failures', async () => {
    configApiMocks.getConfig
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce(undefined);

    await expect(configManager.getOptionalConfig('app.keybindings')).resolves.toBeUndefined();

    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
    expect(configApiMocks.getConfig).toHaveBeenCalledWith(
      'app.keybindings',
      { skipRetryOnNotFound: true }
    );

    await expect(configManager.getConfig('app.keybindings')).resolves.toBeUndefined();
    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(2);
    expect(configApiMocks.getConfig).toHaveBeenLastCalledWith('app.keybindings');
  });

  it('uses bootstrap keybindings for the first optional startup read without a config IPC', async () => {
    const storedKeybindings = {
      version: 1,
      bindings: {
        'session.new': 'Ctrl+Shift+N',
      },
    };
    globalThis.__BITFUN_BOOTSTRAP_KEYBINDINGS__ = storedKeybindings;
    configApiMocks.getConfig.mockResolvedValueOnce({ version: 1, bindings: {} });

    await expect(configManager.getOptionalConfig('app.keybindings')).resolves.toEqual(storedKeybindings);
    expect(configApiMocks.getConfig).not.toHaveBeenCalled();
    expect(Object.prototype.hasOwnProperty.call(globalThis, '__BITFUN_BOOTSTRAP_KEYBINDINGS__')).toBe(false);

    await expect(configManager.getOptionalConfig('app.keybindings')).resolves.toEqual({ version: 1, bindings: {} });
    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
    expect(configApiMocks.getConfig).toHaveBeenCalledWith(
      'app.keybindings',
      { skipRetryOnNotFound: true }
    );
  });

  it('does not reuse bootstrap keybindings after the config path is updated', async () => {
    globalThis.__BITFUN_BOOTSTRAP_KEYBINDINGS__ = {
      version: 1,
      bindings: {
        'session.new': 'Ctrl+Shift+N',
      },
    };
    configApiMocks.setConfig.mockResolvedValueOnce(undefined);
    configApiMocks.getConfig.mockResolvedValueOnce({
      version: 1,
      bindings: {
        'session.new': 'Ctrl+Alt+N',
      },
    });

    await configManager.setConfig('app.keybindings', {
      version: 1,
      bindings: {
        'session.new': 'Ctrl+N',
      },
    });
    configManager.clearCache();

    await expect(configManager.getOptionalConfig('app.keybindings')).resolves.toEqual({
      version: 1,
      bindings: {
        'session.new': 'Ctrl+Alt+N',
      },
    });
    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
    expect(configApiMocks.getConfig).toHaveBeenCalledWith(
      'app.keybindings',
      { skipRetryOnNotFound: true }
    );
  });

  it('clears bootstrap keybindings with the config cache', async () => {
    globalThis.__BITFUN_BOOTSTRAP_KEYBINDINGS__ = {
      version: 1,
      bindings: {
        'session.new': 'Ctrl+Shift+N',
      },
    };
    configApiMocks.getConfig.mockResolvedValueOnce({
      version: 1,
      bindings: {
        'session.new': 'Ctrl+Alt+N',
      },
    });

    configManager.clearCache();

    await expect(configManager.getOptionalConfig('app.keybindings')).resolves.toEqual({
      version: 1,
      bindings: {
        'session.new': 'Ctrl+Alt+N',
      },
    });
    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
    expect(configApiMocks.getConfig).toHaveBeenCalledWith(
      'app.keybindings',
      { skipRetryOnNotFound: true }
    );
  });

  it('does not share optional in-flight reads with strict config reads', async () => {
    const optionalRead = createDeferred<undefined>();
    const strictRead = createDeferred<Record<string, string>>();
    configApiMocks.getConfig
      .mockReturnValueOnce(optionalRead.promise)
      .mockReturnValueOnce(strictRead.promise);

    const optionalPromise = configManager.getOptionalConfig('app.keybindings');
    const strictPromise = configManager.getConfig('app.keybindings');

    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(2);
    expect(configApiMocks.getConfig).toHaveBeenNthCalledWith(
      1,
      'app.keybindings',
      { skipRetryOnNotFound: true }
    );
    expect(configApiMocks.getConfig).toHaveBeenNthCalledWith(2, 'app.keybindings');

    optionalRead.resolve(undefined);
    strictRead.resolve({ 'session.new': 'Ctrl+N' });

    await expect(optionalPromise).resolves.toBeUndefined();
    await expect(strictPromise).resolves.toEqual({ 'session.new': 'Ctrl+N' });
  });

  it('batches cold multi-path reads and caches the returned configs', async () => {
    configApiMocks.getConfigs.mockResolvedValueOnce({
      'ai.models': [{ id: 'model-1' }],
      'ai.default_models': { primary: 'model-1' },
      'ai.func_agent_models': { title: 'primary' },
    });

    const configs = await configManager.getConfigs([
      'ai.models',
      'ai.default_models',
      'ai.func_agent_models',
    ]);

    expect(configApiMocks.getConfigs).toHaveBeenCalledTimes(1);
    expect(configApiMocks.getConfigs).toHaveBeenCalledWith([
      'ai.models',
      'ai.default_models',
      'ai.func_agent_models',
    ]);
    expect(configApiMocks.getConfig).not.toHaveBeenCalled();
    expect(configs['ai.models']).toMatchObject([{ id: 'model-1' }]);
    await expect(configManager.getConfig('ai.default_models')).resolves.toEqual({ primary: 'model-1' });
    expect(configApiMocks.getConfig).not.toHaveBeenCalled();
  });

  it('does not let an older batch read overwrite a newer config write', async () => {
    const staleBatch = createDeferred<Record<string, unknown>>();
    const path = 'ai.agent_model_defaults';
    const previousValue = {
      mode: 'model-old',
      subagents: {},
    };
    const nextValue = {
      mode: 'model-new',
      subagents: {},
    };
    configApiMocks.getConfigs.mockReturnValueOnce(staleBatch.promise);
    configApiMocks.setConfig.mockResolvedValueOnce(undefined);

    const pendingRead = configManager.getConfigs([path]);
    await configManager.setConfig(path, nextValue);
    staleBatch.resolve({ [path]: previousValue });

    await expect(pendingRead).resolves.toEqual({ [path]: nextValue });
    await expect(configManager.getConfig(path)).resolves.toEqual(nextValue);
    expect(configApiMocks.getConfig).not.toHaveBeenCalled();
  });

  it('refreshes an older parent read after a child config path is written', async () => {
    const staleBatch = createDeferred<Record<string, unknown>>();
    const parentPath = 'ai.agent_model_defaults';
    const childPath = `${parentPath}.mode`;
    const previousValue = {
      mode: 'model-old',
      subagents: {
        default: { kind: 'fixed', model_id: 'fast' },
      },
    };
    const nextValue = { ...previousValue, mode: 'model-new' };
    configApiMocks.getConfigs.mockReturnValueOnce(staleBatch.promise);
    configApiMocks.getConfig.mockResolvedValueOnce(nextValue);
    configApiMocks.setConfig.mockResolvedValueOnce(undefined);

    const pendingRead = configManager.getConfigs([parentPath]);
    await configManager.setConfig(childPath, 'model-new');
    staleBatch.resolve({ [parentPath]: previousValue });

    await expect(pendingRead).resolves.toEqual({ [parentPath]: nextValue });
    expect(configApiMocks.getConfig).toHaveBeenCalledWith(parentPath);
  });

  it('waits for a child config write before reading its parent path', async () => {
    const write = createDeferred<void>();
    const parentPath = 'ai.agent_model_defaults';
    const childPath = `${parentPath}.mode`;
    const previousValue = { mode: 'model-old', subagents: {} };
    const nextValue = { mode: 'model-new', subagents: {} };
    configApiMocks.getConfig
      .mockResolvedValueOnce(previousValue)
      .mockResolvedValueOnce(nextValue);
    configApiMocks.setConfig.mockReturnValueOnce(write.promise);

    await expect(configManager.getConfig(parentPath)).resolves.toEqual(previousValue);
    const pendingWrite = configManager.setConfig(childPath, 'model-new');
    await Promise.resolve();
    const pendingRead = configManager.getConfig(parentPath);

    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
    write.resolve();
    await pendingWrite;

    await expect(pendingRead).resolves.toEqual(nextValue);
    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(2);
  });

  it('notifies a parent path watcher when a child config path changes', async () => {
    const watcher = vi.fn();
    const unwatch = configManager.watch('ai.agent_model_defaults', watcher);
    configApiMocks.setConfig.mockResolvedValueOnce(undefined);

    await configManager.setConfig('ai.agent_model_defaults.mode', 'model-new');

    expect(watcher).toHaveBeenCalledTimes(1);
    unwatch();
  });

  it('reuses in-flight single-path reads when batching overlapping config paths', async () => {
    const defaultModels = createDeferred<Record<string, string>>();
    configApiMocks.getConfig.mockReturnValueOnce(defaultModels.promise);
    configApiMocks.getConfigs.mockResolvedValueOnce({
      'ai.models': [],
      'ai.func_agent_models': { title: 'fast' },
    });

    const singleRead = configManager.getConfig('ai.default_models');
    const batchRead = configManager.getConfigs([
      'ai.models',
      'ai.default_models',
      'ai.func_agent_models',
    ]);

    expect(configApiMocks.getConfigs).toHaveBeenCalledWith([
      'ai.models',
      'ai.func_agent_models',
    ]);
    defaultModels.resolve({ primary: 'model-1' });

    await expect(singleRead).resolves.toEqual({ primary: 'model-1' });
    await expect(batchRead).resolves.toEqual({
      'ai.models': [],
      'ai.default_models': { primary: 'model-1' },
      'ai.func_agent_models': { title: 'fast' },
    });
    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
  });

  it('settles batched in-flight reads before propagating overlapping non-fallback failures', async () => {
    const loggingLevel = createDeferred<string>();
    const batchedConfigs = createDeferred<Record<string, unknown>>();
    configApiMocks.getConfig.mockReturnValueOnce(loggingLevel.promise);
    configApiMocks.getConfigs.mockReturnValueOnce(batchedConfigs.promise);

    const singleRead = configManager.getConfig('app.logging.level');
    const batchRead = configManager.getConfigs([
      'app.logging.level',
      'app.window.mode',
    ]);

    expect(configApiMocks.getConfigs).toHaveBeenCalledWith(['app.window.mode']);

    loggingLevel.reject(new Error('single read failed'));
    batchedConfigs.resolve({ 'app.window.mode': 'compact' });

    await expect(singleRead).rejects.toThrow('single read failed');
    await expect(batchRead).rejects.toThrow('single read failed');
    await expect(configManager.getConfig('app.window.mode')).resolves.toBe('compact');
    expect(configApiMocks.getConfig).toHaveBeenCalledTimes(1);
  });

  it('returns AI config fallbacks when a batch read fails', async () => {
    configApiMocks.getConfigs.mockRejectedValueOnce(new Error('batch failed'));

    await expect(configManager.getConfigs([
      'ai.models',
      'ai.default_models',
      'ai.agent_model_defaults',
      'ai.func_agent_models',
    ])).resolves.toEqual({
      'ai.models': [],
      'ai.default_models': {},
      'ai.agent_model_defaults': {
        mode: 'auto',
        subagents: {
          default: { kind: 'fixed', model_id: 'fast' },
          builtin: {
            GeneralPurpose: { kind: 'fixed', model_id: 'primary' },
          },
          fork: { kind: 'inherit' },
        },
      },
      'ai.func_agent_models': {},
    });
  });

  it('returns the GeneralPurpose primary default when a single config read fails', async () => {
    configApiMocks.getConfig.mockRejectedValueOnce(new Error('read failed'));

    await expect(configManager.getConfig('ai.agent_model_defaults')).resolves.toEqual({
      mode: 'auto',
      subagents: {
        default: { kind: 'fixed', model_id: 'fast' },
        builtin: {
          GeneralPurpose: { kind: 'fixed', model_id: 'primary' },
        },
        fork: { kind: 'inherit' },
      },
    });
  });

  it('reloads startup config paths through one batch call', async () => {
    configApiMocks.getConfigs.mockResolvedValueOnce({
      'ai.models': [],
      'ai.agent_model_defaults': {
        mode: 'auto',
        subagents: {
          default: { kind: 'fixed', model_id: 'fast' },
          builtin: {
            GeneralPurpose: { kind: 'fixed', model_id: 'primary' },
          },
          fork: { kind: 'inherit' },
        },
      },
      'ai.func_agent_models': { title: 'gpt-5-mini' },
      'ai.default_models': { chat: 'gpt-5' },
    });

    await configManager.reload();

    expect(configApiMocks.getConfigs).toHaveBeenCalledTimes(1);
    expect(configApiMocks.getConfigs).toHaveBeenCalledWith([
      'ai.models',
      'ai.agent_model_defaults',
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

  it('applyExternalReload notifies listeners only for paths whose value changed', async () => {
    configApiMocks.getConfig.mockImplementation(async (path?: string) => {
      if (path === 'editor') return { fontSize: 14 };
      if (path === 'app.window.mode') return 'compact';
      return undefined;
    });
    await configManager.getConfig('editor');
    await configManager.getConfig('app.window.mode');

    const changes: Array<{ path: string; oldValue: unknown; newValue: unknown }> = [];
    const unsubscribe = configManager.onConfigChange((path, oldValue, newValue) => {
      changes.push({ path, oldValue, newValue });
    });
    const watchedPaths: string[] = [];
    const unwatchKeybindings = configManager.watch('app.keybindings', () => {
      watchedPaths.push('app.keybindings');
    });
    const unwatchEditor = configManager.watch('editor', () => {
      watchedPaths.push('editor');
    });

    // Cloud sync changed `editor` only; other tracked paths stay identical.
    configApiMocks.getConfigs.mockResolvedValueOnce({
      editor: { fontSize: 16 },
      'app.window.mode': 'compact',
      'app.keybindings': undefined,
    });

    await configManager.applyExternalReload();

    expect(configApiMocks.getConfigs).toHaveBeenCalledTimes(1);
    expect(changes).toEqual([
      { path: 'editor', oldValue: { fontSize: 14 }, newValue: { fontSize: 16 } },
    ]);
    expect(watchedPaths).toEqual(['editor']);
    expect(configManager.get('editor')).toEqual({ fontSize: 16 });
    expect(configManager.get('app.window.mode')).toBe('compact');

    unwatchEditor();
    unwatchKeybindings();
    unsubscribe();
  });

  it('applyExternalReload clears the cache without notifying when the re-read fails', async () => {
    configApiMocks.getConfig.mockResolvedValueOnce({ fontSize: 14 });
    await configManager.getConfig('editor');

    const changes: string[] = [];
    const unsubscribe = configManager.onConfigChange(path => {
      changes.push(path);
    });

    configApiMocks.getConfigs.mockRejectedValueOnce(new Error('ipc down'));
    await configManager.applyExternalReload();

    expect(changes).toEqual([]);

    // Cache was cleared, so the next read fetches the fresh value from the backend.
    configApiMocks.getConfig.mockResolvedValueOnce({ fontSize: 16 });
    await expect(configManager.getConfig('editor')).resolves.toEqual({ fontSize: 16 });
    expect(configApiMocks.getConfig).toHaveBeenCalledWith('editor');

    unsubscribe();
  });
});
