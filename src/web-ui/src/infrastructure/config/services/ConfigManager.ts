 

import {
  IConfigManager,
  ConfigValidationResult,
  ConfigExport,
} from '../types';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import { extractProviderSegmentFromBaseUrl, matchProviderCatalogItemByBaseUrl, normalizeProviderBaseUrl } from './providerCatalog';

const log = createLogger('ConfigManager');
const PROVIDER_INSTANCE_METADATA_KEY = 'provider_instance_id';

declare global {
  // Injected by the desktop webview initialization script before the frontend
  // bundle runs. It avoids a startup-window IPC for the initial shortcut load.
  var __BITFUN_BOOTSTRAP_KEYBINDINGS__: unknown | undefined;
}

function legacyProviderInstanceId(seed: string): string {
  let hash = 2166136261;
  for (let i = 0; i < seed.length; i += 1) {
    hash ^= seed.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return `provider_legacy_${(hash >>> 0).toString(36)}`;
}

function readProviderInstanceId(model: Record<string, unknown>): string | undefined {
  const metadata = model.metadata;
  if (!metadata || typeof metadata !== 'object') {
    return undefined;
  }

  const value = (metadata as Record<string, unknown>)[PROVIDER_INSTANCE_METADATA_KEY];
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

function legacyProviderGroupSeed(model: Record<string, unknown>, index: number): string {
  const baseUrl = typeof model.base_url === 'string' ? model.base_url : '';
  const normalizedBaseUrl = baseUrl ? normalizeProviderBaseUrl(baseUrl) : '';
  if (normalizedBaseUrl) {
    return `base_url:${normalizedBaseUrl}`;
  }

  const id = typeof model.id === 'string' ? model.id.trim() : '';
  return id ? `id:${id}` : `index:${index}`;
}

/** Structural equality for JSON-shaped config values. */
function configValuesEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) {
    return true;
  }
  if (typeof a !== typeof b || typeof a !== 'object' || a === null || b === null) {
    return false;
  }
  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) {
      return false;
    }
    return a.every((item, index) => configValuesEqual(item, b[index]));
  }
  const aRecord = a as Record<string, unknown>;
  const bRecord = b as Record<string, unknown>;
  const aKeys = Object.keys(aRecord);
  const bKeys = Object.keys(bRecord);
  return (
    aKeys.length === bKeys.length &&
    aKeys.every(
      key =>
        Object.prototype.hasOwnProperty.call(bRecord, key) &&
        configValuesEqual(aRecord[key], bRecord[key])
    )
  );
}

class ConfigManagerImpl implements IConfigManager {
  
  private configCache: Map<string, any> = new Map();
  private inFlightReads: Map<string, Promise<unknown>> = new Map();
  private inFlightMutations: Map<string, Promise<void>> = new Map();
  private pathMutationVersions: Map<string, number> = new Map();
  private rootMutationVersion = 0;
  private listeners: Set<(path: string, oldValue: any, newValue: any) => void> = new Set();
  private pathListeners: Map<string, Set<() => void>> = new Map();

  constructor() {
    log.info('Initializing config manager (proxy mode)');
  }

  private async migrateLegacyAiModelsIfNeeded(config: unknown): Promise<unknown> {
    if (!Array.isArray(config)) {
      return config;
    }

    let migratedNameCount = 0;
    let migratedProviderInstanceCount = 0;
    const migratedModels = config.map((item, index) => {
      if (!item || typeof item !== 'object') {
        return item;
      }

      const model = item as Record<string, unknown>;
      let nextModel = model;
      const currentName = typeof model.name === 'string' ? model.name.trim() : '';
      if (!currentName) {
        const baseUrl = typeof model.base_url === 'string' ? model.base_url : '';
        const matchedProvider = matchProviderCatalogItemByBaseUrl(baseUrl);
        const inferredProviderName = matchedProvider
          ? i18nService.t(`settings/ai-model:providers.${matchedProvider.id}.name`)
          : extractProviderSegmentFromBaseUrl(baseUrl);

        if (inferredProviderName) {
          migratedNameCount += 1;
          nextModel = {
            ...nextModel,
            name: inferredProviderName,
          };
        }
      }

      if (readProviderInstanceId(nextModel)) {
        return nextModel;
      }

      const metadata = nextModel.metadata && typeof nextModel.metadata === 'object'
        ? nextModel.metadata as Record<string, unknown>
        : {};
      migratedProviderInstanceCount += 1;
      return {
        ...nextModel,
        metadata: {
          ...metadata,
          [PROVIDER_INSTANCE_METADATA_KEY]: legacyProviderInstanceId(
            legacyProviderGroupSeed(nextModel, index)
          ),
        },
      };
    });

    if (migratedNameCount === 0 && migratedProviderInstanceCount === 0) {
      return config;
    }

    await configAPI.setConfig('ai.models', migratedModels);
    log.info('Migrated legacy ai.models', {
      migratedNameCount,
      migratedProviderInstanceCount,
    });
    return migratedModels;
  }

  

  private getReadKey(path?: string, mode: 'normal' | 'optional' = 'normal'): string {
    return `${mode}:${path ?? '<root>'}`;
  }

  private getMutationKey(path?: string): string {
    return path ?? '<root>';
  }

  private configPathsOverlap(left?: string, right?: string): boolean {
    if (!left || !right) {
      return true;
    }

    return left === right || left.startsWith(`${right}.`) || right.startsWith(`${left}.`);
  }

  private getPathMutationVersion(path: string): number {
    if (!this.pathMutationVersions.has(path)) {
      this.pathMutationVersions.set(path, 0);
    }
    return this.pathMutationVersions.get(path)!;
  }

  private bumpOverlappingMutationVersions(path?: string): void {
    this.rootMutationVersion += 1;
    if (path) {
      this.getPathMutationVersion(path);
    }

    for (const knownPath of this.pathMutationVersions.keys()) {
      if (this.configPathsOverlap(knownPath, path)) {
        this.pathMutationVersions.set(knownPath, this.getPathMutationVersion(knownPath) + 1);
      }
    }
  }

  private relatedMutationPromises(paths: Array<string | undefined>): Promise<void>[] {
    const promises = new Set<Promise<void>>();
    for (const [mutationKey, mutation] of this.inFlightMutations) {
      const mutationPath = mutationKey === '<root>' ? undefined : mutationKey;
      if (paths.some(path => this.configPathsOverlap(path, mutationPath))) {
        promises.add(mutation);
      }
    }
    return Array.from(promises);
  }

  private waitForRelatedMutations(paths: Array<string | undefined>): Promise<void> | undefined {
    const mutations = this.relatedMutationPromises(paths);
    if (mutations.length === 0) {
      return undefined;
    }
    return Promise.allSettled(mutations).then(() => undefined);
  }

  private async resolveReadValue<T>(
    path: string,
    readVersion: number,
    value: T,
    retry: () => Promise<T>,
  ): Promise<T> {
    if (readVersion === this.getPathMutationVersion(path)) {
      this.configCache.set(path, value);
      return value;
    }

    const pendingMutations = this.waitForRelatedMutations([path]);
    if (pendingMutations) {
      await pendingMutations;
    }
    if (this.configCache.has(path)) {
      return this.configCache.get(path) as T;
    }
    return retry();
  }

  private readPathFromKey(readKey: string): string | undefined {
    const separatorIndex = readKey.indexOf(':');
    const path = separatorIndex >= 0 ? readKey.slice(separatorIndex + 1) : readKey;
    return path === '<root>' ? undefined : path;
  }

  private invalidateOverlappingLocalState(path?: string): void {
    for (const cachedPath of Array.from(this.configCache.keys())) {
      if (this.configPathsOverlap(cachedPath, path)) {
        this.configCache.delete(cachedPath);
      }
    }

    for (const readKey of Array.from(this.inFlightReads.keys())) {
      if (this.configPathsOverlap(this.readPathFromKey(readKey), path)) {
        this.inFlightReads.delete(readKey);
      }
    }

    if (this.configPathsOverlap('app.keybindings', path)) {
      this.clearBootstrapOptionalConfigs();
    }
  }

  private async runMutation(
    path: string | undefined,
    mutate: () => Promise<void>,
    onSuccess?: () => void,
  ): Promise<void> {
    const mutationKey = this.getMutationKey(path);
    const previousMutations = this.relatedMutationPromises([path]);
    const operation = (async () => {
      if (previousMutations.length > 0) {
        await Promise.allSettled(previousMutations);
      }
      this.bumpOverlappingMutationVersions(path);
      this.invalidateOverlappingLocalState(path);
      await mutate();
      onSuccess?.();
    })();

    this.inFlightMutations.set(mutationKey, operation);
    try {
      await operation;
    } finally {
      if (this.inFlightMutations.get(mutationKey) === operation) {
        this.inFlightMutations.delete(mutationKey);
      }
    }
  }

  private clearBootstrapOptionalConfigs(): void {
    delete globalThis.__BITFUN_BOOTSTRAP_KEYBINDINGS__;
  }

  private consumeBootstrapOptionalConfig<T = any>(path: string): {
    available: boolean;
    value: T | undefined;
  } {
    if (path !== 'app.keybindings') {
      return { available: false, value: undefined };
    }

    if (!Object.prototype.hasOwnProperty.call(globalThis, '__BITFUN_BOOTSTRAP_KEYBINDINGS__')) {
      return { available: false, value: undefined };
    }

    const value = globalThis.__BITFUN_BOOTSTRAP_KEYBINDINGS__;
    delete globalThis.__BITFUN_BOOTSTRAP_KEYBINDINGS__;
    return {
      available: true,
      value: value == null ? undefined : value as T,
    };
  }

  private async readConfig<T = any>(path?: string): Promise<T> {
    const rootReadVersion = this.rootMutationVersion;
    const readVersion = path ? this.getPathMutationVersion(path) : 0;
    const config = await configAPI.getConfig(path);
    const resolvedConfig = path === 'ai.models'
      ? await this.migrateLegacyAiModelsIfNeeded(config)
      : config;

    if (path) {
      return this.resolveReadValue(
        path,
        readVersion,
        resolvedConfig as T,
        () => this.readConfig<T>(path),
      );
    }

    if (rootReadVersion !== this.rootMutationVersion) {
      const pendingMutations = this.waitForRelatedMutations([undefined]);
      if (pendingMutations) {
        await pendingMutations;
      }
      return this.readConfig<T>();
    }

    return resolvedConfig as T;
  }

  private async readOptionalConfig<T = any>(path: string): Promise<T | undefined> {
    const readVersion = this.getPathMutationVersion(path);
    const config = await configAPI.getConfig(path, { skipRetryOnNotFound: true });
    const resolvedConfig = path === 'ai.models'
      ? await this.migrateLegacyAiModelsIfNeeded(config)
      : config;

    if (readVersion !== this.getPathMutationVersion(path)) {
      const pendingMutations = this.waitForRelatedMutations([path]);
      if (pendingMutations) {
        await pendingMutations;
      }
      if (this.configCache.has(path)) {
        return this.configCache.get(path) as T;
      }
      return this.readOptionalConfig<T>(path);
    }

    return resolvedConfig as T | undefined;
  }

  private async readConfigs(paths: string[]): Promise<Record<string, unknown>> {
    const readVersions = new Map(
      paths.map(path => [path, this.getPathMutationVersion(path)] as const),
    );
    const configs = await configAPI.getConfigs(paths);
    const resolvedConfigs: Record<string, unknown> = {};

    for (const path of paths) {
      const resolvedConfig = path === 'ai.models'
        ? await this.migrateLegacyAiModelsIfNeeded(configs[path])
        : configs[path];

      resolvedConfigs[path] = await this.resolveReadValue(
        path,
        readVersions.get(path) ?? 0,
        resolvedConfig,
        () => this.readConfig(path),
      );
    }

    return resolvedConfigs;
  }

  private getFallbackConfigValue(path?: string): { hasFallback: boolean; value: unknown } {
    if (path === 'ai.models') {
      return { hasFallback: true, value: [] };
    }
    if (path === 'ai.func_agent_models' || path === 'ai.default_models') {
      return { hasFallback: true, value: {} };
    }
    if (path === 'ai.agent_model_defaults') {
      return {
        hasFallback: true,
        value: {
          mode: 'auto',
          subagents: {
            default: { kind: 'fixed', model_id: 'fast' },
            builtin: {
              GeneralPurpose: { kind: 'fixed', model_id: 'primary' },
            },
            fork: { kind: 'inherit' },
          },
        },
      };
    }
    return { hasFallback: false, value: undefined };
  }

  private fallbackOrThrow(path: string | undefined, error: unknown): unknown {
    const fallback = this.getFallbackConfigValue(path);
    if (fallback.hasFallback) {
      return fallback.value;
    }
    throw error;
  }

  async getConfig<T = any>(path?: string): Promise<T> {
    try {
      const pendingMutations = this.waitForRelatedMutations([path]);
      if (pendingMutations) {
        await pendingMutations;
      }

      if (path && this.configCache.has(path)) {
        return this.configCache.get(path);
      }

      const readKey = this.getReadKey(path);
      const existingRead = this.inFlightReads.get(readKey);
      if (existingRead) {
        return (await existingRead) as T;
      }

      const readPromise = this.readConfig<T>(path);
      this.inFlightReads.set(readKey, readPromise);
      try {
        return await readPromise;
      } finally {
        if (this.inFlightReads.get(readKey) === readPromise) {
          this.inFlightReads.delete(readKey);
        }
      }
    } catch (error) {
      log.error('Failed to get config', { path, error });
      // Return defaults to avoid breaking the UI.
      if (path === 'ai.models') {
        return [] as T;
      }
      if (path === 'ai.func_agent_models') {
        return {} as T;
      }
      if (path === 'ai.default_models') {
        return {} as T;
      }
      if (path === 'ai.agent_model_defaults') {
        return {
          mode: 'auto',
          subagents: {
            default: { kind: 'fixed', model_id: 'fast' },
            builtin: {
              GeneralPurpose: { kind: 'fixed', model_id: 'primary' },
            },
            fork: { kind: 'inherit' },
          },
        } as T;
      }
      throw error;
    }
  }

  async getOptionalConfig<T = any>(path: string): Promise<T | undefined> {
    try {
      const pendingMutations = this.waitForRelatedMutations([path]);
      if (pendingMutations) {
        await pendingMutations;
      }

      if (this.configCache.has(path)) {
        return this.configCache.get(path);
      }

      const bootstrap = this.consumeBootstrapOptionalConfig<T>(path);
      if (bootstrap.available) {
        return bootstrap.value;
      }

      const readKey = this.getReadKey(path, 'optional');
      const existingRead = this.inFlightReads.get(readKey);
      if (existingRead) {
        return (await existingRead) as T | undefined;
      }

      const readPromise = this.readOptionalConfig<T>(path);
      this.inFlightReads.set(readKey, readPromise);
      try {
        return await readPromise;
      } finally {
        if (this.inFlightReads.get(readKey) === readPromise) {
          this.inFlightReads.delete(readKey);
        }
      }
    } catch (error) {
      log.error('Failed to get optional config', { path, error });
      throw error;
    }
  }

  async getConfigs(paths: string[]): Promise<Record<string, unknown>> {
    const uniquePaths = Array.from(new Set(paths));
    const pendingMutations = this.waitForRelatedMutations(uniquePaths);
    if (pendingMutations) {
      await pendingMutations;
    }
    const results: Record<string, unknown> = {};
    const pendingReads: Array<[string, Promise<unknown>]> = [];
    const missingPaths: string[] = [];

    for (const path of uniquePaths) {
      if (this.configCache.has(path)) {
        results[path] = this.configCache.get(path);
        continue;
      }

      const existingRead = this.inFlightReads.get(this.getReadKey(path));
      if (existingRead) {
        pendingReads.push([path, existingRead]);
        continue;
      }

      missingPaths.push(path);
    }

    const batchRead = missingPaths.length > 0
      ? this.readConfigs(missingPaths)
      : undefined;
    const perPathReads = new Map<string, Promise<unknown>>();
    if (batchRead) {
      for (const path of missingPaths) {
        const perPathRead = batchRead.then(configs => configs[path]);
        void perPathRead.catch(() => undefined);
        perPathReads.set(path, perPathRead);
        this.inFlightReads.set(this.getReadKey(path), perPathRead);
      }
    }

    let fatalError: unknown;

    try {
      for (const [path, pendingRead] of pendingReads) {
        try {
          results[path] = await pendingRead;
        } catch (error) {
          try {
            results[path] = this.fallbackOrThrow(path, error);
          } catch (fallbackError) {
            fatalError ??= fallbackError;
          }
        }
      }

      if (batchRead) {
        try {
          const batchResults = await batchRead;
          Object.assign(results, batchResults);
        } catch (error) {
          log.error('Failed to get configs', { paths: missingPaths, error });
          for (const path of missingPaths) {
            try {
              results[path] = this.fallbackOrThrow(path, error);
            } catch (fallbackError) {
              fatalError ??= fallbackError;
            }
          }
        }
      }
    } finally {
      for (const [path, perPathRead] of perPathReads) {
        const readKey = this.getReadKey(path);
        if (this.inFlightReads.get(readKey) === perPathRead) {
          this.inFlightReads.delete(readKey);
        }
      }
    }

    if (fatalError) {
      throw fatalError;
    }

    return results;
  }

  async setConfig<T = any>(path: string, value: T): Promise<void> {
    try {
      const oldValue = this.configCache.get(path);
      await this.runMutation(
        path,
        () => configAPI.setConfig(path, value),
        () => {
          this.configCache.set(path, value);
          this.notifyConfigChange(path, oldValue, value);
        },
      );
    } catch (error) {
      log.error('Failed to set config', { path, error });
      throw error;
    }
  }

  async resetConfig(path?: string): Promise<void> {
    try {
      await this.runMutation(path, () => configAPI.resetConfig(path));
    } catch (error) {
      log.error('Failed to reset config', { path, error });
      throw error;
    }
  }

  async validateConfig(): Promise<ConfigValidationResult> {
    try {
      
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<ConfigValidationResult>('validate_config');
      return result;
    } catch (error) {
      log.error('Failed to validate config', error);
      return {
        valid: false,
        errors: [{ path: 'root', message: i18nService.t('errors:config.validationError'), code: 'VALIDATION_ERROR' }],
        warnings: []
      };
    }
  }

  async exportConfig(): Promise<ConfigExport> {
    try {
      const exportData = await configAPI.exportConfig();
      return exportData;
    } catch (error) {
      log.error('Failed to export config', error);
      throw error;
    }
  }

  async importConfig(config: ConfigExport): Promise<void> {
    try {
      await this.runMutation(undefined, () => configAPI.importConfig(config));
    } catch (error) {
      log.error('Failed to import config', error);
      throw error;
    }
  }

  

  onConfigChange(callback: (path: string, oldValue: any, newValue: any) => void): () => void {
    this.listeners.add(callback);
    return () => {
      this.listeners.delete(callback);
    };
  }

  async refreshCache(): Promise<void> {
    try {
      this.bumpOverlappingMutationVersions(undefined);
      this.invalidateOverlappingLocalState(undefined);
    } catch (error) {
      log.error('Failed to refresh cache', error);
    }
  }

  clearCache(): void {
    this.bumpOverlappingMutationVersions(undefined);
    this.invalidateOverlappingLocalState(undefined);
  }

  
  private notifyConfigChange(path: string, oldValue: any, newValue: any): void {
    this.listeners.forEach(callback => {
      try {
        callback(path, oldValue, newValue);
      } catch (error) {
        log.error('Config change notification failed', { path, error });
      }
    });
    
    
    for (const [watchedPath, pathCallbacks] of this.pathListeners) {
      if (!this.configPathsOverlap(watchedPath, path)) {
        continue;
      }
      pathCallbacks.forEach(callback => {
        try {
          callback();
        } catch (error) {
          log.error('Path listener notification failed', { path, watchedPath, error });
        }
      });
    }
  }

  
  
  
  get<T = any>(path: string, defaultValue?: T): T {
    if (this.configCache.has(path)) {
      const value = this.configCache.get(path);
      return value !== undefined ? value : (defaultValue as T);
    }
    return defaultValue as T;
  }
  
  
  async set<T = any>(path: string, value: T): Promise<void> {
    return this.setConfig(path, value);
  }
  
  
  watch(path: string, callback: () => void): () => void {
    if (!this.pathListeners.has(path)) {
      this.pathListeners.set(path, new Set());
    }
    
    const pathCallbacks = this.pathListeners.get(path)!;
    pathCallbacks.add(callback);
    
    
    return () => {
      pathCallbacks.delete(callback);
      if (pathCallbacks.size === 0) {
        this.pathListeners.delete(path);
      }
    };
  }
  
  
  async reload(): Promise<void> {
    try {
      this.clearCache();

      await this.getConfigs([
        'ai.models',
        'ai.agent_model_defaults',
        'ai.func_agent_models',
        'ai.default_models',
      ]);
    } catch (error) {
      log.error('Failed to reload config', error);
      throw error;
    }
  }

  /**
   * Re-read every cached/watched path after the backend applied an external
   * config change (e.g. account cloud sync), then notify listeners only for
   * paths whose value actually changed so config-driven UI refreshes.
   */
  async applyExternalReload(): Promise<void> {
    const trackedPaths = new Set<string>([
      ...this.configCache.keys(),
      ...this.pathListeners.keys(),
    ]);

    const previousValues = new Map<string, unknown>();
    for (const path of trackedPaths) {
      if (this.configCache.has(path)) {
        previousValues.set(path, this.configCache.get(path));
      }
    }

    this.clearCache();

    if (trackedPaths.size > 0) {
      try {
        await this.readConfigs([...trackedPaths]);
      } catch (error) {
        // The cache is already cleared, so later reads still pick up fresh
        // values; only listener notification is skipped.
        log.error('Failed to re-read config after external change', error);
        return;
      }
    }

    for (const path of trackedPaths) {
      const oldValue = previousValues.get(path);
      const newValue = this.configCache.get(path);
      if (!configValuesEqual(oldValue, newValue)) {
        this.notifyConfigChange(path, oldValue, newValue);
      }
    }
  }
}


export const configManager = new ConfigManagerImpl();

export default configManager;
