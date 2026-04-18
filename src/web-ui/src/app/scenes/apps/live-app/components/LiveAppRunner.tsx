/**
 * LiveAppRunner — sandboxed iframe that runs a compiled Live App.
 * Injects the bridge script (already in compiledHtml from Rust compiler)
 * and handles all postMessage RPC via useLiveAppBridge.
 */
import React, { useRef } from 'react';
import type { LiveApp } from '@/infrastructure/api/service-api/LiveAppAPI';
import { useLiveAppBridge } from '../hooks/useLiveAppBridge';

interface LiveAppRunnerProps {
  app: LiveApp;
}

const LiveAppRunner: React.FC<LiveAppRunnerProps> = ({ app }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  useLiveAppBridge(iframeRef, app);

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

export default LiveAppRunner;
