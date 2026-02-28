import type { ModelConfig } from '../types/installer';

export type ApiFormat = 'openai' | 'anthropic';

export interface ProviderUrlOption {
  url: string;
  format: ApiFormat;
  noteKey?: string;
}

export interface ProviderTemplate {
  id: string;
  nameKey: string;
  descriptionKey: string;
  baseUrl: string;
  format: ApiFormat;
  models: string[];
  helpUrl?: string;
  baseUrlOptions?: ProviderUrlOption[];
}

export const PROVIDER_DISPLAY_ORDER: string[] = [
  'zhipu',
  'qwen',
  'deepseek',
  'volcengine',
  'minimax',
  'moonshot',
  'anthropic',
];

export const PROVIDER_TEMPLATES: Record<string, ProviderTemplate> = {
  anthropic: {
    id: 'anthropic',
    nameKey: 'model.providers.anthropic.name',
    descriptionKey: 'model.providers.anthropic.description',
    baseUrl: 'https://api.anthropic.com',
    format: 'anthropic',
    models: ['claude-opus-4-6', 'claude-sonnet-4-5-20250929', 'claude-opus-4-5-20251101', 'claude-haiku-4-5-20251001'],
    helpUrl: 'https://console.anthropic.com/',
  },
  minimax: {
    id: 'minimax',
    nameKey: 'model.providers.minimax.name',
    descriptionKey: 'model.providers.minimax.description',
    baseUrl: 'https://api.minimaxi.com/anthropic',
    format: 'anthropic',
    models: ['MiniMax-M2.1', 'MiniMax-M2.1-lightning', 'MiniMax-M2'],
    helpUrl: 'https://platform.minimax.io/',
  },
  moonshot: {
    id: 'moonshot',
    nameKey: 'model.providers.moonshot.name',
    descriptionKey: 'model.providers.moonshot.description',
    baseUrl: 'https://api.moonshot.cn/v1',
    format: 'openai',
    models: ['kimi-k2.5', 'kimi-k2', 'kimi-k2-thinking'],
    helpUrl: 'https://platform.moonshot.ai/console',
  },
  deepseek: {
    id: 'deepseek',
    nameKey: 'model.providers.deepseek.name',
    descriptionKey: 'model.providers.deepseek.description',
    baseUrl: 'https://api.deepseek.com',
    format: 'openai',
    models: ['deepseek-chat', 'deepseek-reasoner'],
    helpUrl: 'https://platform.deepseek.com/api_keys',
  },
  zhipu: {
    id: 'zhipu',
    nameKey: 'model.providers.zhipu.name',
    descriptionKey: 'model.providers.zhipu.description',
    baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
    format: 'openai',
    models: ['glm-5', 'glm-4.7'],
    helpUrl: 'https://open.bigmodel.cn/usercenter/apikeys',
    baseUrlOptions: [
      {
        url: 'https://open.bigmodel.cn/api/paas/v4',
        format: 'openai',
        noteKey: 'model.providers.zhipu.urlOptions.default',
      },
      {
        url: 'https://open.bigmodel.cn/api/anthropic',
        format: 'anthropic',
        noteKey: 'model.providers.zhipu.urlOptions.anthropic',
      },
      {
        url: 'https://open.bigmodel.cn/api/coding/paas',
        format: 'openai',
        noteKey: 'model.providers.zhipu.urlOptions.codingPlan',
      },
    ],
  },
  qwen: {
    id: 'qwen',
    nameKey: 'model.providers.qwen.name',
    descriptionKey: 'model.providers.qwen.description',
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    format: 'openai',
    models: ['qwen3-max', 'qwen3-coder-plus', 'qwen3-coder-flash'],
    helpUrl: 'https://dashscope.console.aliyun.com/apiKey',
  },
  volcengine: {
    id: 'volcengine',
    nameKey: 'model.providers.volcengine.name',
    descriptionKey: 'model.providers.volcengine.description',
    baseUrl: 'https://ark.cn-beijing.volces.com/api/v3',
    format: 'openai',
    models: ['glm-4-7-251222', 'doubao-seed-code-preview-251028'],
    helpUrl: 'https://console.volcengine.com/ark/',
  },
};

export function getOrderedProviders(): ProviderTemplate[] {
  const ordered: ProviderTemplate[] = [];
  for (const id of PROVIDER_DISPLAY_ORDER) {
    const template = PROVIDER_TEMPLATES[id];
    if (template) ordered.push(template);
  }
  for (const template of Object.values(PROVIDER_TEMPLATES)) {
    if (!PROVIDER_DISPLAY_ORDER.includes(template.id)) {
      ordered.push(template);
    }
  }
  return ordered;
}

export function resolveProviderFormat(template: ProviderTemplate, baseUrl: string): ApiFormat {
  if (template.baseUrlOptions && template.baseUrlOptions.length > 0) {
    const selected = template.baseUrlOptions.find((item) => item.url === baseUrl.trim());
    if (selected) return selected.format;
  }
  return template.format;
}

export function createModelConfigFromTemplate(
  template: ProviderTemplate,
  previous: ModelConfig | null
): ModelConfig {
  const modelName = previous?.modelName?.trim() || template.models[0] || '';
  const baseUrl = previous?.baseUrl?.trim() || template.baseUrl;
  return {
    provider: template.id,
    apiKey: previous?.apiKey || '',
    modelName,
    baseUrl,
    format: resolveProviderFormat(template, baseUrl),
    configName: `${template.id} - ${modelName}`.trim(),
    customRequestBody: previous?.customRequestBody,
    skipSslVerify: previous?.skipSslVerify,
    customHeaders: previous?.customHeaders,
    customHeadersMode: previous?.customHeadersMode || 'merge',
  };
}
