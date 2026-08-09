import providerOverlay from '../../../src/shared/ai-provider-catalog/providers.json';

import type { ModelConfig } from '../types/installer';

/** Matches main app `ApiFormat` for installer presets. */
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

interface OverlayEndpoint {
  base_url: string;
  api_format: string;
  label: string;
  is_default?: boolean;
}

interface OverlayProvider {
  id: string;
  display_order: number;
  help_url?: string;
  endpoints: OverlayEndpoint[];
  model_policy: { curated_models: string[]; additional_models?: string[] };
}

const overlayProviders = (providerOverlay as { providers: OverlayProvider[] }).providers;

function isApiFormat(value: string): value is ApiFormat {
  return ['openai', 'anthropic', 'gemini', 'responses'].includes(value);
}

function templateFromOverlay(provider: OverlayProvider): ProviderTemplate {
  const defaultEndpoint = provider.endpoints.find(endpoint => endpoint.is_default)
    ?? provider.endpoints[0];
  if (!defaultEndpoint || !isApiFormat(defaultEndpoint.api_format)) {
    throw new Error(`Installer provider '${provider.id}' has no supported default endpoint`);
  }
  const baseUrlOptions = provider.endpoints.length > 1
    ? provider.endpoints.map(endpoint => {
        if (!isApiFormat(endpoint.api_format)) {
          throw new Error(`Installer provider '${provider.id}' has an unsupported API format`);
        }
        return {
          url: endpoint.base_url,
          format: endpoint.api_format,
          noteKey: `model.providers.${provider.id}.urlOptions.${endpoint.label}`,
        };
      })
    : undefined;
  return {
    id: provider.id,
    nameKey: `model.providers.${provider.id}.name`,
    descriptionKey: `model.providers.${provider.id}.description`,
    baseUrl: defaultEndpoint.base_url,
    format: defaultEndpoint.api_format,
    models: [
      ...provider.model_policy.curated_models,
      ...(provider.model_policy.additional_models ?? []),
    ],
    helpUrl: provider.help_url,
    baseUrlOptions,
  };
}

export const PROVIDER_DISPLAY_ORDER: string[] = [...overlayProviders]
  .sort((left, right) => left.display_order - right.display_order)
  .map(provider => provider.id);

export const PROVIDER_TEMPLATES: Record<string, ProviderTemplate> = Object.fromEntries(
  overlayProviders.map(provider => [provider.id, templateFromOverlay(provider)]),
);

export function getOrderedProviders(): ProviderTemplate[] {
  const ordered: ProviderTemplate[] = [];
  for (const id of PROVIDER_DISPLAY_ORDER) {
    const template = PROVIDER_TEMPLATES[id];
    if (template) ordered.push(template);
  }
  for (const template of Object.values(PROVIDER_TEMPLATES)) {
    if (!PROVIDER_DISPLAY_ORDER.includes(template.id)) ordered.push(template);
  }
  return ordered;
}

export function resolveProviderFormat(template: ProviderTemplate, baseUrl: string): ApiFormat {
  if (template.baseUrlOptions && template.baseUrlOptions.length > 0) {
    const selected = template.baseUrlOptions.find(item => item.url === baseUrl.trim());
    if (selected) return selected.format;
  }
  return template.format;
}

export function createModelConfigFromTemplate(
  template: ProviderTemplate,
  previous: ModelConfig | null,
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
