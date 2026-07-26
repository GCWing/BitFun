import { describe, expect, it, vi } from 'vitest';

import enUS from '@/locales/en-US/settings/ai-model.json';
import zhCN from '@/locales/zh-CN/settings/ai-model.json';
import zhTW from '@/locales/zh-TW/settings/ai-model.json';

vi.mock('@/infrastructure/i18n', () => ({
  i18nService: {
    t: (key: string) => key,
  },
}));

const LOCALES = { 'en-US': enUS, 'zh-CN': zhCN, 'zh-TW': zhTW } as Record<
  string,
  { providers: Record<string, { urlOptions?: Record<string, string> }> }
>;

async function loadPresets() {
  const [{ PROVIDER_TEMPLATES }, { PROVIDER_URL_CATALOG }] = await Promise.all([
    import('./modelConfigs'),
    import('./providerCatalog'),
  ]);
  return { PROVIDER_TEMPLATES, PROVIDER_URL_CATALOG };
}

/**
 * These presets are duplicated across the web UI, the installer, the URL catalog and
 * three locale bundles, so a drift between any two of them ships a provider entry that
 * silently fails at request time or renders an untranslated key. Assert the invariants.
 */
describe('provider presets', () => {
  it('offers every preset URL through the provider URL catalog', async () => {
    const { PROVIDER_TEMPLATES, PROVIDER_URL_CATALOG } = await loadPresets();

    for (const template of Object.values(PROVIDER_TEMPLATES)) {
      const catalogItem = PROVIDER_URL_CATALOG.find(item => item.id === template.id);
      expect(catalogItem, `provider ${template.id} is missing from PROVIDER_URL_CATALOG`).toBeDefined();

      const catalogUrls = new Set([catalogItem!.baseUrl, ...(catalogItem!.baseUrlOptions ?? [])]);
      const presetUrls = new Set([
        template.baseUrl,
        ...(template.baseUrlOptions ?? []).map(option => option.url),
      ]);

      expect([...presetUrls].filter(url => !catalogUrls.has(url))).toEqual([]);
      expect([...catalogUrls].filter(url => !presetUrls.has(url))).toEqual([]);
    }
  });

  it('resolves every base URL option note in all locales, with distinct labels', async () => {
    const { PROVIDER_TEMPLATES } = await loadPresets();

    for (const template of Object.values(PROVIDER_TEMPLATES)) {
      const options = template.baseUrlOptions ?? [];
      if (options.length === 0) continue;

      const notes = options.map(option => option.note);
      expect(new Set(notes).size, `${template.id} reuses a note key`).toBe(notes.length);

      for (const [tag, bundle] of Object.entries(LOCALES)) {
        const urlOptions = bundle.providers[template.id]?.urlOptions ?? {};
        const labels = notes.map(note => urlOptions[note]);

        expect(
          labels.filter(label => !label),
          `${tag} is missing providers.${template.id}.urlOptions entries`,
        ).toEqual([]);
        // Two endpoints sharing a label leaves the user unable to tell them apart.
        expect(new Set(labels).size, `${tag} renders duplicate labels for ${template.id}`).toBe(labels.length);
        expect(Object.keys(urlOptions).filter(key => !notes.includes(key))).toEqual([]);
      }
    }
  });

  it('shapes base URLs so the adapter appends the right suffix', async () => {
    const { PROVIDER_TEMPLATES } = await loadPresets();

    for (const template of Object.values(PROVIDER_TEMPLATES)) {
      const entries = [
        { url: template.baseUrl, format: template.format },
        ...(template.baseUrlOptions ?? []),
      ];

      for (const { url, format } of entries) {
        if (format === 'anthropic') {
          // The Anthropic adapter appends /v1/messages itself.
          expect(url.endsWith('/v1'), `${template.id}: ${url} would request /v1/v1/messages`).toBe(false);
        }
        if (format === 'openai') {
          expect(url.endsWith('/messages'), `${template.id}: ${url} is not an OpenAI path`).toBe(false);
        }
      }
    }
  });

  it('never maps one URL to two providers', async () => {
    const { PROVIDER_TEMPLATES } = await loadPresets();
    const owner = new Map<string, string>();

    for (const template of Object.values(PROVIDER_TEMPLATES)) {
      const urls = [template.baseUrl, ...(template.baseUrlOptions ?? []).map(option => option.url)];
      for (const url of urls) {
        const previous = owner.get(url);
        expect(previous ?? template.id, `${url} is claimed by ${previous} and ${template.id}`).toBe(template.id);
        owner.set(url, template.id);
      }
    }
  });
});
