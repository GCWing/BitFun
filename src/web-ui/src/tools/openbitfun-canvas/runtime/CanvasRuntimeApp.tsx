import React from 'react';
import {
  CanvasRuntimeErrorPanel,
  CanvasRuntimeRoot,
} from './CanvasRuntimeComponents';

type CanvasRuntimeRecord = Record<string, any>;

type CanvasRuntimeWindow = Window & {
  OpenBitFunCanvasSDK?: CanvasRuntimeRecord;
  OpenBitFunCanvasSDKAdapters?: CanvasRuntimeRecord;
  OpenBitFunCanvasRuntime?: CanvasRuntimeRecord;
  OpenBitFunCanvasContract?: {
    runtimeVersion?: string;
    sdkVersion?: string;
  };
  OpenBitFunCanvasRuntimeHooks?: {
    whenStateReady?: (callback: () => void) => (() => void);
  };
  ReactDOM?: {
    createRoot?: (element: HTMLElement) => {
      render: (node: React.ReactNode) => void;
    };
  };
  __openbitfunCanvasPost?: (type: string, payload?: CanvasRuntimeRecord) => void;
};

let reactRoot: { render: (node: React.ReactNode) => void } | null = null;
let renderComponent: React.ComponentType | null = null;
let rootElement: HTMLElement | null = null;

function runtimeWindow(): CanvasRuntimeWindow {
  return window as CanvasRuntimeWindow;
}

function currentRevision(): string {
  return document
    .querySelector('meta[name="openbitfun-canvas-revision"]')
    ?.getAttribute('content') || '';
}

function postCanvasMessage(type: string, payload: CanvasRuntimeRecord = {}) {
  const revision = currentRevision();
  const contract = runtimeWindow().OpenBitFunCanvasContract;
  const enrichedPayload = {
    sourceRevisionSeen: revision,
    runtimeVersion: contract?.runtimeVersion,
    sdkVersion: contract?.sdkVersion,
    ...payload,
  };
  const post = runtimeWindow().__openbitfunCanvasPost;
  if (post) {
    post(type, enrichedPayload);
    return;
  }
  window.parent?.postMessage({ type, ...enrichedPayload }, '*');
}

function errorText(error: unknown): string {
  if (error && typeof error === 'object') {
    const candidate = error as { stack?: unknown; message?: unknown };
    return String(candidate.stack || candidate.message || error);
  }
  return String(error || 'Canvas runtime error');
}

function postReady(): void {
  postCanvasMessage('openbitfun-canvas-ready');
}

function postRuntimeError(error: unknown, componentStack?: string): void {
  const details = error && typeof error === 'object'
    ? error as { message?: unknown; name?: unknown; stack?: unknown }
    : null;
  postCanvasMessage('openbitfun-canvas-runtime-error', {
    message: details?.message ? String(details.message) : String(error || 'Canvas runtime error'),
    name: details?.name ? String(details.name) : undefined,
    stack: details?.stack ? String(details.stack) : undefined,
    componentStack,
  });
}

function installSdkAdapters(): void {
  const target = runtimeWindow();
  if (!target.OpenBitFunCanvasSDK || !target.OpenBitFunCanvasSDKAdapters) return;
  target.OpenBitFunCanvasSDK = {
    ...target.OpenBitFunCanvasSDK,
    ...target.OpenBitFunCanvasSDKAdapters,
  };
}

function renderRuntimeRoot(): void {
  if (!rootElement || !renderComponent) return;
  const createRoot = runtimeWindow().ReactDOM?.createRoot;
  if (!createRoot) {
    reportRuntimeError(new Error('Canvas runtime requires ReactDOM.createRoot'));
    return;
  }

  try {
    if (!reactRoot) reactRoot = createRoot(rootElement);
    reactRoot.render(React.createElement(CanvasRuntimeRoot, {
      component: renderComponent,
      onReady: postReady,
      onError: postRuntimeError,
    }));
  } catch (error) {
    reportRuntimeError(error);
  }
}

function renderErrorPanel(error: unknown): void {
  if (!rootElement) return;
  const createRoot = runtimeWindow().ReactDOM?.createRoot;
  if (createRoot) {
    try {
      if (!reactRoot) reactRoot = createRoot(rootElement);
      reactRoot.render(React.createElement(CanvasRuntimeErrorPanel, { error }));
      return;
    } catch {
      // Fall through to plain DOM rendering below.
    }
  }

  rootElement.innerHTML =
    '<main style="max-width:860px;margin:0 auto;padding:12px;border:1px solid var(--openbitfun-color-border-default);border-radius:8px"><h1 style="font-size:var(--openbitfun-type-heading-page-font-size);font-weight:var(--openbitfun-type-heading-page-font-weight);line-height:var(--openbitfun-type-heading-page-line-height);margin:0 0 8px">Canvas runtime error</h1><pre style="white-space:pre-wrap;color:var(--openbitfun-canvas-danger)"></pre></main>';
  const pre = rootElement.querySelector('pre');
  if (pre) pre.textContent = errorText(error);
}

function reportRuntimeError(error: unknown): void {
  renderErrorPanel(error);
  postRuntimeError(error);
}

export function installOpenBitFunCanvasRuntimeApp(): void {
  rootElement = document.getElementById('openbitfun-canvas-root');
  const target = runtimeWindow();
  const previousRuntime = target.OpenBitFunCanvasRuntime || {};

  target.OpenBitFunCanvasRuntime = {
    ...previousRuntime,
    h: React.createElement,
    Fragment: React.Fragment,
    moduleStarted() {
      installSdkAdapters();
      postCanvasMessage('openbitfun-canvas-module-started');
    },
    reportRuntimeError,
    mount(component: React.ComponentType) {
      renderComponent = component;
      const whenStateReady = target.OpenBitFunCanvasRuntimeHooks?.whenStateReady;
      if (whenStateReady) {
        whenStateReady(renderRuntimeRoot);
      } else {
        renderRuntimeRoot();
      }
    },
  };
}
