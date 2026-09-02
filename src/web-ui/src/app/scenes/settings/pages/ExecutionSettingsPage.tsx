import React, { lazy, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsViewPage } from '../components/SettingsViewPage';
import type { SettingsPageProps } from '../settingsTypes';

const ExecutionCommonSettingsPage = lazy(() => (
  import('@/infrastructure/config/components/RuntimeSettingsPages').then((module) => ({
    default: module.ExecutionCommonSettingsPage,
  }))
));

const ExecutionAdvancedSettingsPage = lazy(() => (
  import('@/infrastructure/config/components/RuntimeSettingsPages').then((module) => ({
    default: module.ExecutionAdvancedSettingsPage,
  }))
));

const ExecutionSettingsPage: React.FC<SettingsPageProps> = (props) => {
  const { t } = useTranslation('settings');
  const views = useMemo(() => [
    {
      id: 'common' as const,
      label: t('navigation.views.common'),
      content: <ExecutionCommonSettingsPage />,
    },
    {
      id: 'advanced' as const,
      label: t('navigation.views.advanced'),
      content: <ExecutionAdvancedSettingsPage />,
    },
  ], [t]);

  return (
    <SettingsViewPage
      {...props}
      defaultViewId="common"
      views={views}
    />
  );
};

export default ExecutionSettingsPage;
