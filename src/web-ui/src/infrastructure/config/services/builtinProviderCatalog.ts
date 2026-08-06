import overlayDocument from '../../../../../shared/ai-provider-catalog/providers.json';

import type { ProviderCatalog, ProviderCatalogProvider } from '@/infrastructure/api/service-api/AIApi';
import type { ApiFormat, ProviderRegion, ProviderTemplate } from '@/shared/types';

interface RawEndpoint {
  id: string;
  base_url: string;
  api_format: string;
  label: string;
  is_default?: boolean;
}

interface RawProvider {
  id: string;
  display_order: number;
  region?: string;
  name: string;
  description: string;
  help_url?: string;
  requires_api_key: boolean;
  endpoints: RawEndpoint[];
  model_policy: { curated_models: string[]; additional_models?: string[] };
}

const overlayProviders = (overlayDocument as { providers: RawProvider[] }).providers;

function isApiFormat(value: string): value is ApiFormat {
  return ['openai', 'responses', 'anthropic', 'gemini', 'gemini-code-assist'].includes(value);
}

function toRegion(value: string | undefined): ProviderRegion {
  return value === 'cn' || value === 'global' ? value : 'any';
}

function toTemplate(provider: RawProvider): ProviderTemplate {
  const defaultEndpoint = provider.endpoints.find(endpoint => endpoint.is_default)
    ?? provider.endpoints[0];
  if (!defaultEndpoint || !isApiFormat(defaultEndpoint.api_format)) {
    throw new Error(`Built-in provider '${provider.id}' has no supported default endpoint`);
  }
  return {
    id: provider.id,
    displayOrder: provider.display_order,
    region: toRegion(provider.region),
    name: provider.name,
    baseUrl: defaultEndpoint.base_url,
    format: defaultEndpoint.api_format,
    models: [
      ...provider.model_policy.curated_models,
      ...(provider.model_policy.additional_models ?? []),
    ],
    requiresApiKey: provider.requires_api_key,
    description: provider.description,
    helpUrl: provider.help_url,
    baseUrlOptions: provider.endpoints.length > 1
      ? provider.endpoints.map(endpoint => {
          if (!isApiFormat(endpoint.api_format)) {
            throw new Error(
              `Built-in provider '${provider.id}' endpoint '${endpoint.id}' has an unsupported API format`,
            );
          }
          return { url: endpoint.base_url, format: endpoint.api_format, note: endpoint.label };
        })
      : undefined,
  };
}

function resolvedProviderToTemplate(
  resolved: ProviderCatalogProvider,
  fallback: ProviderTemplate,
): ProviderTemplate {
  return {
    ...fallback,
    models: resolved.models.map(model => model.id),
  };
}

export const BUILTIN_PROVIDER_TEMPLATES: Record<string, ProviderTemplate> = Object.fromEntries(
  overlayProviders.map(provider => [provider.id, toTemplate(provider)]),
);

export function resolveProviderTemplates(
  catalog?: ProviderCatalog | null,
): Record<string, ProviderTemplate> {
  if (!catalog?.providers?.length) return BUILTIN_PROVIDER_TEMPLATES;
  const resolvedById = new Map(catalog.providers.map(provider => [provider.id, provider]));
  return Object.fromEntries(
    Object.values(BUILTIN_PROVIDER_TEMPLATES).map(fallback => {
      const resolved = resolvedById.get(fallback.id);
      return [fallback.id, resolved ? resolvedProviderToTemplate(resolved, fallback) : fallback];
    }),
  );
}
