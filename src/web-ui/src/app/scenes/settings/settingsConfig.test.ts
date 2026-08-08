import { describe, expect, it } from 'vitest';
import { normalizeSettingsTarget, SETTINGS_CATEGORIES } from './settingsConfig';
import { SETTINGS_TAB_SEARCH_CONTENT } from './settingsTabSearchContent';

describe('settings deep-link targets', () => {
  it('routes the legacy Hooks target into External AI applications with focus intent', () => {
    expect(normalizeSettingsTarget('hooks')).toEqual({
      tab: 'external-sources',
      focus: 'hooks',
    });
  });

  it('keeps ordinary settings targets free of content focus', () => {
    expect(normalizeSettingsTarget('external-sources')).toEqual({
      tab: 'external-sources',
    });
  });

  it('keeps Hook management discoverable through the owning settings entry', () => {
    const externalSources = SETTINGS_CATEGORIES
      .flatMap((category) => category.tabs)
      .find((tab) => tab.id === 'external-sources');

    expect(externalSources?.keywords).toEqual(expect.arrayContaining(['hook', 'hooks']));
    expect(SETTINGS_TAB_SEARCH_CONTENT['external-sources']).toEqual(
      expect.arrayContaining([
        { ns: 'settings/hooks', key: 'title' },
        { ns: 'settings/hooks', key: 'activation.title' },
      ]),
    );
  });
});
