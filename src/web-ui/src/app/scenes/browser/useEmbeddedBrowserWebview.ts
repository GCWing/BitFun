import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { BLANK_TARGET_INTERCEPT_SCRIPT } from './browserInspectorScript';
import { STREAM_RENDER_OPTIMIZATION_SCRIPT } from './browserStreamPerformanceScript';
import { validateUrl } from './browserUrlCheck';

const WEBVIEW_RESIZE_DEBOUNCE_MS = 160;
const WEBVIEW_BOUNDS_EPSILON = 1;
const WEBVIEW_BOUNDS_WAIT_TIMEOUT_MS = 2000;
const OVERLAY_SELECTOR = '.modal-overlay, .canvas-mission-control';
const BROWSER_WEBVIEW_PAGE_LOAD_EVENT = 'browser-webview-page-load';
const WEBVIEW_CREATE_RETRY_DELAYS_MS = [0, 250, 750];

// #region agent log
function writeBrowserWebviewDiagnostic(
  hypothesis: string,
  location: string,
  message: string,
  data: Record<string, unknown>,
): void {
  void fetch('http://127.0.0.1:7469/log', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      hypothesis,
      location,
      message,
      data,
      timestamp: new Date().toISOString(),
    }),
  }).catch(() => {});
}
// #endregion

type BrowserLogger = {
  warn: (message: string, ...args: unknown[]) => void;
  error: (message: string, ...args: unknown[]) => void;
};

type BrowserWebviewHandle = {
  close: () => Promise<void>;
  hide: () => Promise<void>;
  label: string;
  setFocus: () => Promise<void>;
  show: () => Promise<void>;
};

type WebviewBounds = {
  left: number;
  top: number;
  width: number;
  height: number;
};

type BrowserWebviewPageLoadPayload = {
  label: string;
  event: 'started' | 'finished';
  url: string;
};

export interface UseEmbeddedBrowserWebviewOptions {
  defaultUrl: string;
  initialUrl?: string;
  isVisible: boolean;
  labelPrefix: string;
  log: BrowserLogger;
}

function isTauriEnvironment(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

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
    try {
      return JSON.stringify(error);
    } catch {
      return String(error);
    }
  }
  return String(error);
}

function isWebviewNotFoundError(error: unknown): boolean {
  return formatUnknownError(error).toLowerCase().includes('webview not found');
}

function isTransientWebviewCreationError(error: unknown): boolean {
  const message = formatUnknownError(error).toLowerCase();
  return message.includes('0x80070057')
    || message.includes('0x8007139f')
    || message.includes('failed to create webview');
}

function normalizeUrl(raw: string, defaultUrl: string): string {
  const value = raw.trim();
  if (!value) return defaultUrl;
  if (/^[a-zA-Z][a-zA-Z\d+\-.]*:/.test(value)) return value;
  return `https://${value}`;
}

async function evalWebview(label: string, script: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('browser_webview_eval', { request: { label, script } });
}

async function injectBrowserPageScripts(label: string): Promise<void> {
  await evalWebview(label, `${BLANK_TARGET_INTERCEPT_SCRIPT};\n${STREAM_RENDER_OPTIMIZATION_SCRIPT};`);
}

async function navigateWebview(label: string, url: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('browser_webview_navigate', { request: { label, url } });
}

async function reloadWebview(label: string): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('browser_webview_reload', { request: { label } });
}

async function setWebviewBounds(label: string, bounds: WebviewBounds): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('browser_webview_set_bounds', {
    request: {
      label,
      x: bounds.left,
      y: bounds.top,
      width: bounds.width,
      height: bounds.height,
    },
  });
}

async function createBrowserWebview(label: string, url: string, bounds: WebviewBounds): Promise<BrowserWebviewHandle> {
  const [{ invoke }, { Webview }] = await Promise.all([
    import('@tauri-apps/api/core'),
    import('@tauri-apps/api/webview'),
  ]);
  await invoke('browser_webview_create', {
    request: {
      label,
      url,
      x: bounds.left,
      y: bounds.top,
      width: bounds.width,
      height: bounds.height,
    },
  });
  const handle = await Webview.getByLabel(label) as unknown as BrowserWebviewHandle | null;
  if (!handle) {
    throw new Error(`Webview not found after creation: ${label}`);
  }
  return handle;
}

export function useEmbeddedBrowserWebview(options: UseEmbeddedBrowserWebviewOptions) {
  const { defaultUrl, initialUrl, isVisible, labelPrefix, log } = options;
  const isTauri = useMemo(() => isTauriEnvironment(), []);
  const startUrl = initialUrl ?? defaultUrl;

  const viewportRef = useRef<HTMLDivElement>(null);
  const webviewRef = useRef<BrowserWebviewHandle | null>(null);
  const webviewSequenceRef = useRef(0);
  const currentUrlRef = useRef<string>(startUrl);
  const resizeTimerRef = useRef<number | null>(null);
  const lastBoundsRef = useRef<WebviewBounds | null>(null);
  const webviewLabelRef = useRef<string>('');
  const pageLoadUnlistenRef = useRef<(() => void) | null>(null);

  const [inputValue, setInputValue] = useState(startUrl);
  const [currentUrl, setCurrentUrl] = useState(startUrl);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [webviewLabel, setWebviewLabel] = useState('');

  const readViewportBounds = useCallback((): WebviewBounds | null => {
    if (!viewportRef.current) return null;

    const rect = viewportRef.current.getBoundingClientRect();
    if (rect.width <= 1 || rect.height <= 1) return null;

    return {
      left: Math.round(rect.left),
      top: Math.round(rect.top),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    };
  }, []);

  const waitForViewportBounds = useCallback(async (): Promise<WebviewBounds> => {
    const startedAt = performance.now();

    while (performance.now() - startedAt < WEBVIEW_BOUNDS_WAIT_TIMEOUT_MS) {
      const bounds = readViewportBounds();
      if (bounds) return bounds;

      await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
    }

    throw new Error('Browser viewport did not become visible before webview creation');
  }, [readViewportBounds]);

  const syncWebviewBounds = useCallback(async (handle?: BrowserWebviewHandle | null) => {
    const target = handle ?? webviewRef.current;
    if (!isTauri || !target || !viewportRef.current) return;

    const nextBounds = readViewportBounds();
    if (!nextBounds) {
      // #region agent log
      const viewport = viewportRef.current;
      const viewportRect = viewport?.getBoundingClientRect();
      const ancestors: Array<Record<string, unknown>> = [];
      let ancestor = viewport?.parentElement ?? null;
      for (let index = 0; ancestor && index < 6; index += 1, ancestor = ancestor.parentElement) {
        const style = window.getComputedStyle(ancestor);
        const rect = ancestor.getBoundingClientRect();
        ancestors.push({
          tagName: ancestor.tagName,
          className: ancestor.className,
          display: style.display,
          visibility: style.visibility,
          width: rect.width,
          height: rect.height,
        });
      }
      writeBrowserWebviewDiagnostic('F', 'useEmbeddedBrowserWebview.syncWebviewBounds', isVisible
        ? 'keeping active webview at its last valid bounds while viewport is transiently unavailable'
        : 'hiding inactive webview because viewport has no usable bounds', {
        label: target.label,
        isVisible,
        hasViewport: Boolean(viewport),
        isConnected: viewport?.isConnected ?? false,
        viewportRect: viewportRect ? {
          left: viewportRect.left,
          top: viewportRect.top,
          width: viewportRect.width,
          height: viewportRect.height,
        } : null,
        windowSize: { width: window.innerWidth, height: window.innerHeight },
        ancestors,
      });
      // #endregion
      if (!isVisible) {
        await target.hide().catch(() => {});
      }
      return;
    }

    const previous = lastBoundsRef.current;
    const boundsChanged =
      !previous ||
      Math.abs(previous.left - nextBounds.left) > WEBVIEW_BOUNDS_EPSILON ||
      Math.abs(previous.top - nextBounds.top) > WEBVIEW_BOUNDS_EPSILON ||
      Math.abs(previous.width - nextBounds.width) > WEBVIEW_BOUNDS_EPSILON ||
      Math.abs(previous.height - nextBounds.height) > WEBVIEW_BOUNDS_EPSILON;

    if (boundsChanged) {
      await setWebviewBounds(target.label, nextBounds);
      lastBoundsRef.current = nextBounds;
    }
    if (isVisible) {
      // #region agent log
      writeBrowserWebviewDiagnostic('F', 'useEmbeddedBrowserWebview.syncWebviewBounds', 'showing webview after bounds sync', {
        label: target.label,
        bounds: nextBounds,
      });
      // #endregion
      await target.show().catch(() => {});
    }
  }, [isTauri, isVisible, readViewportBounds]);

  const closeWebview = useCallback(async (handle?: BrowserWebviewHandle | null) => {
    const target = handle ?? webviewRef.current;
    if (!target) return;

    try {
      await target.close();
    } catch (closeError) {
      if (!isWebviewNotFoundError(closeError)) {
        log.warn('Close browser webview failed', closeError);
      }
    } finally {
      if (!handle || target === webviewRef.current) {
        webviewRef.current = null;
        webviewLabelRef.current = '';
        setWebviewLabel('');
        lastBoundsRef.current = null;
        pageLoadUnlistenRef.current?.();
        pageLoadUnlistenRef.current = null;
      }
    }
  }, [log]);

  const startPageLoadListener = useCallback(async (label: string) => {
    pageLoadUnlistenRef.current?.();
    pageLoadUnlistenRef.current = null;

    const { listen } = await import('@tauri-apps/api/event');
    pageLoadUnlistenRef.current = await listen<BrowserWebviewPageLoadPayload>(
      BROWSER_WEBVIEW_PAGE_LOAD_EVENT,
      ({ payload }) => {
        if (!payload || payload.label !== label) return;
        if (payload.event === 'started') {
          setIsLoading(true);
        } else {
          setIsLoading(false);
        }
        if (payload.url && payload.url !== currentUrlRef.current) {
          currentUrlRef.current = payload.url;
          setInputValue(payload.url);
          setCurrentUrl(payload.url);
          setError(null);
          injectBrowserPageScripts(label).catch(() => {});
        }
      },
    );
  }, []);

  const createWebview = useCallback(async (url: string) => {
    const previous = webviewRef.current;
    if (previous) await closeWebview(previous);

    const { Webview } = await import('@tauri-apps/api/webview');
    const initialBounds = await waitForViewportBounds();
    let lastError: unknown = null;

    for (let attempt = 0; attempt < WEBVIEW_CREATE_RETRY_DELAYS_MS.length; attempt += 1) {
      const delay = WEBVIEW_CREATE_RETRY_DELAYS_MS[attempt];
      if (delay > 0) {
        await new Promise((resolve) => window.setTimeout(resolve, delay));
      }

      const label = `${labelPrefix}-${webviewSequenceRef.current++}`;
      webviewLabelRef.current = label;
      setWebviewLabel(label);
      try {
        const handle = await createBrowserWebview(label, url, initialBounds);
        webviewRef.current = handle;
        lastBoundsRef.current = initialBounds;
        await injectBrowserPageScripts(label);
        await startPageLoadListener(label);
        return handle;
      } catch (creationError) {
        lastError = creationError;
        const staleHandle = await Webview.getByLabel(label).catch(() => null);
        await staleHandle?.close().catch(() => {});
        if (!isTransientWebviewCreationError(creationError)
          || attempt === WEBVIEW_CREATE_RETRY_DELAYS_MS.length - 1) {
          throw creationError;
        }
        log.warn('Retry browser webview creation after transient WebView2 error', {
          attempt: attempt + 1,
          error: formatUnknownError(creationError),
        });
      }
    }

    throw lastError;
  }, [closeWebview, labelPrefix, log, startPageLoadListener, waitForViewportBounds]);

  const navigateExistingWebview = useCallback(async (url: string): Promise<boolean> => {
    const label = webviewLabelRef.current;
    if (!label || !webviewRef.current) return false;

    try {
      await navigateWebview(label, url);
      window.setTimeout(() => {
        if (webviewLabelRef.current === label) {
          void injectBrowserPageScripts(label).catch(() => {});
        }
      }, 1000);
      window.setTimeout(() => {
        if (webviewLabelRef.current === label) {
          void injectBrowserPageScripts(label).catch(() => {});
        }
      }, 2500);
      return true;
    } catch (navigationError) {
      log.warn('Navigate browser webview via existing instance failed', navigationError);
      return false;
    }
  }, [log]);

  const loadUrl = useCallback(async (rawUrl: string) => {
    const nextUrl = normalizeUrl(rawUrl, defaultUrl);
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
      validateUrl(nextUrl);
      let handle = webviewRef.current;
      if (!handle) {
        handle = await createWebview(nextUrl);
      } else {
        const navigated = await navigateExistingWebview(nextUrl);
        if (!navigated) {
          handle = await createWebview(nextUrl);
        }
      }
      await syncWebviewBounds(handle);
      if (isVisible) {
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
  }, [createWebview, defaultUrl, isTauri, isVisible, log, navigateExistingWebview, syncWebviewBounds]);

  const queueSync = useCallback(() => {
    if (resizeTimerRef.current !== null) window.clearTimeout(resizeTimerRef.current);
    resizeTimerRef.current = window.setTimeout(() => {
      resizeTimerRef.current = null;
      void syncWebviewBounds().catch((syncError) => {
        log.warn('Sync browser webview bounds failed', syncError);
      });
    }, WEBVIEW_RESIZE_DEBOUNCE_MS);
  }, [log, syncWebviewBounds]);

  useEffect(() => {
    if (!isTauri) return;

    if (isVisible) {
      // #region agent log
      writeBrowserWebviewDiagnostic('G', 'useEmbeddedBrowserWebview.visibilityEffect', 'browser surface activated', {
        label: webviewRef.current?.label ?? null,
      });
      // #endregion
      if (!webviewRef.current) {
        void loadUrl(currentUrlRef.current).catch((loadError) => {
          log.warn('Restore browser webview failed', loadError);
        });
        return;
      }

      void syncWebviewBounds()
        .then(() => webviewRef.current?.show())
        .then(() => webviewRef.current?.setFocus())
        .catch((syncError) => {
          log.warn('Activate browser webview failed', syncError);
        });
      return;
    }

    if (webviewRef.current) {
      // #region agent log
      writeBrowserWebviewDiagnostic('G', 'useEmbeddedBrowserWebview.visibilityEffect', 'hiding webview because browser surface deactivated', {
        label: webviewRef.current.label,
      });
      // #endregion
      void webviewRef.current.hide().catch((hideError) => {
        log.warn('Hide browser webview on deactivate failed', hideError);
      });
    }
  }, [isTauri, isVisible, loadUrl, log, syncWebviewBounds]);

  useEffect(() => {
    if (!isTauri) return;

    const observer = new ResizeObserver(() => {
      if (isVisible) queueSync();
    });

    if (viewportRef.current) observer.observe(viewportRef.current);

    const handleResize = () => {
      if (isVisible) queueSync();
    };
    window.addEventListener('resize', handleResize);

    return () => {
      observer.disconnect();
      window.removeEventListener('resize', handleResize);
      if (resizeTimerRef.current !== null) {
        window.clearTimeout(resizeTimerRef.current);
        resizeTimerRef.current = null;
      }
    };
  }, [isTauri, isVisible, queueSync]);

  useEffect(() => () => {
    pageLoadUnlistenRef.current?.();
    pageLoadUnlistenRef.current = null;
    if (resizeTimerRef.current !== null) {
      window.clearTimeout(resizeTimerRef.current);
      resizeTimerRef.current = null;
    }
    void closeWebview();
  }, [closeWebview]);

  useEffect(() => {
    if (!isTauri) return;

    let hiddenByOverlay = false;
    const checkOverlays = () => {
      const overlay = document.querySelector<HTMLElement>(OVERLAY_SELECTOR);
      const hasOverlay = overlay !== null;
      // #region agent log
      if (overlay) {
        const style = window.getComputedStyle(overlay);
        const rect = overlay.getBoundingClientRect();
        writeBrowserWebviewDiagnostic('E', 'useEmbeddedBrowserWebview.checkOverlays', 'overlay selector matched', {
          label: webviewRef.current?.label ?? null,
          className: overlay.className,
          display: style.display,
          visibility: style.visibility,
          opacity: style.opacity,
          width: rect.width,
          height: rect.height,
          hiddenByOverlay,
        });
      }
      // #endregion
      if (hasOverlay && !hiddenByOverlay) {
        hiddenByOverlay = true;
        // #region agent log
        writeBrowserWebviewDiagnostic('E', 'useEmbeddedBrowserWebview.checkOverlays', 'hiding webview because overlay selector exists', {
          label: webviewRef.current?.label ?? null,
          className: overlay?.className ?? null,
        });
        // #endregion
        void webviewRef.current?.hide().catch(() => {});
      } else if (!hasOverlay && hiddenByOverlay) {
        hiddenByOverlay = false;
        if (isVisible) {
          // #region agent log
          writeBrowserWebviewDiagnostic('E', 'useEmbeddedBrowserWebview.checkOverlays', 'showing webview because overlay selector disappeared', {
            label: webviewRef.current?.label ?? null,
          });
          // #endregion
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
  }, [isTauri, isVisible, syncWebviewBounds]);

  const evalInWebview = useCallback(async (script: string) => {
    const label = webviewLabelRef.current;
    if (!isTauri || !label) return;
    await evalWebview(label, script);
  }, [isTauri]);

  const goBack = useCallback(() => {
    void evalInWebview('history.back()').catch(() => {});
  }, [evalInWebview]);

  const goForward = useCallback(() => {
    void evalInWebview('history.forward()').catch(() => {});
  }, [evalInWebview]);

  const reload = useCallback(() => {
    const label = webviewLabelRef.current;
    if (!isTauri || !label) return;
    void reloadWebview(label).catch(() => {});
  }, [isTauri]);

  const getWebviewLabel = useCallback(() => webviewLabelRef.current, []);
  const getCurrentUrl = useCallback(() => currentUrlRef.current, []);
  const hasWebview = useCallback(() => webviewRef.current !== null, []);

  return useMemo(() => ({
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
  }), [
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
    viewportRef,
    webviewLabel,
  ]);
}
