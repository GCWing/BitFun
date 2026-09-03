import React from 'react';
import AcpAgentsConfig from '@/infrastructure/config/components/AcpAgentsConfig';
import { useSettingsStore } from '../settingsStore';
import type { SettingsPageProps } from '../settingsTypes';

const AcpSettingsPage: React.FC<SettingsPageProps> = ({
  viewId,
  navigationRequestId,
}) => {
  const setActiveView = useSettingsStore((state) => state.setActiveView);
  return (
    <AcpAgentsConfig
      navigationRequestId={navigationRequestId}
      onViewChange={setActiveView}
      settingsDraftEnabled
      viewId={viewId === 'ssh' || viewId === 'json' ? viewId : 'local'}
    />
  );
};

export default AcpSettingsPage;
