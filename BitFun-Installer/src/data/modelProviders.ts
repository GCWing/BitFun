import type { ModelConfig } from '../types/installer';

/** Matches main app `src/web-ui/.../modelConfigs.ts` ApiFormat for presets. */
export type ApiFormat = 'openai' | 'anthropic' | 'gemini' | 'responses';

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

/** Same order as `AIModelConfig.tsx` `providerOrder`. */
export const PROVIDER_DISPLAY_ORDER: string[] = [
  'openbitfun',
  'zhipu',
  'qwen',
  'deepseek',
  'volcengine',
  'minimax',
  'moonshot',
  'gemini',
  'anthropic',
  'siliconflow',
  'nvidia',
  'openrouter',
];

export const PROVIDER_TEMPLATES: Record<string, ProviderTemplate> = {
  openbitfun: {
    id: 'openbitfun',
    nameKey: 'model.providers.openbitfun.name',
    descriptionKey: 'model.providers.openbitfun.description',
    baseUrl: 'https://api.openbitfun.com',
    format: 'anthropic',
    models: [],
  },
  gemini: {
    id: 'gemini',
    nameKey: 'model.providers.gemini.name',
    descriptionKey: 'model.providers.gemini.description',
    baseUrl: 'https://generativelanguage.googleapis.com',
    format: 'gemini',
    models: ['gemini-3.1-pro-preview', 'gemini-3.1-flash-lite-preview'],
    helpUrl: 'https://aistudio.google.com/app/apikey',
  },
  anthropic: {
    id: 'anthropic',
    nameKey: 'model.providers.anthropic.name',
    descriptionKey: 'model.providers.anthropic.description',
    baseUrl: 'https://api.anthropic.com',
    format: 'anthropic',
    models: ['claude-opus-4-6', 'claude-sonnet-4-6'],
    helpUrl: 'https://console.anthropic.com/',
  },
  minimax: {
    id: 'minimax',
    nameKey: 'model.providers.minimax.name',
    descriptionKey: 'model.providers.minimax.description',
    baseUrl: 'https://api.minimaxi.com/anthropic',
    format: 'anthropic',
    models: ['MiniMax-M2.7-highspeed', 'MiniMax-M2.5-highspeed'],
    helpUrl: 'https://platform.minimax.io/',
    baseUrlOptions: [
      {
        url: 'https://api.minimaxi.com/anthropic',
        format: 'anthropic',
        noteKey: 'model.providers.minimax.urlOptions.default',
      },
      {
        url: 'https://api.minimaxi.com/v1',
        format: 'openai',
        noteKey: 'model.providers.minimax.urlOptions.openai',
      },
    ],
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
    baseUrl: 'https://api.deepseek.com/v1',
    format: 'openai',
    models: ['deepseek-v4-flash', 'deepseek-v4-pro'],
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
        url: 'https://open.bigmodel.cn/api/coding/paas/v4',
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
    models: ['Qwen3.5-Plus', 'Qwen3.5-Flash'],
    helpUrl: 'https://dashscope.console.aliyun.com/apiKey',
    baseUrlOptions: [
      {
        url: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
        format: 'openai',
        noteKey: 'model.providers.qwen.urlOptions.default',
      },
      {
        url: 'https://coding.dashscope.aliyuncs.com/v1',
        format: 'openai',
        noteKey: 'model.providers.qwen.urlOptions.codingPlan',
      },
      {
        url: 'https://coding.dashscope.aliyuncs.com/apps/anthropic',
        format: 'anthropic',
        noteKey: 'model.providers.qwen.urlOptions.codingPlanAnthropic',
      },
    ],
  },
  volcengine: {
    id: 'volcengine',
    nameKey: 'model.providers.volcengine.name',
    descriptionKey: 'model.providers.volcengine.description',
    baseUrl: 'https://ark.cn-beijing.volces.com/api/v3',
    format: 'openai',
    models: ['doubao-seed-2-0-code-preview-260215', 'doubao-seed-2-0-pro-260215'],
    helpUrl: 'https://console.volcengine.com/ark/',
  },
  siliconflow: {
    id: 'siliconflow',
    nameKey: 'model.providers.siliconflow.name',
    descriptionKey: 'model.providers.siliconflow.description',
    baseUrl: 'https://api.siliconflow.cn/v1',
    format: 'openai',
    models: [],
    helpUrl: 'https://cloud.siliconflow.cn/account/ak',
    baseUrlOptions: [
      {
        url: 'https://api.siliconflow.cn/v1',
        format: 'openai',
        noteKey: 'model.providers.siliconflow.urlOptions.default',
      },
      {
        url: 'https://api.siliconflow.cn/v1/messages',
        format: 'anthropic',
        noteKey: 'model.providers.siliconflow.urlOptions.anthropic',
      },
    ],
  },
  nvidia: {
    id: 'nvidia',
    nameKey: 'model.providers.nvidia.name',
    descriptionKey: 'model.providers.nvidia.description',
    baseUrl: 'https://integrate.api.nvidia.com/v1',
    format: 'openai',
    models: [],
    helpUrl: 'https://build.nvidia.com/settings/api-keys',
  },
  openrouter: {
    id: 'openrouter',
    nameKey: 'model.providers.openrouter.name',
    descriptionKey: 'model.providers.openrouter.description',
    baseUrl: 'https://openrouter.ai/api/v1',
    format: 'openai',
    models: [],
    helpUrl: 'https://openrouter.ai/keys',
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
