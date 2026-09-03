import { describe, expect, it, vi } from 'vitest';
import {
  isLegacyEcosystemCompatibilityDestination,
  resolveSettingsDestination,
} from './settingsDestination';
import {
  SETTINGS_CATEGORIES,
  SETTINGS_PAGE_MANIFESTS,
} from './settingsRegistry';
import { useSettingsStore } from './settingsStore';
import type { SettingsDestination } from './settingsTypes';

vi.mock('@/infrastructure/i18n/core/I18nService', () => ({
  i18nService: { loadNamespace: vi.fn(async () => undefined) },
}));

describe('settings information architecture', () => {
  it('uses five ownership categories and twenty canonical pages', () => {
    expect(SETTINGS_CATEGORIES.map((category) => category.id)).toEqual([
      'application',
      'ai',
      'workspace',
      'tools',
      'data',
    ]);
    expect(SETTINGS_PAGE_MANIFESTS).toHaveLength(20);
    expect(new Set(SETTINGS_PAGE_MANIFESTS.map((page) => page.id)).size).toBe(20);
  });

  it('keeps memory with AI, pet with application, and review inside execution', () => {
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'ai.memory')?.categoryId).toBe('ai');
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.pet')?.categoryId).toBe('application');
    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id.includes('review'))).toBe(false);
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.execution')?.searchPhrases)
      .toContainEqual({ namespace: 'settings/review-capacity', key: 'capacity.title' });
  });

  it('keeps execution and permissions on one page, with browser and desktop control in one page', () => {
    const execution = SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.execution');
    const control = SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.desktop-control');

    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id === 'tools.device-control')).toBe(false);
    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id === 'tools.browser-control')).toBe(false);
    expect(execution?.views).toBeUndefined();
    expect(execution?.searchPhrases)
      .toContainEqual({ namespace: 'settings/runtime', key: 'permissionPolicy.sectionTitle' });
    expect(execution?.searchPhrases)
      .toContainEqual({ namespace: 'settings/runtime', key: 'toolExecution.sectionTitle' });
    expect(control?.labelKey).toBe('navigation.pages.browserDesktopControl.label');
    expect(control?.searchPhrases)
      .toContainEqual({ namespace: 'settings/runtime', key: 'computerUse.sectionTitle' });
    expect(control?.searchPhrases)
      .toContainEqual({ namespace: 'settings/runtime', key: 'browserControl.sectionTitle' });
    expect(resolveSettingsDestination('tools.device-control')).toEqual({ pageId: 'tools.desktop-control' });
    expect(resolveSettingsDestination('tools.browser-control')).toEqual({ pageId: 'tools.desktop-control' });
    expect(resolveSettingsDestination('review')).toEqual({ pageId: 'tools.execution' });
  });

  it('normalizes the retired browser-control page at the store boundary', () => {
    useSettingsStore.getState().openDestination({
      pageId: 'tools.browser-control',
    } as unknown as SettingsDestination);

    expect(useSettingsStore.getState().activePageId).toBe('tools.desktop-control');
    expect(useSettingsStore.getState().pageTransitionTarget).toBe('tools.desktop-control');
  });

  it('keeps Assistant ownership outside Settings and model preferences with network proxy', () => {
    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id.includes('assistant'))).toBe(false);
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'ai.models')?.searchPhrases)
      .toEqual(expect.arrayContaining([
        { namespace: 'settings/default-model', key: 'sections.defaults' },
        { namespace: 'settings/default-model', key: 'sections.proxy' },
      ]));
  });

  it('keeps external-source governance outside Settings while retaining WebSearch, MCP, and ACP owners', () => {
    expect(SETTINGS_CATEGORIES.find((category) => category.id === 'tools')?.pages.map((page) => page.id))
      .toEqual([
        'tools.execution',
        'tools.desktop-control',
        'tools.automation',
        'tools.webSearch',
        'tools.mcp',
        'tools.acp',
      ]);
    expect(SETTINGS_PAGE_MANIFESTS.some((page) => page.id === 'tools.integrations')).toBe(false);
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.mcp')?.views).toBeUndefined();
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.acp')?.views?.map((view) => view.id))
      .toEqual(['local', 'ssh', 'json']);
  });

  it('keeps automation as a page with deep-linkable views', () => {
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'tools.automation')?.views?.map((view) => view.id))
      .toEqual(['quick-actions', 'hooks']);
  });

  it('exposes voice and shortcuts as independent application pages', () => {
    const applicationPages = SETTINGS_CATEGORIES.find((category) => category.id === 'application')?.pages;

    expect(applicationPages?.map((page) => page.id)).toEqual([
      'application.general',
      'application.appearance',
      'application.pet',
      'application.voice',
      'application.shortcuts',
      'application.terminal',
      'application.editor',
    ]);
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.voice')?.views)
      .toBeUndefined();
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.shortcuts')?.views)
      .toBeUndefined();
    expect(resolveSettingsDestination('application.input')).toEqual({ pageId: 'application.voice' });
    expect(resolveSettingsDestination('shortcuts')).toEqual({ pageId: 'application.shortcuts' });
  });

  it('exposes terminal and editor as independent application pages', () => {
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.terminal')?.views)
      .toBeUndefined();
    expect(SETTINGS_PAGE_MANIFESTS.find((page) => page.id === 'application.editor')?.views)
      .toBeUndefined();
    expect(resolveSettingsDestination('application.development')).toEqual({
      pageId: 'application.terminal',
    });
    expect(resolveSettingsDestination('terminal')).toEqual({ pageId: 'application.terminal' });
    expect(resolveSettingsDestination('editor')).toEqual({ pageId: 'application.editor' });
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
