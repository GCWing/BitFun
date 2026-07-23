/**
 * Component preview entry
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import { PreviewApp } from './PreviewApp';
import { I18nProvider } from '@/infrastructure/i18n';
import { WorkspaceProvider } from '@/infrastructure/contexts/WorkspaceProvider';
import { themeService } from '@/infrastructure/theme';
import './preview.css';
import './flowchat-cards-preview.css';

import '../../app/styles/index.scss';

void themeService.initialize();

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <I18nProvider>
      <WorkspaceProvider>
        <PreviewApp />
      </WorkspaceProvider>
    </I18nProvider>
  </React.StrictMode>
);
