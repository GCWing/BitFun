/**
 * Copied from main app settings (AIModelConfig) request URL helpers — installer-local only.
 */

export function isResponsesProvider(provider?: string): boolean {
  return provider === 'response' || provider === 'responses';
}

/**
 * Stored request URL (matches settings `resolveRequestUrl`).
 */
export function resolveRequestUrl(baseUrl: string, provider: string, _modelName = ''): string {
  const trimmed = baseUrl.trim().replace(/\/+$/, '');
  if (trimmed.endsWith('#')) {
    return trimmed.slice(0, -1).replace(/\/+$/, '');
  }
  if (provider === 'openai') {
    return trimmed.endsWith('chat/completions') ? trimmed : `${trimmed}/chat/completions`;
  }
  if (isResponsesProvider(provider)) {
    return trimmed.endsWith('responses') ? trimmed : `${trimmed}/responses`;
  }
  if (provider === 'anthropic') {
    return trimmed.endsWith('v1/messages') ? trimmed : `${trimmed}/v1/messages`;
  }
  if (provider === 'gemini') {
    return geminiBaseUrl(trimmed);
  }
  return trimmed;
}

export function geminiBaseUrl(url: string): string {
  return url
    .replace(/\/v1beta(?:\/models(?:\/[^/?#]*(?::(?:stream)?[Gg]enerateContent)?(?:\?[^]*)?)?)?$/, '')
    .replace(/\/models(?:\/[^/?#]*(?::(?:stream)?[Gg]enerateContent)?(?:\?[^]*)?)?$/, '')
    .replace(/\/+$/, '');
}

/**
 * Read-only preview for UI (matches settings `previewRequestUrl`).
 */
export function previewRequestUrl(baseUrl: string, provider: string): string {
  if (provider === 'gemini') {
    return `${geminiBaseUrl(baseUrl.trim().replace(/\/+$/, ''))}/v1beta/models/...`;
  }
  return resolveRequestUrl(baseUrl, provider);
}
