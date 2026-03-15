/**
 * BrowserPanel — embeds a browser into the AuxPane right panel.
 *
 * Uses a Tauri native Webview overlay positioned over the panel's DOM element.
 * When the panel is not active (tab switch / scene switch / AuxPane collapse),
 * the webview is reparented to a hidden holder window to preserve page state.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, Globe, RefreshCw, MousePointer2 } from 'lucide-react';
import { IconButton } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { useSceneStore } from '@/app/stores/sceneStore';
import { useContextStore } from '@/shared/context-system';
import type { WebElementContext } from '@/shared/types/context';
import { createInspectorScript, CANCEL_INSPECTOR_SCRIPT } from './browserInspectorScript';
import './BrowserPanel.scss';

const log = createLogger('BrowserPanel');
const DEFAULT_URL = 'https://openbitfun.com/';
const PANEL_HOLDER_WINDOW_LABEL = 'embedded-browser-panel-holder';

function isTauriEnvironment(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

type BrowserWebviewHandle = {
  close: () => Promise<void>;
  hide: () => Promise<void>;
  label: string;
  once: (event: string, handler: (event?: unknown) => void) => Promise<() => void>;
  reparent: (window: string | unknown) => Promise<void>;
  setFocus: () => Promise<void>;
  setPosition: (position: unknown) => Promise<void>;
  setSize: (size: unknown) => Promise<void>;
  show: () => Promise<void>;
};

async function evalWebview(label: string, script: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('browser_webview_eval', { request: { label, script } });
}

type BrowserHolderWindowHandle = {
  close: () => Promise<void>;
  hide: () => Promise<void>;
  once: (event: string, handler: (event?: unknown) => void) => Promise<() => void>;
};

function formatUnknownError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  if (error && typeof error === 'object') {
    const record = error as Record<string, unknown>;
    const payload = 'payload' in record ? record.payload : undefined;
    const message =
      (typeof record.message === 'string' && record.message) ||
      (payload && typeof payload === 'object' && typeof (payload as Record<string, unknown>).message === 'string'
        ? String((payload as Record<string, unknown>).message)
        : null);
    if (message) return message;
    try { return JSON.stringify(error); } catch { return String(error); }
  }
  return String(error);
}

function isWebviewNotFoundError(error: unknown): boolean {
  return formatUnknownError(error).toLowerCase().includes('webview not found');
}

async function waitForWebviewCreated(handle: BrowserWebviewHandle): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const finish = (cb: () => void) => { if (!settled) { settled = true; cb(); } };
    void handle.once('tauri://created', () => finish(resolve));
    void handle.once('tauri://error', (event) => finish(() => reject(new Error(formatUnknownError(event)))));
  });
}

async function waitForWindowCreated(handle: BrowserHolderWindowHandle): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const finish = (cb: () => void) => { if (!settled) { settled = true; cb(); } };
    void handle.once('tauri://created', () => finish(resolve));
    void handle.once('tauri://error', (event) => finish(() => reject(new Error(formatUnknownError(event)))));
  });
}

function normalizeUrl(raw: string): string {
  const value = raw.trim();
  if (!value) return DEFAULT_URL;
  if (/^[a-zA-Z][a-zA-Z\d+\-.]*:/.test(value)) return value;
  return `https://${value}`;
}

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
  const activeTabId = useSceneStore((s) => s.activeTabId);
  // Show webview only when this tab is active AND the session scene is visible
  const isSceneActive = activeTabId === 'session';
  const shouldShowWebview = isActive && isSceneActive;

  const isTauri = useMemo(() => isTauriEnvironment(), []);

  const startUrl = initialUrl ?? DEFAULT_URL;
  const viewportRef = useRef<HTMLDivElement>(null);
  const webviewRef = useRef<BrowserWebviewHandle | null>(null);
  const holderWindowRef = useRef<BrowserHolderWindowHandle | null>(null);
  const webviewSequenceRef = useRef(0);
  const currentUrlRef = useRef<string>(startUrl);
  const resizeFrameRef = useRef<number | null>(null);
  const webviewLabelRef = useRef<string>('');
  const inspectorUnlistenRef = useRef<(() => void) | null>(null);

  const [inputValue, setInputValue] = useState(startUrl);
  const [currentUrl, setCurrentUrl] = useState(startUrl);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isInspectorActive, setIsInspectorActive] = useState(false);

  const addContext = useContextStore((s) => s.addContext);

  /**
   * Sync webview bounds to the panel container.
   * Hides the webview if the container has no visible area (AuxPane collapsed, etc.).
   */
  const syncWebviewBounds = useCallback(async (handle?: BrowserWebviewHandle | null) => {
    const target = handle ?? webviewRef.current;
    if (!isTauri || !target || !viewportRef.current) return;

    const rect = viewportRef.current.getBoundingClientRect();
    if (rect.width <= 1 || rect.height <= 1) {
      await target.hide().catch(() => {});
      return;
    }

    const { LogicalPosition, LogicalSize } = await import('@tauri-apps/api/dpi');
    await Promise.all([
      target.setPosition(new LogicalPosition(rect.left, rect.top)),
      target.setSize(new LogicalSize(rect.width, rect.height)),
    ]);
    if (shouldShowWebview) {
      await target.show().catch(() => {});
    }
  }, [isTauri, shouldShowWebview]);

  const closeWebview = useCallback(async (handle?: BrowserWebviewHandle | null) => {
    const target = handle ?? webviewRef.current;
    if (!target) return;
    try {
      await target.close();
    } catch (e) {
      if (!isWebviewNotFoundError(e)) log.warn('Close browser panel webview failed', e);
    } finally {
      if (!handle || target === webviewRef.current) webviewRef.current = null;
    }
  }, []);

  const ensureHolderWindow = useCallback(async (): Promise<BrowserHolderWindowHandle> => {
    if (holderWindowRef.current) return holderWindowRef.current;

    const { Window } = await import('@tauri-apps/api/window');
    const existing = (await Window.getByLabel(PANEL_HOLDER_WINDOW_LABEL)) as BrowserHolderWindowHandle | null;
    if (existing) {
      holderWindowRef.current = existing;
      return existing;
    }

    const holder = new Window(PANEL_HOLDER_WINDOW_LABEL, {
      visible: false,
      decorations: false,
      skipTaskbar: true,
      shadow: false,
      width: 1,
      height: 1,
      x: -10000,
      y: -10000,
      title: 'Browser Panel Holder',
    }) as BrowserHolderWindowHandle;

    await waitForWindowCreated(holder);
    await holder.hide().catch(() => {});
    holderWindowRef.current = holder;
    return holder;
  }, []);

  const recreateWebview = useCallback(async (url: string) => {
    const previous = webviewRef.current;
    if (previous) await closeWebview(previous);

    const [{ Webview }, { getCurrentWindow }] = await Promise.all([
      import('@tauri-apps/api/webview'),
      import('@tauri-apps/api/window'),
    ]);
    const label = `embedded-browser-panel-view-${webviewSequenceRef.current++}`;
    webviewLabelRef.current = label;
    const handle = new Webview(getCurrentWindow(), label, {
      url,
      x: 0,
      y: 0,
      width: 960,
      height: 640,
    }) as unknown as BrowserWebviewHandle;

    await waitForWebviewCreated(handle);
    webviewRef.current = handle;
    return handle;
  }, [closeWebview]);

  const loadUrl = useCallback(async (rawUrl: string) => {
    const nextUrl = normalizeUrl(rawUrl);
    setInputValue(nextUrl);
    setCurrentUrl(nextUrl);
    currentUrlRef.current = nextUrl;
    setError(null);
    setIsLoading(true);
    if (inspectorUnlistenRef.current && webviewLabelRef.current) {
      void evalWebview(webviewLabelRef.current, CANCEL_INSPECTOR_SCRIPT).catch(() => {});
      inspectorUnlistenRef.current();
      inspectorUnlistenRef.current = null;
    }
    setIsInspectorActive(false);

    if (!isTauri) {
      setIsLoading(false);
      return;
    }

    try {
      const handle = await recreateWebview(nextUrl);
      await syncWebviewBounds(handle);
      if (shouldShowWebview) {
        await handle.show();
        await handle.setFocus();
      }
    } catch (loadError) {
      const message = formatUnknownError(loadError);
      log.error('Load browser panel url failed', loadError);
      setError(message);
    } finally {
      setIsLoading(false);
    }
  }, [isTauri, recreateWebview, shouldShowWebview, syncWebviewBounds]);

  const queueSync = useCallback(() => {
    if (resizeFrameRef.current !== null) window.cancelAnimationFrame(resizeFrameRef.current);
    resizeFrameRef.current = window.requestAnimationFrame(() => {
      resizeFrameRef.current = null;
      void syncWebviewBounds().catch((e) => log.warn('Sync browser panel webview bounds failed', e));
    });
  }, [syncWebviewBounds]);

  // Activate / deactivate webview based on shouldShowWebview
  useEffect(() => {
    if (!isTauri) return;

    if (shouldShowWebview) {
      if (!webviewRef.current) {
        void loadUrl(currentUrlRef.current).catch((e) => log.warn('Restore browser panel webview failed', e));
        return;
      }

      void (async () => {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await webviewRef.current?.reparent(getCurrentWindow());
        await syncWebviewBounds();
      })()
        .then(() => webviewRef.current?.show())
        .then(() => webviewRef.current?.setFocus())
        .catch((e) => log.warn('Activate browser panel webview failed', e));
      return;
    }

    if (webviewRef.current) {
      void ensureHolderWindow()
        .then((holder) => webviewRef.current?.reparent(holder))
        .then(() => holderWindowRef.current?.hide())
        .catch((e) => {
          log.warn('Reparent browser panel webview to holder failed', e);
          return closeWebview();
        })
        .catch((e) => log.warn('Close browser panel webview on deactivate failed', e));
    }
  }, [closeWebview, ensureHolderWindow, loadUrl, shouldShowWebview, syncWebviewBounds, isTauri]);

  // ResizeObserver + window resize → sync bounds
  useEffect(() => {
    if (!isTauri) return;

    const observer = new ResizeObserver(() => {
      if (shouldShowWebview) queueSync();
    });

    if (viewportRef.current) observer.observe(viewportRef.current);

    const handleResize = () => { if (shouldShowWebview) queueSync(); };
    window.addEventListener('resize', handleResize);

    return () => {
      observer.disconnect();
      window.removeEventListener('resize', handleResize);
      if (resizeFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
    };
  }, [isTauri, queueSync, shouldShowWebview]);

  // Cleanup on unmount
  useEffect(() => () => {
    if (inspectorUnlistenRef.current) {
      inspectorUnlistenRef.current();
      inspectorUnlistenRef.current = null;
    }
    if (webviewLabelRef.current) {
      void evalWebview(webviewLabelRef.current, CANCEL_INSPECTOR_SCRIPT).catch(() => {});
    }
    void closeWebview();
  }, [closeWebview]);

  // Hide webview when any overlay (modal, mission-control, toolbar-mode) is present.
  // Uses MutationObserver on document.body to detect overlay DOM nodes, so no
  // coupling with individual overlay components is needed.
  useEffect(() => {
    if (!isTauri) return;

    const OVERLAY_SELECTOR = '.modal-overlay, .canvas-mission-control';
    let hiddenByOverlay = false;

    const checkOverlays = () => {
      const hasOverlay = document.querySelector(OVERLAY_SELECTOR) !== null;
      if (hasOverlay && !hiddenByOverlay) {
        hiddenByOverlay = true;
        void webviewRef.current?.hide().catch(() => {});
      } else if (!hasOverlay && hiddenByOverlay) {
        hiddenByOverlay = false;
        if (shouldShowWebview) {
          void syncWebviewBounds()
            .then(() => webviewRef.current?.show())
            .catch(() => {});
        }
      }
    };

    const observer = new MutationObserver(checkOverlays);
    observer.observe(document.body, { childList: true, subtree: true });

    const handleToolbarActivating = () => {
      void webviewRef.current?.hide().catch(() => {});
    };
    window.addEventListener('toolbar-mode-activating', handleToolbarActivating);

    return () => {
      observer.disconnect();
      window.removeEventListener('toolbar-mode-activating', handleToolbarActivating);
    };
  }, [isTauri, shouldShowWebview, syncWebviewBounds]);

  useEffect(() => () => {
    if (holderWindowRef.current) {
      void holderWindowRef.current.close().catch((e) => log.warn('Close browser panel holder window failed', e));
    }
  }, []);

  const handleSubmit = useCallback((event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void loadUrl(inputValue);
  }, [inputValue, loadUrl]);

  const handleRefresh = useCallback(() => {
    void loadUrl(currentUrl);
  }, [currentUrl, loadUrl]);

  const handleInspector = useCallback(async () => {
    if (!isTauri || !webviewRef.current) return;

    if (isInspectorActive) {
      try {
        await evalWebview(webviewLabelRef.current, CANCEL_INSPECTOR_SCRIPT);
      } catch (e) {
        log.warn('Cancel inspector eval failed', e);
      }
      setIsInspectorActive(false);
      inspectorUnlistenRef.current?.();
      inspectorUnlistenRef.current = null;
      return;
    }

    const label = webviewLabelRef.current;
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
            sourceUrl: currentUrlRef.current,
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

      await evalWebview(label, createInspectorScript(label));
      setIsInspectorActive(true);

    } catch (e) {
      log.error('Start inspector failed', e);
      setIsInspectorActive(false);
    }
  }, [addContext, isInspectorActive, isTauri]);

  return (
    <div className="browser-panel">
      <form className="browser-panel__toolbar" onSubmit={handleSubmit}>
        <div className="browser-panel__address">
          <Globe size={16} />
          <input
            type="text"
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            placeholder="输入网址，例如 https://example.com"
            spellCheck={false}
          />
        </div>
        <IconButton
          type="button"
          variant="ghost"
          size="small"
          onClick={handleRefresh}
          disabled={isLoading}
          aria-label="刷新"
        >
          <RefreshCw size={14} className={isLoading ? 'browser-panel__spinning' : undefined} />
        </IconButton>
        {isTauri && (
          <IconButton
            type="button"
            variant="ghost"
            size="small"
            onClick={() => void handleInspector()}
            aria-label={isInspectorActive ? '退出元素选取' : '选取元素'}
            className={isInspectorActive ? 'browser-panel__inspector-btn--active' : undefined}
          >
            <MousePointer2 size={14} />
          </IconButton>
        )}
      </form>

      {error ? (
        <div className="browser-panel__error">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </div>
      ) : null}

      <div className="browser-panel__content">
        {!isTauri ? (
          <iframe
            className="browser-panel__iframe"
            src={currentUrl}
            title="Embedded Browser Panel"
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-downloads"
          />
        ) : (
          <div ref={viewportRef} className="browser-panel__webview-host">
            <div className="browser-panel__webview-placeholder">
              <Globe size={20} />
              <span>{currentUrl}</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default BrowserPanel;
