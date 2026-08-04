import React from 'react';
import App from './App';
import { WorkspaceProvider } from '@/infrastructure/contexts/WorkspaceProvider';
import { I18nProvider } from '@/infrastructure/i18n/providers/I18nProvider';
import { PeerDeviceProvider } from '@/infrastructure/peer-device/PeerDeviceContext';
import { PeerDirectoryPickerHost } from '@/infrastructure/peer-device/PeerDirectoryPickerHost';
import { PeerHostInvokeBridge } from '@/infrastructure/peer-device/PeerHostInvokeBridge';

const BusinessApplication: React.FC = () => (
  <I18nProvider>
    <WorkspaceProvider>
      <PeerDeviceProvider>
        <PeerHostInvokeBridge />
        <PeerDirectoryPickerHost />
        <App />
      </PeerDeviceProvider>
    </WorkspaceProvider>
  </I18nProvider>
);

export default BusinessApplication;
