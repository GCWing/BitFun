import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../scenes/settings/settingsStore';
import { getSettingsPageManifest } from '../scenes/settings/settingsRegistry';

export function useCurrentSettingsPageTitle(): string {
  const { t } = useTranslation('settings');
  const activePageId = useSettingsStore((state) => state.activePageId);

  return useMemo(
    () => t(getSettingsPageManifest(activePageId).labelKey, activePageId),
    [activePageId, t],
  );
}
