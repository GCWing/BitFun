import React, { useCallback, useMemo } from 'react';
import { FontPreferencePanel } from '@/infrastructure/font-preference';
import { useMouseGlowPreference } from '@/infrastructure/mouse-glow';
import { useTranslation } from 'react-i18next';
import { Select, Switch } from '@/component-library';
import {
  SYSTEM_APPEARANCE_ID,
  useAppearance,
  type AppearanceCatalogEntry,
} from '@/infrastructure/appearance';
import { useLanguageSelector } from '@/infrastructure/i18n';
import type { LocaleId } from '@/infrastructure/i18n/types';
import { AppearancePackageConfigSection } from './AppearancePackageConfigSection';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageSection,
  ConfigPageSectionStack,
  ConfigPageRow,
} from './common';
import './AppearanceConfig.scss';

function AppearanceSelectionSection() {
  const { t } = useTranslation('settings/basics');
  const {
    selectedAppearanceId,
    appearances,
    select,
    initialized,
    status,
  } = useAppearance();
  const { currentLanguage, supportedLocales, selectLanguage, isChanging } = useLanguageSelector();

  const handleAppearanceChange = async (nextAppearanceId: string) => {
    await select(nextAppearanceId);
  };

  const getAppearanceDisplayName = useCallback((appearance: AppearanceCatalogEntry) => {
    const presetId = appearance.id.replace(/^builtin\./, '');
    const i18nKey = `appearance.presets.${presetId}`;
    return appearance.source === 'builtin'
      ? t(`${i18nKey}.name`, { defaultValue: appearance.name })
      : appearance.name;
  }, [t]);

  const getAppearanceDisplayDescription = useCallback((appearance: AppearanceCatalogEntry) => {
    const presetId = appearance.id.replace(/^builtin\./, '');
    const i18nKey = `appearance.presets.${presetId}`;
    return appearance.source === 'builtin'
      ? t(`${i18nKey}.description`, { defaultValue: appearance.description || '' })
      : appearance.description || '';
  }, [t]);

  const appearanceOptions = useMemo(
    () => [
      {
        value: SYSTEM_APPEARANCE_ID,
        label: t('appearance.systemAppearance'),
        description: t('appearance.systemAppearanceDescription'),
        testId: 'appearance-palette-option',
        testAttributes: {
          'data-appearance-id': SYSTEM_APPEARANCE_ID,
        },
      },
      ...appearances.map((appearance) => ({
        value: appearance.id,
        label: getAppearanceDisplayName(appearance),
        description: getAppearanceDisplayDescription(appearance),
        testId: 'appearance-palette-option',
        testAttributes: {
          'data-appearance-id': appearance.id,
        },
      })),
    ],
    [appearances, t, getAppearanceDisplayDescription, getAppearanceDisplayName]
  );

  return (
    <div
      className="appearance-settings"
      data-testid="appearance-settings-section"
      data-bf-component="appearance-config"
      data-bf-part="settings"
    >
      <div
        className="appearance-settings__content"
        data-bf-component="appearance-config"
        data-bf-part="settingsContent"
      >
        <ConfigPageSection title={t('appearance.title')} description={t('appearance.hint')}>
          <ConfigPageRow
            label={t('appearance.language')}
            description={t('appearance.languageRowHint')}
            align="center"
          >
            <div
              className="appearance-settings__language-select"
              data-bf-component="appearance-config"
              data-bf-part="language"
            >
              <Select
                value={currentLanguage}
                onChange={(value) =>
                  selectLanguage(String(Array.isArray(value) ? value[0] ?? '' : value) as LocaleId)
                }
                options={supportedLocales.map((locale) => ({
                  value: locale.id,
                  label: locale.nativeName,
                  testId: 'appearance-language-option',
                  testAttributes: {
                    'data-locale-id': locale.id,
                  },
                }))}
                disabled={isChanging}
                placeholder={t('appearance.language')}
                triggerTestId="appearance-language-select"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={t('appearance.appearances')}
            description={t('appearance.appearanceRowHint')}
            align="center"
          >
            <div
              className="appearance-settings__palette-picker"
              data-bf-component="appearance-config"
              data-bf-part="palettePicker"
            >
              <div
                className="appearance-settings__palette-select"
                data-bf-component="appearance-config"
                data-bf-part="paletteSelect"
              >
                <Select
                  value={selectedAppearanceId}
                  onChange={(value) => handleAppearanceChange(value as string)}
                  disabled={!initialized || status === 'applying'}
                  options={appearanceOptions}
                  triggerTestId="appearance-palette-select"
                  renderOption={(option) => (
                    <div
                      className="appearance-settings__palette-option"
                      data-bf-component="appearance-config"
                      data-bf-part="paletteOption"
                    >
                      <span className="appearance-settings__palette-option-name">{option.label}</span>
                      {option.description && (
                        <span className="appearance-settings__palette-option-desc">{option.description}</span>
                      )}
                    </div>
                  )}
                />
              </div>
            </div>
          </ConfigPageRow>
        </ConfigPageSection>
      </div>
    </div>
  );
}

function AppearanceEffectsSection() {
  const { t } = useTranslation('settings/appearance');
  const { enabled, setEnabled } = useMouseGlowPreference();

  return (
    <ConfigPageSection title={t('effects.title')} description={t('effects.hint')}>
      <ConfigPageRow
        label={t('effects.mouseGlow.label')}
        description={t('effects.mouseGlow.description')}
        align="center"
      >
        <Switch
          checked={enabled}
          onChange={(event) => setEnabled(event.target.checked)}
          aria-label={t('effects.mouseGlow.label')}
          data-testid="appearance-mouse-glow-switch"
          size="small"
        />
      </ConfigPageRow>
    </ConfigPageSection>
  );
}

const AppearanceConfig: React.FC = () => {
  const { t } = useTranslation('settings/appearance');

  return (
    <ConfigPageLayout
      className="bitfun-appearance-config"
      data-bf-component="appearance-config"
      data-bf-part="root"
    >
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent
        className="bitfun-appearance-config__content"
        data-bf-component="appearance-config"
        data-bf-part="content"
      >
        <ConfigPageSectionStack data-testid="appearance-config">
          <AppearanceSelectionSection />
          <AppearancePackageConfigSection />
          <AppearanceEffectsSection />
          <FontPreferencePanel />
        </ConfigPageSectionStack>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AppearanceConfig;
