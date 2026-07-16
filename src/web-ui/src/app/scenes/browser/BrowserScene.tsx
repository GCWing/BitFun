import React, { useCallback } from 'react';
import { AlertTriangle, ChevronLeft, ChevronRight, Globe, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { IconButton } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useEmbeddedBrowserWebview } from './useEmbeddedBrowserWebview';
import './BrowserScene.scss';

const log = createLogger('BrowserScene');
const DEFAULT_URL = 'https://openbitfun.com/';

const BrowserScene: React.FC = () => {
  const { t } = useTranslation('common');
  const activeTabId = useSceneStore((state) => state.activeTabId);
  const isActive = activeTabId === 'browser';
  const browser = useEmbeddedBrowserWebview({
    defaultUrl: DEFAULT_URL,
    isVisible: isActive,
    labelPrefix: 'embedded-browser-view',
    log,
  });

  const handleSubmit = useCallback((event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void browser.loadUrl(browser.inputValue);
  }, [browser]);

  return (
    <div className="browser-scene" data-testid="browser-panel">
      <form className="browser-scene__toolbar" onSubmit={handleSubmit} data-testid="browser-panel-title">
        <IconButton
          type="button"
          variant="ghost"
          size="small"
          onClick={browser.goBack}
          aria-label={t('nav.back')}
          data-testid="browser-back-button"
        >
          <ChevronLeft size={14} />
        </IconButton>
        <IconButton
          type="button"
          variant="ghost"
          size="small"
          onClick={browser.goForward}
          aria-label={t('nav.forward')}
          data-testid="browser-forward-button"
        >
          <ChevronRight size={14} />
        </IconButton>
        <IconButton
          type="button"
          variant="ghost"
          size="small"
          onClick={browser.reload}
          disabled={browser.isLoading}
          aria-label={t('actions.refresh')}
          data-testid="browser-refresh-button"
        >
          <RefreshCw
            size={14}
            className={browser.isLoading ? 'browser-scene__spinning' : undefined}
            data-testid={browser.isLoading ? 'browser-loading-indicator' : undefined}
          />
        </IconButton>
        <div className="browser-scene__address">
          <Globe size={16} />
          <input
            type="text"
            value={browser.inputValue}
            onChange={(event) => browser.setInputValue(event.target.value)}
            placeholder={t('browserView.addressPlaceholder', { exampleUrl: 'https://example.com' })}
            spellCheck={false}
            data-testid="browser-url-input"
          />
        </div>
      </form>

      {browser.error ? (
        <div className="browser-scene__error" data-testid="browser-error-message">
          <AlertTriangle size={16} />
          <span>{browser.error}</span>
        </div>
      ) : null}

      <div className="browser-scene__content" data-testid="browser-page-frame">
        {!browser.isTauri ? (
          <iframe
            className="browser-scene__iframe"
            src={browser.currentUrl}
            title="Embedded Browser"
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-downloads"
          />
        ) : (
          <div
            ref={browser.viewportRef}
            className="browser-scene__webview-host"
            data-webview-label={browser.webviewLabel}
          >
            <div className="browser-scene__webview-placeholder">
              <Globe size={20} />
              <span data-testid="browser-current-url">{browser.currentUrl}</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default BrowserScene;
