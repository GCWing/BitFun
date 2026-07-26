export interface ProviderUrlCatalogItem {
  id: string;
  baseUrl: string;
  baseUrlOptions?: string[];
}

export const PROVIDER_URL_CATALOG: ProviderUrlCatalogItem[] = [
  {
    id: 'openbitfun',
    baseUrl: 'https://api.openbitfun.com',
  },
  {
    id: 'gemini',
    baseUrl: 'https://generativelanguage.googleapis.com',
  },
  {
    id: 'anthropic',
    baseUrl: 'https://api.anthropic.com',
  },
  {
    id: 'minimax',
    baseUrl: 'https://api.minimaxi.com/anthropic',
    baseUrlOptions: [
      'https://api.minimaxi.com/anthropic',
      'https://api.minimaxi.com/v1',
      'https://api.minimax.io/anthropic',
      'https://api.minimax.io/v1',
    ],
  },
  {
    id: 'moonshot',
    baseUrl: 'https://api.moonshot.cn/v1',
    baseUrlOptions: [
      'https://api.moonshot.cn/v1',
      'https://api.moonshot.cn/anthropic',
      'https://api.moonshot.ai/v1',
      'https://api.moonshot.ai/anthropic',
      'https://api.kimi.com/coding',
    ],
  },
  {
    id: 'deepseek',
    baseUrl: 'https://api.deepseek.com/v1',
    baseUrlOptions: [
      'https://api.deepseek.com/v1',
      'https://api.deepseek.com/anthropic',
    ],
  },
  {
    id: 'zhipu',
    baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
    baseUrlOptions: [
      'https://open.bigmodel.cn/api/paas/v4',
      'https://open.bigmodel.cn/api/anthropic',
      'https://open.bigmodel.cn/api/coding/paas/v4',
      'https://api.z.ai/api/paas/v4',
      'https://api.z.ai/api/anthropic',
      'https://api.z.ai/api/coding/paas/v4',
    ],
  },
  {
    id: 'qwen',
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    baseUrlOptions: [
      'https://dashscope.aliyuncs.com/compatible-mode/v1',
      'https://dashscope-intl.aliyuncs.com/compatible-mode/v1',
      'https://coding.dashscope.aliyuncs.com/v1',
      'https://coding.dashscope.aliyuncs.com/apps/anthropic',
      'https://coding-intl.dashscope.aliyuncs.com/v1',
    ],
  },
  {
    id: 'volcengine',
    baseUrl: 'https://ark.cn-beijing.volces.com/api/v3',
    baseUrlOptions: [
      'https://ark.cn-beijing.volces.com/api/v3',
      'https://ark.cn-beijing.volces.com/api/coding/v3',
      'https://ark.ap-southeast.bytepluses.com/api/v3',
    ],
  },
  {
    id: 'siliconflow',
    baseUrl: 'https://api.siliconflow.cn/v1',
    baseUrlOptions: [
      'https://api.siliconflow.cn/v1',
      'https://api.siliconflow.cn/v1/messages',
      'https://api.siliconflow.com/v1',
    ],
  },
  {
    id: 'nvidia',
    baseUrl: 'https://integrate.api.nvidia.com/v1',
  },
  {
    id: 'openrouter',
    baseUrl: 'https://openrouter.ai/api/v1',
  },
];

export function normalizeProviderBaseUrl(url: string): string {
  let normalized = url.trim().replace(/#$/, '').replace(/\/+$/, '');

  normalized = normalized
    .replace(/\/chat\/completions$/i, '')
    .replace(/\/responses$/i, '')
    .replace(/\/v1\/messages$/i, '');

  const geminiModelEndpointMatch = normalized.match(/^(.*)\/models\/[^/?#:]+(?::[^/?#]+)?(?:\?.*)?$/i);
  if (geminiModelEndpointMatch?.[1]) {
    normalized = geminiModelEndpointMatch[1].replace(/\/+$/, '');
  }

  normalized = normalized.replace(/\/v1beta$/i, '');

  return normalized;
}

export function matchProviderCatalogItemByBaseUrl(
  baseUrl?: string,
  catalog: ProviderUrlCatalogItem[] = PROVIDER_URL_CATALOG
): ProviderUrlCatalogItem | undefined {
  const configBaseUrl = baseUrl?.trim();
  if (!configBaseUrl) return undefined;

  const normalizedConfigBaseUrl = normalizeProviderBaseUrl(configBaseUrl);
  const candidates = catalog.flatMap(item => {
    const urls = [item.baseUrl, ...(item.baseUrlOptions || [])];
    return urls.map(url => ({
      item,
      normalizedUrl: normalizeProviderBaseUrl(url),
    }));
  });

  const matched = candidates
    .filter(candidate => (
      normalizedConfigBaseUrl === candidate.normalizedUrl ||
      normalizedConfigBaseUrl.startsWith(`${candidate.normalizedUrl}/`) ||
      candidate.normalizedUrl.startsWith(`${normalizedConfigBaseUrl}/`)
    ))
    .sort((a, b) => b.normalizedUrl.length - a.normalizedUrl.length)[0];

  return matched?.item;
}

export function extractProviderSegmentFromBaseUrl(baseUrl?: string): string | undefined {
  const rawBaseUrl = baseUrl?.trim();
  if (!rawBaseUrl) return undefined;

  try {
    const hostname = new URL(rawBaseUrl).hostname.toLowerCase();
    const parts = hostname.split('.').filter(Boolean);
    if (parts.length >= 2) {
      return parts[parts.length - 2];
    }
    return parts[0];
  } catch {
    return undefined;
  }
}
