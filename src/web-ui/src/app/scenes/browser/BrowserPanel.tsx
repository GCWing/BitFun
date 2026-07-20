/**
 * BrowserPanel — embeds a browser into the AuxPane right panel.
 *
 * Uses a Tauri native Webview overlay positioned over the panel's DOM element.
 * The webview is kept attached to the main window and reused across navigations
 * so video/WebRTC surfaces are not repeatedly torn down or reparented.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { AlertTriangle, ChevronLeft, ChevronRight, Globe, RefreshCw, MousePointer2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { IconButton } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useContextStore } from '@/shared/context-system';
import type { WebElementContext } from '@/shared/types/context';
import { createInspectorScript, CANCEL_INSPECTOR_SCRIPT } from './browserInspectorScript';
import { useEmbeddedBrowserWebview } from './useEmbeddedBrowserWebview';
import './BrowserPanel.scss';

const log = createLogger('BrowserPanel');
const DEFAULT_URL = 'https://openbitfun.com/';

interface InspectorElementData {
  tagName: string;
  path: string;
  attributes: Record<string, string>;
  textContent: string;
  outerHTML: string;
}

export interface BrowserPanelProps {
  /** Whether this panel is the active tab in the EditorGroup */
  isActive: boolean;
  /** Optional initial URL (falls back to DEFAULT_URL) */
  initialUrl?: string;
}

const BrowserPanel: React.FC<BrowserPanelProps> = ({ isActive, initialUrl }) => {
  const { t } = useTranslation('common');
  const activeTabId = useSceneStore((s) => s.activeTabId);
  const shouldShowWebview = isActive && activeTabId === 'session';
  const addContext = useContextStore((s) => s.addContext);
  const inspectorUnlistenRef = useRef<(() => void) | null>(null);
  const [isInspectorActive, setIsInspectorActive] = useState(false);

  const browser = useEmbeddedBrowserWebview({
    defaultUrl: DEFAULT_URL,
    initialUrl,
    isVisible: shouldShowWebview,
    labelPrefix: 'embedded-browser-panel-view',
    log,
  });

  const {
    currentUrl,
    error,
    evalInWebview,
    getCurrentUrl,
    getWebviewLabel,
    goBack,
    goForward,
    hasWebview,
    inputValue,
    isLoading,
    isTauri,
    loadUrl,
    reload,
    setInputValue,
    viewportRef,
    webviewLabel,
  } = browser;

  const stopInspector = useCallback(() => {
    if (getWebviewLabel()) {
      void evalInWebview(CANCEL_INSPECTOR_SCRIPT).catch(() => {});
    }
    inspectorUnlistenRef.current?.();
    inspectorUnlistenRef.current = null;
    setIsInspectorActive(false);
  }, [evalInWebview, getWebviewLabel]);

  const loadPanelUrl = useCallback(async (rawUrl: string) => {
    stopInspector();
    await loadUrl(rawUrl);
  }, [loadUrl, stopInspector]);

  useEffect(() => () => {
    stopInspector();
  }, [stopInspector]);

  const handleSubmit = useCallback((event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void loadPanelUrl(inputValue);
  }, [inputValue, loadPanelUrl]);

  const handleInspector = useCallback(async () => {
    if (!isTauri || !hasWebview()) return;

    if (isInspectorActive) {
      stopInspector();
      return;
    }

    const label = getWebviewLabel();
    if (!label) return;

    try {
      const { listen } = await import('@tauri-apps/api/event');

      const eventSelected = `browser-inspector-element-selected-${label}`;
      const eventCancelled = `browser-inspector-cancelled-${label}`;

      const unlistenSelected = await listen<InspectorElementData>(
        eventSelected,
        (event) => {
          const data = event.payload;
          const context: WebElementContext = {
            id: `web-element-${Date.now()}`,
            type: 'web-element',
            timestamp: Date.now(),
            tagName: data.tagName,
            path: data.path,
            attributes: data.attributes,
            textContent: data.textContent,
            outerHTML: data.outerHTML,
            sourceUrl: getCurrentUrl(),
          };

          addContext(context);
          window.dispatchEvent(
            new CustomEvent('insert-context-tag', { detail: { context } }),
          );
        },
      );

      const unlistenCancelled = await listen(
        eventCancelled,
        () => {
          unlistenSelected();
          unlistenCancelled();
          inspectorUnlistenRef.current = null;
          setIsInspectorActive(false);
        },
      );

      inspectorUnlistenRef.current = () => {
        unlistenSelected();
        unlistenCancelled();
      };

      await evalInWebview(createInspectorScript(label));
      setIsInspectorActive(true);
    } catch (inspectorError) {
      log.error('Start inspector failed', inspectorError);
      setIsInspectorActive(false);
    }
  }, [addContext, evalInWebview, getCurrentUrl, getWebviewLabel, hasWebview, isInspectorActive, isTauri, stopInspector]);

  return (
    <div className="browser-panel" data-testid="browser-panel">
      <form className="browser-panel__toolbar" onSubmit={handleSubmit} data-testid="browser-panel-title">
        <IconButton
          type="button"
          variant="ghost"
          size="small"
          onClick={goBack}
          aria-label={t('nav.back')}
          data-testid="browser-back-button"
        >
          <ChevronLeft size={14} />
        </IconButton>
        <IconButton
          type="button"
          variant="ghost"
          size="small"
          onClick={goForward}
          aria-label={t('nav.forward')}
          data-testid="browser-forward-button"
        >
          <ChevronRight size={14} />
        </IconButton>
        <IconButton
          type="button"
          variant="ghost"
          size="small"
          onClick={reload}
          disabled={isLoading}
          aria-label={t('actions.refresh')}
          data-testid="browser-refresh-button"
        >
          <RefreshCw
            size={14}
            className={isLoading ? 'browser-panel__spinning' : undefined}
            data-testid={isLoading ? 'browser-loading-indicator' : undefined}
          />
        </IconButton>
        <div className="browser-panel__address">
          <Globe size={16} />
          <input
            type="text"
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            placeholder={t('browserView.addressPlaceholder', { exampleUrl: 'https://example.com' })}
            spellCheck={false}
            data-testid="browser-url-input"
          />
        </div>
        {isTauri && (
          <IconButton
            type="button"
            variant="ghost"
            size="small"
            onClick={() => void handleInspector()}
            aria-label={isInspectorActive ? t('browserView.stopElementSelection') : t('browserView.startElementSelection')}
            className={isInspectorActive ? 'browser-panel__inspector-btn--active' : undefined}
          >
            <MousePointer2 size={14} />
          </IconButton>
        )}
      </form>

      {error ? (
        <div className="browser-panel__error" data-testid="browser-error-message">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </div>
      ) : null}

      <div className="browser-panel__content" data-testid="browser-page-frame">
        {!isTauri ? (
          <iframe
            className="browser-panel__iframe"
            src={currentUrl}
            title="Embedded Browser Panel"
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-downloads"
          />
        ) : (
          <div
            ref={viewportRef}
            className="browser-panel__webview-host"
            data-webview-label={webviewLabel}
          >
            <div className="browser-panel__webview-placeholder">
              <Globe size={20} />
              <span data-testid="browser-current-url">{currentUrl}</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default BrowserPanel;
