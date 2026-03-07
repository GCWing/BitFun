/**
 * MiniAppRunner — sandboxed iframe that runs a compiled MiniApp.
 * Injects the bridge script (already in compiledHtml from Rust compiler)
 * and handles all postMessage RPC via useMiniAppBridge.
 */
import React, { useRef } from 'react';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useMiniAppBridge } from '../hooks/useMiniAppBridge';

interface MiniAppRunnerProps {
  app: MiniApp;
}

const MiniAppRunner: React.FC<MiniAppRunnerProps> = ({ app }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  useMiniAppBridge(iframeRef, app);

  return (
    <iframe
      ref={iframeRef}
      srcDoc={app.compiled_html}
      data-app-id={app.id}
      sandbox="allow-scripts allow-forms allow-modals allow-popups allow-downloads"
      style={{ width: '100%', height: '100%', border: 'none', display: 'block' }}
      title={app.name}
    />
  );
};

export default MiniAppRunner;
