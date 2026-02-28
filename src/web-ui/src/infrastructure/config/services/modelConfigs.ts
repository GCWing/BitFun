import { ModelConfig, ProviderTemplate, ApiFormat } from '../../../shared/types';
import { configManager } from './ConfigManager';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ModelConfigManager');
const t = (key: string, options?: Record<string, unknown>) => i18nService.t(key, options);

export const PROVIDER_TEMPLATES: Record<string, ProviderTemplate> = {
  anthropic: {
    id: 'anthropic',
    name: t('settings/ai-model:providers.anthropic.name'),
    baseUrl: 'https://api.anthropic.com',
    format: 'anthropic',
    models: ['claude-opus-4-6', 'claude-sonnet-4-5-20250929', 'claude-opus-4-5-20251101', 'claude-haiku-4-5-20251001'],
    requiresApiKey: true,
    description: t('settings/ai-model:providers.anthropic.description'),
    helpUrl: 'https://console.anthropic.com/'
  },
  
  minimax: {
    id: 'minimax',
    name: t('settings/ai-model:providers.minimax.name'),
    baseUrl: 'https://api.minimaxi.com/anthropic',
    format: 'anthropic',
    models: ['MiniMax-M2.1', 'MiniMax-M2.1-lightning', 'MiniMax-M2'],
    requiresApiKey: true,
    description: t('settings/ai-model:providers.minimax.description'),
    helpUrl: 'https://platform.minimax.io/'
  },

  moonshot: {
    id: 'moonshot',
    name: t('settings/ai-model:providers.moonshot.name'),
    baseUrl: 'https://api.moonshot.cn/v1',
    format: 'openai',
    models: ['kimi-k2.5', 'kimi-k2', 'kimi-k2-thinking'],
    requiresApiKey: true,
    description: t('settings/ai-model:providers.moonshot.description'),
    helpUrl: 'https://platform.moonshot.ai/console'
  },

  deepseek: {
    id: 'deepseek',
    name: t('settings/ai-model:providers.deepseek.name'),
    baseUrl: 'https://api.deepseek.com',
    format: 'openai',
    models: ['deepseek-chat', 'deepseek-reasoner'],
    requiresApiKey: true,
    description: t('settings/ai-model:providers.deepseek.description'),
    helpUrl: 'https://platform.deepseek.com/api_keys'
  },

  zhipu: {
    id: 'zhipu',
    name: t('settings/ai-model:providers.zhipu.name'),
    baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
    format: 'openai',
    models: ['glm-5', 'glm-4.7'],
    requiresApiKey: true,
    description: t('settings/ai-model:providers.zhipu.description'),
    helpUrl: 'https://open.bigmodel.cn/usercenter/apikeys',
    baseUrlOptions: [
      { url: 'https://open.bigmodel.cn/api/paas/v4', format: 'openai', note: 'default' },
      { url: 'https://open.bigmodel.cn/api/anthropic', format: 'anthropic', note: 'Coding Plan' },
      { url: 'https://open.bigmodel.cn/api/coding/paas', format: 'openai', note: 'Coding Plan' },
    ]
  },

  qwen: {
    id: 'qwen',
    name: t('settings/ai-model:providers.qwen.name'),
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    format: 'openai',
    models: ['qwen3.5-plus', 'glm-5', 'kimi-k2.5', 'MiniMax-M2.5', 'qwen3-max', 'qwen3-coder-plus', 'qwen3-coder-flash'],
    requiresApiKey: true,
    description: t('settings/ai-model:providers.qwen.description'),
    helpUrl: 'https://dashscope.console.aliyun.com/apiKey',
    baseUrlOptions: [
      { url: 'https://dashscope.aliyuncs.com/compatible-mode/v1', format: 'openai', note: 'default' },
      { url: 'https://coding.dashscope.aliyuncs.com/v1', format: 'openai', note: 'Coding Plan' },
      { url: 'https://coding.dashscope.aliyuncs.com/apps/anthropic', format: 'anthropic', note: 'Coding Plan' },
    ]
  },

  volcengine: {
    id: 'volcengine',
    name: t('settings/ai-model:providers.volcengine.name'),
    baseUrl: 'https://ark.cn-beijing.volces.com/api/v3',
    format: 'openai',
    models: ['doubao-seed-1-8-251228', 'glm-4-7-251222', 'doubao-seed-code-preview-251028'],
    requiresApiKey: true,
    description: t('settings/ai-model:providers.volcengine.description'),
    helpUrl: 'https://console.volcengine.com/ark/'
  }
};

type ConfigChangeListener = (configs: ModelConfig[]) => void;

class ModelConfigManager {
  private configs: ModelConfig[] = [];
  private listeners: Set<ConfigChangeListener> = new Set();

  constructor() {
    this.loadConfigs();
  }

  // Listener management
  addListener(listener: ConfigChangeListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  // Notify listeners
  private notifyListeners(): void {
    const configsCopy = [...this.configs];
    
    this.listeners.forEach(listener => {
      try {
        listener(configsCopy);
      } catch (error) {
        log.error('Error in config change listener', error);
      }
    });
  }

  // New architecture: load via the unified config manager.
  private loadConfigs(): void {
    // Start with an empty set, then sync async.
    this.configs = [];
    
    // Async load the real config.
    this.syncFromConfigManager().catch(error => {
      log.error('Failed to load configs', error);
      this.configs = [];
      this.notifyListeners();
    });
  }

  // New architecture: sync from the unified config manager.
  private async syncFromConfigManager(): Promise<void> {
    try {
      // Fetch AI model configuration from the unified config manager.
      const aiModels = await configManager.getConfig<any[]>('ai.models');
      
      if (aiModels && aiModels.length > 0) {
        // Convert backend shape -> frontend shape.
        this.configs = aiModels.map(model => ({
          id: model.id,
          name: model.name,
          baseUrl: model.base_url,
          apiKey: model.api_key,
          modelName: model.model_name,
          format: model.provider as ApiFormat,
          description: model.description || t('settings/ai-model:messages.defaultDescription', { name: model.name }),
          isBuiltIn: false,
          contextWindow: model.context_window,
          maxTokens: model.max_tokens
        }));
      } else {
        // No config available from backend.
        this.configs = [];
      }
      
      this.notifyListeners();
    } catch (error) {
      log.error('Failed to load configs from backend', error);
      this.configs = [];
      this.notifyListeners();
    }
  }

  // New architecture: persist via the unified config manager.
  private async saveConfigs(): Promise<void> {
    try {
      // Convert to backend shape.
      const backendConfigs = this.configs.map(config => ({
        id: config.id,
        name: config.name,
        model_name: config.modelName,
        provider: config.format,
        base_url: config.baseUrl,
        api_key: config.apiKey || '',
        enabled: true,
        description: config.description,
        context_window: config.contextWindow,
        max_tokens: config.maxTokens
      }));
      
      // Save to the unified config system.
      await configManager.setConfig('ai.models', backendConfigs);
      
      this.notifyListeners();
    } catch (error) {
      log.error('Failed to save configs', error);
      throw error;
    }
  }

  // Reload configuration (public).
  async reload(): Promise<void> {
    await this.syncFromConfigManager();
  }

  // Read operations
  getAllConfigs(): ModelConfig[] {
    return [...this.configs];
  }

  getConfigById(id: string): ModelConfig | undefined {
    return this.configs.find(config => config.id === id);
  }

  // Write operations
  addConfig(config: Omit<ModelConfig, 'id'>): ModelConfig {
    const newConfig: ModelConfig = {
      ...config,
      id: this.generateId(),
    };
    this.configs.push(newConfig);
    
    // Persist async.
    this.saveConfigs().catch(error => {
      log.error('Failed to save new config', error);
    });
    
    return newConfig;
  }

  updateConfig(id: string, updates: Partial<ModelConfig>): boolean {
    const index = this.configs.findIndex(config => config.id === id);
    if (index === -1) return false;

    this.configs[index] = { ...this.configs[index], ...updates };
    
    // Persist async.
    this.saveConfigs().catch(error => {
      log.error('Failed to update config', { configId: id, error });
    });
    
    return true;
  }

  deleteConfig(id: string): boolean {
    const index = this.configs.findIndex(config => config.id === id);
    if (index === -1) return false;

    this.configs.splice(index, 1);
    
    this.saveConfigs().catch(error => {
      log.error('Failed to delete config', { configId: id, error });
    });
    
    return true;
  }

  cloneConfig(id: string): ModelConfig | null {
    const config = this.getConfigById(id);
    if (!config) return null;

    const cloned = this.addConfig({
      ...config,
      name: t('settings/ai-model:messages.cloneName', { name: config.name }),
      isBuiltIn: false
    });
    return cloned;
  }

  private generateId(): string {
    return `config_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  createFromTemplate(providerId: string, modelName: string): ModelConfig | null {
    const template = PROVIDER_TEMPLATES[providerId];
    if (!template) return null;

    return this.addConfig({
      name: t('settings/ai-model:messages.templateName', { provider: template.name, modelName }),
      baseUrl: template.baseUrl,
      modelName,
      format: template.format,
      description: t('settings/ai-model:messages.templateDescription', { description: template.description, modelName }),
      isBuiltIn: false
    });
  }

  resetToDefault(): void {
    this.configs = [];
    this.saveConfigs().catch(error => {
      log.error('Failed to reset configs', error);
    });
  }
}

export const getAllTemplates = (): ProviderTemplate[] => {
  return Object.values(PROVIDER_TEMPLATES);
};

export const getFormatDisplayName = (format: ApiFormat): string => {
  switch (format) {
    case 'openai':
      return t('settings/ai-model:formats.openaiCompatible');
    case 'anthropic':
      return t('settings/ai-model:formats.claudeApi');
    default:
      return format;
  }
};

export const modelConfigManager = new ModelConfigManager();

if (typeof window !== 'undefined' && process.env.NODE_ENV === 'development') {
  (window as any).modelConfigManager = modelConfigManager;
}
