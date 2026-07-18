/**
 * MiniAppRunner — sandboxed iframe that runs a compiled MiniApp.
 * Injects the bridge script (already in compiledHtml from Rust compiler)
 * and handles all postMessage RPC via useMiniAppBridge.
 */
import React, { useCallback, useEffect, useRef } from 'react';
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

  const writeCompiledHtml = useCallback(() => {
    const iframe = iframeRef.current;
    const html = app.compiled_html?.trim();
    if (!iframe || !html) {
      return false;
    }

    const doc = iframe.contentDocument;
    if (!doc) {
      return false;
    }

    doc.open();
    doc.write(html);
    doc.close();
    return true;
  }, [app.compiled_html]);

  useEffect(() => {
    const iframe = iframeRef.current;
    if (!iframe) {
      return undefined;
    }

    const html = app.compiled_html?.trim();
    if (!html) {
      return undefined;
    }

    if (writeCompiledHtml()) {
      return undefined;
    }

    const handleLoad = () => {
      writeCompiledHtml();
    };

    iframe.addEventListener('load', handleLoad);
    if (iframe.src !== 'about:blank') {
      iframe.src = 'about:blank';
    }

    return () => {
      iframe.removeEventListener('load', handleLoad);
    };
  }, [app.id, app.compiled_html, writeCompiledHtml]);

  return (
    <iframe
      ref={iframeRef}
      src="about:blank"
      data-app-id={app.id}
      data-run-scope={runScope?.kind ?? 'active'}
      sandbox="allow-scripts allow-same-origin allow-forms allow-modals allow-popups allow-downloads"
      style={{ flex: '1 1 auto', width: '100%', minHeight: 0, border: 'none', display: 'block' }}
      title={app.name}
    />
  );
};

export default MiniAppRunner;
