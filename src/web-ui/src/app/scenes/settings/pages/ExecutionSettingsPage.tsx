import React from 'react';
import {
  ExecutionSettingsPage as RuntimeExecutionSettingsPage,
} from '@/infrastructure/config/components/RuntimeSettingsPages';
import type { SettingsPageProps } from '../settingsTypes';

const ExecutionSettingsPage: React.FC<SettingsPageProps> = () => (
  <RuntimeExecutionSettingsPage />
);

export default ExecutionSettingsPage;
