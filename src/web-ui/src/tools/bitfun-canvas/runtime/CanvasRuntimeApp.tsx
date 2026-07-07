import React from 'react';

type CanvasRuntimeRecord = Record<string, any>;

type CanvasRuntimeWindow = Window & {
  BitfunCanvasSDK?: CanvasRuntimeRecord;
  BitfunCanvasSDKAdapters?: CanvasRuntimeRecord;
  BitfunCanvasRuntime?: CanvasRuntimeRecord;
  ReactDOM?: {
    createRoot?: (element: HTMLElement) => {
      render: (node: React.ReactNode) => void;
    };
  };
  __bitfunCanvasPost?: (type: string, payload?: CanvasRuntimeRecord) => void;
};

let reactRoot: { render: (node: React.ReactNode) => void } | null = null;
let renderComponent: React.ComponentType | null = null;
let rootElement: HTMLElement | null = null;

function runtimeWindow(): CanvasRuntimeWindow {
  return window as CanvasRuntimeWindow;
}

function currentRevision(): string {
  return document
    .querySelector('meta[name="bitfun-canvas-revision"]')
    ?.getAttribute('content') || '';
}

function postCanvasMessage(type: string, payload: CanvasRuntimeRecord = {}) {
  const revision = currentRevision();
  const post = runtimeWindow().__bitfunCanvasPost;
  if (post) {
    post(type, { sourceRevisionSeen: revision, ...payload });
    return;
  }
  window.parent?.postMessage({ type, sourceRevisionSeen: revision, ...payload }, '*');
}

function errorText(error: unknown): string {
  if (error && typeof error === 'object') {
    const candidate = error as { stack?: unknown; message?: unknown };
    return String(candidate.stack || candidate.message || error);
  }
  return String(error || 'Canvas runtime error');
}

function postReady(): void {
  postCanvasMessage('bitfun-canvas-ready');
}

function postRuntimeError(error: unknown): void {
  const details = error && typeof error === 'object'
    ? error as { message?: unknown; name?: unknown; stack?: unknown }
    : null;
  postCanvasMessage('bitfun-canvas-runtime-error', {
    message: details?.message ? String(details.message) : String(error || 'Canvas runtime error'),
    name: details?.name ? String(details.name) : undefined,
    stack: details?.stack ? String(details.stack) : undefined,
  });
}

function ErrorPanel({ error }: { error: unknown }) {
  return (
    <main style={{ maxWidth: 860, margin: '0 auto', padding: 12, border: '1px solid var(--border-base)', borderRadius: 8 }}>
      <h1 style={{ fontSize: 18, margin: '0 0 8px' }}>Canvas runtime error</h1>
      <pre style={{ whiteSpace: 'pre-wrap', color: 'var(--bitfun-canvas-danger)' }}>{errorText(error)}</pre>
    </main>
  );
}

class RuntimeErrorBoundary extends React.Component<{ children?: React.ReactNode }, { error: unknown | null }> {
  state = { error: null };

  static getDerivedStateFromError(error: unknown) {
    return { error };
  }

  componentDidCatch(error: unknown) {
    postRuntimeError(error);
  }

  render() {
    if (this.state.error) return <ErrorPanel error={this.state.error} />;
    return this.props.children;
  }
}

function RuntimeRoot() {
  React.useEffect(() => {
    postReady();
    const timeout = window.setTimeout(postReady, 0);
    return () => window.clearTimeout(timeout);
  }, []);

  const Component = renderComponent;
  return (
    <RuntimeErrorBoundary>
      {Component ? <Component /> : null}
    </RuntimeErrorBoundary>
  );
}

function installSdkAdapters(): void {
  const target = runtimeWindow();
  if (!target.BitfunCanvasSDK || !target.BitfunCanvasSDKAdapters) return;
  target.BitfunCanvasSDK = {
    ...target.BitfunCanvasSDK,
    ...target.BitfunCanvasSDKAdapters,
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
    reactRoot.render(<RuntimeRoot />);
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
      reactRoot.render(<ErrorPanel error={error} />);
      return;
    } catch {
      // Fall through to plain DOM rendering below.
    }
  }

  rootElement.innerHTML =
    '<main style="max-width:860px;margin:0 auto;padding:12px;border:1px solid var(--border-base);border-radius:8px"><h1 style="font-size:18px;margin:0 0 8px">Canvas runtime error</h1><pre style="white-space:pre-wrap;color:var(--bitfun-canvas-danger)"></pre></main>';
  const pre = rootElement.querySelector('pre');
  if (pre) pre.textContent = errorText(error);
}

function reportRuntimeError(error: unknown): void {
  renderErrorPanel(error);
  postRuntimeError(error);
}

export function installBitfunCanvasRuntimeApp(): void {
  rootElement = document.getElementById('bitfun-canvas-root');
  const target = runtimeWindow();
  const previousRuntime = target.BitfunCanvasRuntime || {};

  target.BitfunCanvasRuntime = {
    ...previousRuntime,
    h: React.createElement,
    Fragment: React.Fragment,
    moduleStarted() {
      installSdkAdapters();
      postCanvasMessage('bitfun-canvas-module-started');
    },
    reportRuntimeError,
    mount(component: React.ComponentType) {
      renderComponent = component;
      renderRuntimeRoot();
      postReady();
    },
  };
}
