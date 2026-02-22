/**
 * useCurrentSettingsTabTitle — returns translated active settings tab label.
 */

import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../scenes/settings/settingsStore';
import { SETTINGS_CATEGORIES } from '../scenes/settings/settingsConfig';

export function useCurrentSettingsTabTitle(): string {
  const { t } = useTranslation('settings');
  const activeTab = useSettingsStore((state) => state.activeTab);

  return useMemo(() => {
    for (const category of SETTINGS_CATEGORIES) {
      const tab = category.tabs.find((item) => item.id === activeTab);
      if (tab) {
        return t(tab.labelKey, activeTab);
      }
    }
    return '';
  }, [activeTab, t]);
}

