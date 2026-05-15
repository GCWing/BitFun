/**
 * MiniAppRunner — sandboxed iframe that runs a compiled MiniApp.
 * Injects the bridge script (already in compiledHtml from Rust compiler)
 * and handles all postMessage RPC via useMiniAppBridge.
 */
import React, { useRef } from 'react';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useMiniAppBridge } from '../hooks/useMiniAppBridge';
import type { MiniAppRunScope } from '../customization/miniAppCustomizationTypes';

interface MiniAppRunnerProps {
  app: MiniApp;
  runScope?: MiniAppRunScope;
}

const MiniAppRunner: React.FC<MiniAppRunnerProps> = ({ app, runScope }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  useMiniAppBridge(iframeRef, app, runScope ?? { kind: 'active', appId: app.id });

  return (
    <iframe
      ref={iframeRef}
      srcDoc={app.compiled_html}
      data-app-id={app.id}
      data-run-scope={runScope?.kind ?? 'active'}
      sandbox="allow-scripts allow-forms allow-modals allow-popups allow-downloads"
      style={{ width: '100%', height: '100%', border: 'none', display: 'block' }}
      title={app.name}
    />
  );
};

export default MiniAppRunner;
