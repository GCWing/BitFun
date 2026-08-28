/**
 * Component preview entry
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import { PreviewApp } from './PreviewApp';
import { I18nProvider } from '@/infrastructure/i18n';
import { WorkspaceProvider } from '@/infrastructure/contexts/WorkspaceProvider';
import {
  appearanceService,
  getSystemAppearanceId,
} from '@/infrastructure/appearance';
import './preview.css';

import '../../app/styles/index.scss';

async function renderPreview(): Promise<void> {
  // The standalone component preview has no Desktop/WebSocket config host.
  // Seed a built-in appearance so initialization stays local and CSS tokens
  // exist before React paints the preview surface.
  const appearanceId = getSystemAppearanceId();
  globalThis.__BITFUN_BOOTSTRAP_APPEARANCE_ID__ = appearanceId;
  globalThis.__BITFUN_BOOTSTRAP_APPEARANCE_SELECTION__ = appearanceId;

  await appearanceService.initialize();

  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>
      <I18nProvider>
        <WorkspaceProvider>
          <PreviewApp />
        </WorkspaceProvider>
      </I18nProvider>
    </React.StrictMode>
  );
}

void renderPreview();
