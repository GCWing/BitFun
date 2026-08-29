import { describe, expect, it, vi } from 'vitest';
import {
  isLegacyEcosystemCompatibilityDestination,
  resolveSettingsDestination,
} from './settingsDestination';
import {
  SETTINGS_CATEGORIES,
  SETTINGS_PAGE_MANIFESTS,
} from './settingsRegistry';

vi.mock('@/infrastructure/i18n/core/I18nService', () => ({
  i18nService: { loadNamespace: vi.fn(async () => undefined) },
}));

describe('settings information architecture', () => {
  it('uses five ownership categories and sixteen canonical pages', () => {
    expect(SETTINGS_CATEGORIES.map((category) => category.id)).toEqual([
      'application',
      'ai',
      'workspace',
      'tools',
      'data',
    ]);
    expect(SETTINGS_PAGE_MANIFESTS).toHaveLength(16);
    expect(new Set(SETTINGS_PAGE_MANIFESTS.map((page) => page.id)).size).toBe(16);
  });

  it('keeps memory with AI, pet with application, and review inside execution', () => {
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'ai.memory')?.categoryId).toBe('ai');
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.pet')?.categoryId).toBe('application');
    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id.includes('review'))).toBe(false);
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.execution')?.searchPhrases)
      .toContainEqual({ namespace: 'settings/review-capacity', key: 'capacity.title' });
  });

  it('keeps execution, permissions, desktop control, and browser control on one page', () => {
    const execution = SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.execution');

    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id === 'tools.device-control')).toBe(false);
    expect(execution?.views).toBeUndefined();
    expect(execution?.searchPhrases).toEqual(expect.arrayContaining([
      { namespace: 'settings/runtime', key: 'permissionPolicy.sectionTitle' },
      { namespace: 'settings/runtime', key: 'computerUse.sectionTitle' },
      { namespace: 'settings/runtime', key: 'browserControl.sectionTitle' },
    ]));
    expect(resolveSettingsDestination('tools.device-control')).toEqual({ pageId: 'tools.execution' });
  });

  it('keeps Assistant ownership outside Settings and model preferences with network proxy', () => {
    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id.includes('assistant'))).toBe(false);
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'ai.models')?.searchPhrases)
      .toEqual(expect.arrayContaining([
        { namespace: 'settings/default-model', key: 'sections.defaults' },
        { namespace: 'settings/default-model', key: 'sections.proxy' },
      ]));
  });

  it('keeps external-source governance outside Settings while retaining MCP and ACP owners', () => {
    expect(SETTINGS_CATEGORIES.find((category) => category.id === 'tools')?.pages.map((page) => page.id))
      .toEqual([
        'tools.execution',
        'tools.automation',
        'tools.mcp',
        'tools.acp',
      ]);
    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id === 'tools.integrations')).toBe(false);
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.mcp')?.views).toBeUndefined();
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.acp')?.views).toBeUndefined();
  });

  it('keeps automation as a page with deep-linkable views', () => {
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.automation')?.views?.map((view) => view.id))
      .toEqual(['quick-actions', 'hooks']);
  });

  it('keeps voice input and shortcuts as anchors in one input settings page', () => {
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.input')?.views?.map((view) => view.id))
      .toEqual(['voice', 'shortcuts']);
  });

  it('keeps terminal and editor as anchors in one development page, terminal first', () => {
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.development')?.views?.map((view) => view.id))
      .toEqual(['terminal', 'editor']);
    expect(resolveSettingsDestination('terminal')).toEqual({
      pageId: 'application.development',
      viewId: 'terminal',
    });
    expect(resolveSettingsDestination('editor')).toEqual({
      pageId: 'application.development',
      viewId: 'editor',
    });
  });

  it('keeps appearance packages discoverable inside Appearance', () => {
    const appearance = SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.appearance');

    expect(appearance?.searchPhrases).toEqual(expect.arrayContaining([
      { namespace: 'settings/appearance', key: 'package.title' },
    ]));
    expect(SETTINGS_PAGE_MANIFESTS.some((page) => /motion|package/.test(page.id))).toBe(false);
  });

  it('exposes usage and archived sessions as separate second-level pages', () => {
    const dataPages = SETTINGS_CATEGORIES.find((category) => category.id === 'data')?.pages;

    expect(dataPages?.map((page) => page.id)).toEqual([
      'data.usage',
      'data.archived',
      'data.diagnostics',
    ]);
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'data.usage')?.views).toBeUndefined();
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'data.archived')?.views).toBeUndefined();
  });

  it('contains old links at the upgrade boundary and emits canonical destinations', () => {
    expect(resolveSettingsDestination('hooks')).toEqual({
      pageId: 'tools.automation',
      viewId: 'hooks',
    });
    expect(isLegacyEcosystemCompatibilityDestination('external-sources')).toBe(true);
    expect(isLegacyEcosystemCompatibilityDestination('tools.integrations')).toBe(true);
    expect(isLegacyEcosystemCompatibilityDestination({ pageId: 'tools.integrations' })).toBe(true);
    expect(resolveSettingsDestination('mcp-tools')).toEqual({ pageId: 'tools.mcp' });
    expect(resolveSettingsDestination('acp-agents')).toEqual({ pageId: 'tools.acp' });
    expect(resolveSettingsDestination('ai.models')).toEqual({ pageId: 'ai.models' });
    expect(resolveSettingsDestination('usage-statistics')).toEqual({ pageId: 'data.usage' });
    expect(resolveSettingsDestination('archived-sessions')).toEqual({ pageId: 'data.archived' });
    expect(resolveSettingsDestination('data.history')).toEqual({ pageId: 'data.usage' });
  });
});
