import { Combobox, Select } from '@bitfun/ui';
import React, { useCallback, useMemo, useState } from 'react';
import { FontPreferencePanel } from '@/infrastructure/font-preference';
import { useTranslation } from 'react-i18next';
import {
  SYSTEM_APPEARANCE_ID,
  getAppearancePackageValidationError,
  useAppearance,
  type AppearanceCatalogEntry,
} from '@/infrastructure/appearance';
import { useLanguageSelector } from '@/infrastructure/i18n';
import type { LocaleId } from '@/infrastructure/i18n/types';
import { notificationService } from '@/shared/notification-system';
import {
  AppearancePackageConfigSection,
  AppearancePackageFailurePanel,
  type AppearancePackageFailure,
} from './AppearancePackageConfigSection';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageSection,
  ConfigPageSectionStack,
  ConfigPageRow,
} from './common';
import './AppearanceSettingsPage.scss';

function AppearanceSelectionSection() {
  const { t } = useTranslation('settings/application');
  const { t: tAppearance } = useTranslation('settings/appearance');
  const [packageActivationFailure, setPackageActivationFailure] = useState<
    AppearancePackageFailure | null
  >(null);
  const {
    selectedAppearanceId,
    appearances,
    select,
    activate,
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
  const packageAppearances = useMemo(
    () => appearances.filter(appearance => appearance.source === 'imported'),
    [appearances],
  );
  const selectedPackage = packageAppearances.find(
    appearance => appearance.id === selectedAppearanceId,
  );
  const selectedPackageId = selectedPackage?.id ?? SYSTEM_APPEARANCE_ID;
  const packageOptions = useMemo(() => [
    {
      value: SYSTEM_APPEARANCE_ID,
      label: tAppearance('package.nativeName'),
      description: tAppearance('package.nativeDescription'),
      testId: 'appearance-package-option',
      testAttributes: { 'data-appearance-id': SYSTEM_APPEARANCE_ID },
    },
    ...packageAppearances.map(appearance => ({
      value: appearance.id,
      label: appearance.name,
      description: `${appearance.author || tAppearance('package.unknownAuthor')} · v${appearance.version}`,
      testId: 'appearance-package-option',
      testAttributes: { 'data-appearance-id': appearance.id },
    })),
  ], [packageAppearances, tAppearance]);

  const handlePackageChange = async (value: string | number | (string | number)[]) => {
    if (Array.isArray(value)) return;
    try {
      await activate(String(value));
      setPackageActivationFailure(null);
    } catch (error) {
      const validationError = getAppearancePackageValidationError(error);
      setPackageActivationFailure({
        operation: 'activate',
        ...(validationError
          ? { validationError }
          : { message: error instanceof Error ? error.message : String(error) }),
      });
      notificationService.error(
        validationError
          ? tAppearance('package.diagnostics.activateSummary', { count: validationError.issues.length })
          : tAppearance('package.activateFailed'),
        { duration: 5000 },
      );
    }
  };

  return (
    <div
      className="appearance-settings"
      data-testid="appearance-settings-section"
      data-bf-component="appearance-settings"
      data-bf-part="settings"
    >
      <div
        className="appearance-settings__content"
        data-bf-component="appearance-settings"
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
              data-bf-component="appearance-settings"
              data-bf-part="language"
            >
              <Select
                size="sm"
                value={currentLanguage}
                onValueChange={(value) =>
                  selectLanguage(String(value) as LocaleId)
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
                data-testid="appearance-language-select"
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
              data-bf-component="appearance-settings"
              data-bf-part="palettePicker"
            >
              <div
                className="appearance-settings__palette-select"
                data-bf-component="appearance-settings"
                data-bf-part="paletteSelect"
              >
                <Combobox
                  size="sm"
                  value={selectedAppearanceId}
                  onValueChange={(value) => handleAppearanceChange(value as string)}
                  disabled={!initialized || status === 'applying'}
                  options={appearanceOptions}
                  data-testid="appearance-palette-select"
                />
              </div>
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            className="appearance-settings__package-row"
            label={tAppearance('package.title')}
            description={tAppearance('package.description')}
            align="center"
          >
            <div
              className="appearance-settings__package-select"
              data-bf-component="appearance-settings"
              data-bf-part="packageSelect"
            >
              <Select
                size="sm"
                value={selectedPackageId}
                options={packageOptions}
                onValueChange={handlePackageChange}
                disabled={!initialized || status === 'applying'}
                data-testid="appearance-package-select"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>
        {packageActivationFailure && (
          <AppearancePackageFailurePanel
            failure={packageActivationFailure}
            onDismiss={() => setPackageActivationFailure(null)}
          />
        )}
        <AppearancePackageConfigSection />
      </div>
    </div>
  );
}

const AppearanceSettingsPage: React.FC = () => {
  const { t } = useTranslation('settings/appearance');

  return (
    <ConfigPageLayout
      className="bitfun-appearance-settings"
      data-bf-component="appearance-settings"
      data-bf-part="root"
    >
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent
        className="bitfun-appearance-settings__content"
        data-bf-component="appearance-settings"
        data-bf-part="content"
      >
        <ConfigPageSectionStack data-testid="appearance-settings">
          <AppearanceSelectionSection />
          <FontPreferencePanel />
        </ConfigPageSectionStack>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AppearanceSettingsPage;
