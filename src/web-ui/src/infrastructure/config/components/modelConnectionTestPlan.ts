import type { AIModelConfig } from '../types';

function normalizeComparableString(value: string | undefined): string {
  return (value || '').trim();
}

export function stableJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(',')}]`;
  }
  if (value && typeof value === 'object') {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, entryValue]) => `${JSON.stringify(key)}:${stableJson(entryValue)}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

export function providerConnectionChanged(
  previous: AIModelConfig | undefined,
  next: AIModelConfig,
): boolean {
  if (!previous) return true;

  return (
    normalizeComparableString(previous.provider) !== normalizeComparableString(next.provider)
    || normalizeComparableString(previous.base_url) !== normalizeComparableString(next.base_url)
    || normalizeComparableString(previous.api_key) !== normalizeComparableString(next.api_key)
    || stableJson(previous.auth || { type: 'api_key' }) !== stableJson(next.auth || { type: 'api_key' })
    || stableJson(previous.custom_headers || {}) !== stableJson(next.custom_headers || {})
    || normalizeComparableString(previous.custom_headers_mode) !== normalizeComparableString(next.custom_headers_mode)
    || normalizeComparableString(previous.custom_request_body) !== normalizeComparableString(next.custom_request_body)
    || normalizeComparableString(previous.custom_request_body_mode) !== normalizeComparableString(next.custom_request_body_mode)
    || (previous.skip_ssl_verify ?? false) !== (next.skip_ssl_verify ?? false)
  );
}

function supportsImageProbe(config: AIModelConfig): boolean {
  return config.category === 'multimodal'
    || config.capabilities.some(capability => capability === 'image_understanding');
}

function modelProbeChanged(previous: AIModelConfig, next: AIModelConfig): boolean {
  return (
    normalizeComparableString(previous.model_name) !== normalizeComparableString(next.model_name)
    || normalizeComparableString(previous.request_url) !== normalizeComparableString(next.request_url)
    || supportsImageProbe(previous) !== supportsImageProbe(next)
  );
}

function uniqueConfigs(configs: AIModelConfig[]): AIModelConfig[] {
  const seen = new Set<string>();
  return configs.filter(config => {
    const key = config.id || `${config.provider}:${config.base_url}:${config.model_name}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

/**
 * Plan the expensive post-save model probes.
 *
 * A provider probe is a real model generation (and can include a second image
 * request), so shared connection edits need one representative model rather
 * than an unbounded fan-out. Newly added or probe-relevant models are still
 * checked individually. Local metadata such as context limits and reasoning
 * presets does not require a connection probe.
 */
export function configsNeedingAutoTest(
  previousModels: AIModelConfig[],
  nextConfigs: AIModelConfig[],
  isProviderGroupEdit: boolean,
): AIModelConfig[] {
  const previousById = new Map(previousModels.map(model => [model.id, model]));
  const added = nextConfigs.filter(config => !previousById.get(config.id));

  // A brand-new provider may contain several selected models. One successful
  // representative request is enough for automatic onboarding verification;
  // every model remains available for an explicit per-model test.
  if (!isProviderGroupEdit && added.length === nextConfigs.length) {
    return nextConfigs.slice(0, 1);
  }

  const providerChanged = nextConfigs.filter(config => {
    const previous = previousById.get(config.id);
    return previous ? providerConnectionChanged(previous, config) : false;
  });
  const modelProbeChangedConfigs = nextConfigs.filter(config => {
    const previous = previousById.get(config.id);
    return previous ? modelProbeChanged(previous, config) : false;
  });

  if (!isProviderGroupEdit) {
    return uniqueConfigs([...added, ...providerChanged, ...modelProbeChangedConfigs]);
  }

  if (providerChanged.length === 0) {
    return uniqueConfigs([...added, ...modelProbeChangedConfigs]);
  }

  const representative = added[0] || modelProbeChangedConfigs[0] || providerChanged[0];
  return uniqueConfigs([
    ...(representative ? [representative] : []),
    ...added,
    ...modelProbeChangedConfigs,
  ]);
}
