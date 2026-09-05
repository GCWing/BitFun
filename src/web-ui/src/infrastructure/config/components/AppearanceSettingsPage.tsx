import { Select } from '@openbitfun/ui';
import React from 'react';
import { FontPreferencePanel } from '@/infrastructure/font-preference';
import { useTranslation } from 'react-i18next';
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
import './AppearanceSettingsPage.scss';

function AppearanceSelectionSection() {
  const { t } = useTranslation('settings/application');
  const { currentLanguage, supportedLocales, selectLanguage, isChanging } = useLanguageSelector();

  return (
    <div
      className="appearance-settings"
      data-testid="appearance-settings-section"
      data-openbitfun-component="appearance-settings"
      data-openbitfun-part="settings"
    >
      <div
        className="appearance-settings__content"
        data-openbitfun-component="appearance-settings"
        data-openbitfun-part="settingsContent"
      >
        <ConfigPageSection
          title={t('appearance.interfaceTitle')}
          description={t('appearance.interfaceHint')}
        >
          <ConfigPageRow
            label={t('appearance.language')}
            align="center"
          >
            <div
              className="appearance-settings__language-select"
              data-openbitfun-component="appearance-settings"
              data-openbitfun-part="language"
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
        </ConfigPageSection>
        <AppearancePackageConfigSection />
      </div>
    </div>
  );
}

const AppearanceSettingsPage: React.FC = () => {
  const { t } = useTranslation('settings/appearance');

  return (
    <ConfigPageLayout
      className="openbitfun-appearance-settings"
      data-openbitfun-component="appearance-settings"
      data-openbitfun-part="root"
    >
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent
        className="openbitfun-appearance-settings__content"
        data-openbitfun-component="appearance-settings"
        data-openbitfun-part="content"
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
