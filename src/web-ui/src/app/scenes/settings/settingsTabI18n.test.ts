import { describe, expect, it, vi } from 'vitest';

vi.mock('../../../infrastructure/config/components/BasicsConfig', () => ({
  default: () => null,
}));

import { i18nService } from '@/infrastructure/i18n/core/I18nService';
import { SETTINGS_TAB_I18N_NAMESPACES, preloadSettingsShellI18n } from './settingsTabI18n';
import { isSettingsTabContentReady, preloadSettingsTabContent } from './settingsContentRegistry';
import { SETTINGS_CATEGORIES } from './settingsConfig';

/**
 * A namespace that has not resolved yet makes `t(key)` echo the key back, which is
 * the raw-key frame a panel paints when it mounts ahead of its translations.
 */
function resolveKey(namespace: string, key: string): string {
  return String(i18nService.getI18nInstance().getFixedT(null, namespace)(key));
}

describe('settings tab i18n preloading', () => {
  it('covers every nav tab', () => {
    for (const category of SETTINGS_CATEGORIES) {
      for (const tab of category.tabs) {
        expect(SETTINGS_TAB_I18N_NAMESPACES[tab.id]?.length ?? 0).toBeGreaterThan(0);
      }
    }
  });

  it('resolves a tab namespace before the panel can render', async () => {
    expect(resolveKey('settings/basics', 'title')).toBe('title');

    await preloadSettingsTabContent('basics');

    expect(isSettingsTabContentReady('basics')).toBe(true);
    expect(i18nService.getI18nInstance().hasLoadedNamespace('settings/basics')).toBe(true);
    expect(resolveKey('settings/basics', 'title')).not.toBe('title');
  });

  it('resolves the shell namespace behind the nav labels', async () => {
    await preloadSettingsShellI18n();

    expect(resolveKey('settings', 'configCenter.tabs.basics')).not.toBe(
      'configCenter.tabs.basics'
    );
  });
});
