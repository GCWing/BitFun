/**
 * MiniAppRunner — sandboxed iframe that runs a compiled MiniApp.
 * Injects the bridge script (already in compiledHtml from Rust compiler)
 * and handles all postMessage RPC via useMiniAppBridge.
 */
import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { MiniApp } from '@/infrastructure/api/service-api/MiniAppAPI';
import { useMiniAppBridge } from '../hooks/useMiniAppBridge';
import type { MiniAppRunScope } from '../customization/miniAppCustomizationTypes';

interface MiniAppRunnerProps {
  app: MiniApp;
  runScope?: MiniAppRunScope;
  strictRuntime?: boolean;
}

const MiniAppRunner: React.FC<MiniAppRunnerProps> = ({ app, runScope, strictRuntime = false }) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const bridgeReady = useMiniAppBridge(
    iframeRef,
    app,
    runScope ?? { kind: 'active', appId: app.id },
    strictRuntime,
  );
  const [strictDocumentUrl, setStrictDocumentUrl] = useState<string>();

  const writeCompiledHtml = useCallback(() => {
    const iframe = iframeRef.current;
    const html = app.compiled_html?.trim();
    if (strictRuntime || !iframe || !html) {
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
  }, [app.compiled_html, strictRuntime]);

  useEffect(() => {
    const iframe = iframeRef.current;
    if (strictRuntime || !iframe) {
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
  }, [app.id, app.compiled_html, strictRuntime, writeCompiledHtml]);

  useLayoutEffect(() => {
    const html = app.compiled_html?.trim();
    if (!strictRuntime || !bridgeReady || !html) {
      setStrictDocumentUrl(undefined);
      return undefined;
    }

    // Tauri's WKWebView has rendered srcdoc as a blank document in production.
    // A sandboxed blob navigation is reliable while retaining the strict
    // runtime's unique opaque origin because allow-same-origin is absent.
    const documentUrl = URL.createObjectURL(
      new Blob([html], { type: 'text/html;charset=utf-8' }),
    );
    setStrictDocumentUrl(documentUrl);
    return () => {
      URL.revokeObjectURL(documentUrl);
    };
  }, [app.id, app.compiled_html, bridgeReady, strictRuntime]);

  if (strictRuntime) {
    return (
      <iframe
        ref={iframeRef}
        src={strictDocumentUrl ?? 'about:blank'}
        data-app-id={app.id}
        data-run-scope={runScope?.kind ?? 'active'}
        data-runtime-profile="market-strict"
        sandbox="allow-scripts allow-downloads"
        referrerPolicy="no-referrer"
        style={{ flex: '1 1 auto', width: '100%', minHeight: 0, border: 'none', display: 'block' }}
        title={app.name}
      />
    );
  }

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
