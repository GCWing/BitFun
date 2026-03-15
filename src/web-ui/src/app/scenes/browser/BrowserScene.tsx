import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, Globe, RefreshCw } from 'lucide-react';
import { IconButton } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import { useSceneStore } from '@/app/stores/sceneStore';
import './BrowserScene.scss';

const log = createLogger('BrowserScene');
const DEFAULT_URL = 'https://openbitfun.com/';
const BROWSER_HOLDER_WINDOW_LABEL = 'embedded-browser-holder-window';

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

type BrowserHolderWindowHandle = {
  close: () => Promise<void>;
  hide: () => Promise<void>;
  once: (event: string, handler: (event?: unknown) => void) => Promise<() => void>;
};

function formatUnknownError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === 'string') {
    return error;
  }

  if (error && typeof error === 'object') {
    const record = error as Record<string, unknown>;
    const payload = 'payload' in record ? record.payload : undefined;
    const message =
      (typeof record.message === 'string' && record.message) ||
      (payload && typeof payload === 'object' && typeof (payload as Record<string, unknown>).message === 'string'
        ? String((payload as Record<string, unknown>).message)
        : null);

    if (message) {
      return message;
    }

    try {
      return JSON.stringify(error);
    } catch {
      return String(error);
    }
  }

  return String(error);
}

function isWebviewNotFoundError(error: unknown): boolean {
  const message = formatUnknownError(error);
  return message.toLowerCase().includes('webview not found');
}

async function waitForWebviewCreated(handle: BrowserWebviewHandle): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let settled = false;

    const finish = (callback: () => void) => {
      if (settled) {
        return;
      }
      settled = true;
      callback();
    };

    void handle.once('tauri://created', () => {
      finish(resolve);
    });

    void handle.once('tauri://error', (event) => {
      finish(() => reject(new Error(formatUnknownError(event))));
    });
  });
}

async function waitForWindowCreated(handle: BrowserHolderWindowHandle): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let settled = false;

    const finish = (callback: () => void) => {
      if (settled) {
        return;
      }
      settled = true;
      callback();
    };

    void handle.once('tauri://created', () => {
      finish(resolve);
    });

    void handle.once('tauri://error', (event) => {
      finish(() => reject(new Error(formatUnknownError(event))));
    });
  });
}

function normalizeUrl(raw: string): string {
  const value = raw.trim();
  if (!value) {
    return DEFAULT_URL;
  }

  if (/^[a-zA-Z][a-zA-Z\d+\-.]*:/.test(value)) {
    return value;
  }

  return `https://${value}`;
}

const BrowserScene: React.FC = () => {
  const activeTabId = useSceneStore((state) => state.activeTabId);
  const isActive = activeTabId === 'browser';
  const isTauri = useMemo(() => isTauriEnvironment(), []);

  const viewportRef = useRef<HTMLDivElement>(null);
  const webviewRef = useRef<BrowserWebviewHandle | null>(null);
  const holderWindowRef = useRef<BrowserHolderWindowHandle | null>(null);
  const webviewSequenceRef = useRef(0);
  const currentUrlRef = useRef<string>(DEFAULT_URL);
  const resizeFrameRef = useRef<number | null>(null);

  const [inputValue, setInputValue] = useState(DEFAULT_URL);
  const [currentUrl, setCurrentUrl] = useState(DEFAULT_URL);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const syncWebviewBounds = useCallback(async (handle?: BrowserWebviewHandle | null) => {
    const target = handle ?? webviewRef.current;
    if (!isTauri || !isActive || !viewportRef.current || !target) {
      return;
    }

    const rect = viewportRef.current.getBoundingClientRect();
    if (rect.width <= 1 || rect.height <= 1) {
      return;
    }

    const [{ LogicalPosition, LogicalSize }] = await Promise.all([
      import('@tauri-apps/api/dpi'),
    ]);

    await Promise.all([
      target.setPosition(new LogicalPosition(rect.left, rect.top)),
      target.setSize(new LogicalSize(rect.width, rect.height)),
    ]);
  }, [isActive, isTauri]);

  const closeWebview = useCallback(async (handle?: BrowserWebviewHandle | null) => {
    const target = handle ?? webviewRef.current;
    if (!target) {
      return;
    }

    try {
      await target.close();
    } catch (closeError) {
      if (!isWebviewNotFoundError(closeError)) {
        log.warn('Close browser webview failed', closeError);
      }
    } finally {
      if (!handle || target === webviewRef.current) {
        webviewRef.current = null;
      }
    }
  }, []);

  const ensureHolderWindow = useCallback(async (): Promise<BrowserHolderWindowHandle> => {
    if (holderWindowRef.current) {
      return holderWindowRef.current;
    }

    const { Window } = await import('@tauri-apps/api/window');
    const existing = (await Window.getByLabel(BROWSER_HOLDER_WINDOW_LABEL)) as BrowserHolderWindowHandle | null;
    if (existing) {
      holderWindowRef.current = existing;
      return existing;
    }

    const holder = new Window(BROWSER_HOLDER_WINDOW_LABEL, {
      visible: false,
      decorations: false,
      skipTaskbar: true,
      shadow: false,
      width: 1,
      height: 1,
      x: -10000,
      y: -10000,
      title: 'Browser Holder',
    }) as BrowserHolderWindowHandle;

    await waitForWindowCreated(holder);
    await holder.hide().catch(() => {});
    holderWindowRef.current = holder;
    return holder;
  }, []);

  const recreateWebview = useCallback(async (url: string) => {
    const previous = webviewRef.current;
    if (previous) {
      await closeWebview(previous);
    }

    const [{ Webview }, { getCurrentWindow }] = await Promise.all([
      import('@tauri-apps/api/webview'),
      import('@tauri-apps/api/window'),
    ]);
    const nextLabel = `embedded-browser-view-${webviewSequenceRef.current++}`;
    const handle = new Webview(getCurrentWindow(), nextLabel, {
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

    if (!isTauri) {
      setIsLoading(false);
      return;
    }

    try {
      const handle = await recreateWebview(nextUrl);
      await syncWebviewBounds(handle);
      if (isActive) {
        await handle.show();
        await handle.setFocus();
      }
    } catch (loadError) {
      const message = formatUnknownError(loadError);
      log.error('Load browser url failed', loadError);
      setError(message);
    } finally {
      setIsLoading(false);
    }
  }, [isActive, isTauri, recreateWebview, syncWebviewBounds]);

  const queueSync = useCallback(() => {
    if (resizeFrameRef.current !== null) {
      window.cancelAnimationFrame(resizeFrameRef.current);
    }
    resizeFrameRef.current = window.requestAnimationFrame(() => {
      resizeFrameRef.current = null;
      void syncWebviewBounds().catch((syncError) => {
        log.warn('Sync browser webview bounds failed', syncError);
      });
    });
  }, [syncWebviewBounds]);

  useEffect(() => {
    if (!isTauri) {
      return;
    }

    if (isActive) {
      if (!webviewRef.current) {
        void loadUrl(currentUrlRef.current).catch((loadError) => {
          log.warn('Restore browser webview failed', loadError);
        });
        return;
      }

      void (async () => {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        await webviewRef.current?.reparent(getCurrentWindow());
        await syncWebviewBounds();
      })()
        .then(() => webviewRef.current?.show())
        .then(() => webviewRef.current?.setFocus())
        .catch((syncError) => {
          log.warn('Activate browser webview failed', syncError);
        });
      return;
    }

    if (webviewRef.current) {
      void ensureHolderWindow()
        .then((holderWindow) => webviewRef.current?.reparent(holderWindow))
        .then(() => holderWindowRef.current?.hide())
        .catch((reparentError) => {
          log.warn('Reparent browser webview to holder window failed', reparentError);
          return closeWebview();
        })
        .catch((closeError) => {
          log.warn('Close browser webview on tab switch failed', closeError);
        });
    }
  }, [closeWebview, ensureHolderWindow, isActive, isTauri, loadUrl, syncWebviewBounds]);

  useEffect(() => () => {
    if (holderWindowRef.current) {
      void holderWindowRef.current.close().catch((closeError) => {
        log.warn('Close browser holder window failed', closeError);
      });
    }
  }, []);

  useEffect(() => {
    if (!isTauri) {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (isActive) {
        queueSync();
      }
    });

    if (viewportRef.current) {
      observer.observe(viewportRef.current);
    }

    const handleResize = () => {
      if (isActive) {
        queueSync();
      }
    };

    window.addEventListener('resize', handleResize);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', handleResize);
      if (resizeFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
    };
  }, [isActive, isTauri, queueSync]);

  useEffect(() => () => {
    void closeWebview();
  }, [closeWebview]);

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
        if (isActive) {
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
  }, [isActive, isTauri, syncWebviewBounds]);

  const handleSubmit = useCallback((event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    void loadUrl(inputValue);
  }, [inputValue, loadUrl]);

  const handleRefresh = useCallback(() => {
    void loadUrl(currentUrl);
  }, [currentUrl, loadUrl]);

  return (
    <div className="browser-scene">
      <form className="browser-scene__toolbar" onSubmit={handleSubmit}>
        <div className="browser-scene__address">
          <Globe size={16} />
          <input
            type="text"
            value={inputValue}
            onChange={(event) => setInputValue(event.target.value)}
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
          <RefreshCw size={14} className={isLoading ? 'browser-scene__spinning' : undefined} />
        </IconButton>
      </form>

      {error ? (
        <div className="browser-scene__error">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </div>
      ) : null}

      <div className="browser-scene__content">
        {!isTauri ? (
          <iframe
            className="browser-scene__iframe"
            src={currentUrl}
            title="Embedded Browser"
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-downloads"
          />
        ) : (
          <div ref={viewportRef} className="browser-scene__webview-host">
            <div className="browser-scene__webview-placeholder">
              <Globe size={20} />
              <span>{currentUrl}</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default BrowserScene;
