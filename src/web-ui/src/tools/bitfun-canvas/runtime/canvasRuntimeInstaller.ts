type CanvasRuntimeRecord = Record<string, any>;
type RuntimeReact = any;
type RuntimeReactDOM = any;
type RuntimeWindow = Window & {
  React?: unknown;
  ReactDOM?: unknown;
  BitfunCanvasSDK?: CanvasRuntimeRecord;
  BitfunCanvasSDKAdapters?: CanvasRuntimeRecord;
  BitfunCanvasRuntimeHooks?: Record<string, unknown>;
  BitfunCanvasRuntime?: CanvasRuntimeRecord;
};

export function buildCanvasRuntimeInstallerScript(revision: string): string {
  return `(${installBitfunCanvasRuntime.toString()})(${JSON.stringify(revision)});`;
}

function installBitfunCanvasRuntime(initialRevision: string): void {
  const runtimeWindow = window as unknown as RuntimeWindow;
  const React = runtimeWindow.React as RuntimeReact | undefined;
  const ReactDOM = runtimeWindow.ReactDOM as RuntimeReactDOM | undefined;
  const rootElement = document.getElementById('bitfun-canvas-root');
  let reactRoot: ReturnType<RuntimeReactDOM['createRoot']> | null = null;
  let renderComponent: any = null;
  let hostTheme = makeTheme({
    type: 'auto',
    bg: 'var(--bitfun-canvas-bg)',
    panel: 'var(--bitfun-canvas-panel)',
    fg: 'var(--bitfun-canvas-fg)',
    muted: 'var(--bitfun-canvas-muted)',
    border: 'var(--bitfun-canvas-border)',
    accent: 'var(--bitfun-canvas-accent)',
    success: 'var(--bitfun-canvas-success)',
    warning: 'var(--bitfun-canvas-warning)',
    danger: 'var(--bitfun-canvas-danger)',
    info: 'var(--bitfun-canvas-info)',
  });
  let sourceRevision = initialRevision;
  let hostStateValues: CanvasRuntimeRecord = {};
  let designModeEnabled = false;
  let hoveredDesignElement: Element | null = null;
  const stateListeners = new Set<() => void>();
  let requestSeq = 0;
  const pendingRequests = new Map<
    string,
    { resolve: (value: unknown) => void; reject: (reason?: unknown) => void }
  >();

  if (!React || !ReactDOM) {
    reportRuntimeError(new Error('Canvas runtime requires React and ReactDOM'));
    return;
  }

  function makeTheme(tokens: CanvasRuntimeRecord): CanvasRuntimeRecord {
    const readToken = (value: unknown, fallback: string): string =>
      value === undefined || value === null || value === '' ? fallback : String(value);
    const bg = readToken(tokens.bg, 'var(--bitfun-canvas-bg)');
    const panel = readToken(tokens.panel, 'var(--bitfun-canvas-panel)');
    const fg = readToken(tokens.fg, 'var(--bitfun-canvas-fg)');
    const muted = readToken(tokens.muted, 'var(--bitfun-canvas-muted)');
    const border = readToken(tokens.border, 'var(--bitfun-canvas-border)');
    const accent = readToken(tokens.accent, 'var(--bitfun-canvas-accent)');
    const success = readToken(tokens.success, 'var(--bitfun-canvas-success)');
    const warning = readToken(tokens.warning, 'var(--bitfun-canvas-warning)');
    const danger = readToken(tokens.danger, 'var(--bitfun-canvas-danger)');
    const info = readToken(tokens.info, 'var(--bitfun-canvas-info)');
    const token = (value: string, fields: CanvasRuntimeRecord = {}) =>
      Object.assign(new String(value), {
        toString() {
          return value;
        },
        valueOf() {
          return value;
        },
        ...fields,
      });
    const semanticBg = {
      editor: bg,
      chrome: 'var(--element-bg-subtle)',
      elevated: panel,
    };
    const semanticText = {
      primary: fg,
      secondary: muted,
      tertiary: muted,
      quaternary: muted,
      link: accent,
      onAccent: 'var(--color-static-white)',
    };
    const semanticFill = {
      primary: panel,
      secondary: 'var(--element-bg-base)',
      tertiary: 'var(--element-bg-soft)',
      quaternary: 'var(--element-bg-subtle)',
    };
    const semanticStroke = {
      primary: border,
      secondary: 'var(--border-base)',
      tertiary: 'var(--border-subtle)',
      focused: accent,
    };
    const semanticAccent = {
      primary: accent,
      control: accent,
      controlHover: accent,
      success,
      warning,
      danger,
      info,
    };
    const category = {
      gray: muted,
      purple: accent,
      green: success,
      yellow: warning,
      cyan: info,
      pink: danger,
      blue: accent,
      orange: warning,
    };
    const diff = {
      insertedLine: 'color-mix(in srgb, var(--color-success) 12%, transparent)',
      removedLine: 'color-mix(in srgb, var(--color-error) 12%, transparent)',
      stripAdded: success,
      stripRemoved: danger,
    };
    const semanticTokens = {
      bg: semanticBg,
      text: semanticText,
      fill: semanticFill,
      stroke: semanticStroke,
      accent: semanticAccent,
      diff,
      category,
    };
    return {
      ...tokens,
      bg: token(bg, { canvas: bg, ...semanticBg }),
      panel,
      fg,
      muted,
      border,
      accent: token(accent, semanticAccent),
      success,
      warning,
      danger,
      info,
      text: semanticText,
      fill: semanticFill,
      stroke: semanticStroke,
      category,
      diff,
      palette: category,
      status: { success, warning, danger, info },
      tokens: semanticTokens,
    };
  }

  function applyTheme(nextTheme: CanvasRuntimeRecord): void {
    if (!nextTheme || typeof nextTheme !== 'object') return;
    const allowed = ['bg', 'panel', 'fg', 'muted', 'border', 'accent', 'success', 'warning', 'danger', 'info'];
    const rootStyle = document.documentElement.style;
    if (nextTheme.vars && typeof nextTheme.vars === 'object') {
      for (const [name, value] of Object.entries(nextTheme.vars)) {
        if (/^--[a-zA-Z0-9_-]+$/.test(name) && typeof value === 'string' && value.trim()) {
          rootStyle.setProperty(name, value.trim());
        }
      }
    }
    for (const key of allowed) {
      const value = nextTheme[key];
      if (typeof value === 'string' && value.trim()) {
        rootStyle.setProperty(`--bitfun-canvas-${key}`, value.trim());
      }
    }
    if (nextTheme.type === 'dark' || nextTheme.type === 'light') {
      document.documentElement.style.colorScheme = nextTheme.type;
    }
    hostTheme = makeTheme({ ...hostTheme, ...nextTheme });
    rerender();
  }

  function useHostTheme(): CanvasRuntimeRecord {
    const [, force] = React.useState(0);
    React.useEffect(() => {
      const listener = () => force((value: number) => value + 1);
      stateListeners.add(listener);
      return () => {
        stateListeners.delete(listener);
      };
    }, []);
    return hostTheme;
  }

  function useCanvasState(key: string, defaultValue: unknown): [unknown, (nextValue: unknown) => void] {
    const initialValue = Object.prototype.hasOwnProperty.call(hostStateValues, key)
      ? hostStateValues[key]
      : defaultValue;
    const [value, setValue] = React.useState(initialValue);
    React.useEffect(() => {
      const listener = () => {
        setValue(Object.prototype.hasOwnProperty.call(hostStateValues, key) ? hostStateValues[key] : defaultValue);
      };
      stateListeners.add(listener);
      return () => {
        stateListeners.delete(listener);
      };
    }, [key, defaultValue]);
    const update = React.useCallback(
      (nextValue: unknown) => {
        const resolved =
          typeof nextValue === 'function'
            ? (nextValue as (value: unknown) => unknown)(hostStateValues[key] ?? defaultValue)
            : nextValue;
        hostStateValues = { ...hostStateValues, [key]: resolved };
        setValue(resolved);
        window.parent?.postMessage(
          {
            type: 'bitfun-canvas-save-state',
            sourceRevisionSeen: sourceRevision,
            values: hostStateValues,
            updatedAt: Date.now(),
          },
          '*',
        );
      },
      [key, defaultValue],
    );
    return [value, update];
  }

  function useCanvasAction(): (action: unknown) => Promise<unknown> {
    return React.useCallback(
      (action: unknown) =>
        new Promise((resolve, reject) => {
          const requestId = `canvas-action-${++requestSeq}`;
          pendingRequests.set(requestId, { resolve, reject });
          window.parent?.postMessage({ type: 'bitfun-canvas-action', requestId, action }, '*');
        }),
      [],
    );
  }

  function errorText(error: any): string {
    return String(error?.stack || error?.message || error);
  }

  function postReady(): void {
    window.parent?.postMessage({ type: 'bitfun-canvas-ready', sourceRevisionSeen: sourceRevision }, '*');
  }

  function postRuntimeError(error: any): void {
    window.parent?.postMessage({
      type: 'bitfun-canvas-runtime-error',
      message: String(error?.message || error || 'Canvas runtime error'),
      name: error?.name ? String(error.name) : undefined,
      stack: error?.stack ? String(error.stack) : undefined,
    }, '*');
  }

  function isSelectableCanvasElement(target: EventTarget | null): target is Element {
    if (!(target instanceof Element)) return false;
    if (target === document.documentElement || target === document.body || target === rootElement) return false;
    return Boolean(rootElement?.contains(target));
  }

  function elementText(element: Element): string | undefined {
    const text = (element.textContent || '').replace(/\s+/g, ' ').trim();
    return text ? text.slice(0, 180) : undefined;
  }

  function elementSelector(element: Element): string {
    const escapeCss = (value: string) => {
      const css = (window as unknown as { CSS?: { escape?: (input: string) => string } }).CSS;
      return css?.escape ? css.escape(value) : value.replace(/["\\]/g, '\\$&');
    };
    if (element.id) return `#${escapeCss(element.id)}`;
    const testId = element.getAttribute('data-testid') || element.getAttribute('data-test-id');
    if (testId) return `[data-testid="${escapeCss(testId)}"]`;
    const parts: string[] = [];
    let current: Element | null = element;
    while (current && current !== rootElement && current !== document.body) {
      const tag = current.tagName.toLowerCase();
      const className = Array.from(current.classList).slice(0, 2).map(name => `.${escapeCss(name)}`).join('');
      const parent: Element | null = current.parentElement;
      const sameTagIndex = parent
        ? Array.from(parent.children).filter((child: Element) => child.tagName === current?.tagName).indexOf(current) + 1
        : 0;
      parts.unshift(`${tag}${className}${sameTagIndex > 1 ? `:nth-of-type(${sameTagIndex})` : ''}`);
      current = parent;
      if (parts.length >= 5) break;
    }
    return parts.join(' > ') || element.tagName.toLowerCase();
  }

  function elementComponentName(element: Element): string | undefined {
    const className = Array.from(element.classList).find(name => name.startsWith('bf-') || name.startsWith('bitfun-'));
    if (!className) return undefined;
    return className
      .replace(/^bf-/, '')
      .replace(/^bitfun-canvas-/, '')
      .split('-')
      .filter(Boolean)
      .map(part => part.charAt(0).toUpperCase() + part.slice(1))
      .join('');
  }

  function elementReference(element: Element): CanvasRuntimeRecord {
    const rect = element.getBoundingClientRect();
    return {
      nodeId: element.id || null,
      component: elementComponentName(element),
      tagName: element.tagName.toLowerCase(),
      selector: elementSelector(element),
      text: elementText(element),
      bounds: {
        x: Math.round(rect.x),
        y: Math.round(rect.y),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
      },
    };
  }

  function clearDesignHover(): void {
    hoveredDesignElement?.removeAttribute('data-bitfun-canvas-hovered');
    hoveredDesignElement = null;
  }

  function setDesignHover(element: Element | null): void {
    if (hoveredDesignElement === element) return;
    clearDesignHover();
    hoveredDesignElement = element;
    hoveredDesignElement?.setAttribute('data-bitfun-canvas-hovered', 'true');
  }

  function setDesignMode(enabled: boolean): void {
    designModeEnabled = enabled;
    document.documentElement.toggleAttribute('data-bitfun-canvas-design-mode', enabled);
    if (!enabled) clearDesignHover();
  }

  function handleDesignPointerMove(event: PointerEvent): void {
    if (!designModeEnabled) return;
    const target = isSelectableCanvasElement(event.target) ? event.target : null;
    setDesignHover(target);
  }

  function handleDesignPointerLeave(): void {
    if (designModeEnabled) clearDesignHover();
  }

  function handleDesignClick(event: MouseEvent): void {
    if (!designModeEnabled || !isSelectableCanvasElement(event.target)) return;
    event.preventDefault();
    event.stopPropagation();
    window.parent?.postMessage({
      type: 'bitfun-canvas-element-selected',
      sourceRevisionSeen: sourceRevision,
      reference: elementReference(event.target),
    }, '*');
    setDesignMode(false);
  }

  function ErrorPanel({ error }: CanvasRuntimeRecord = {}) {
    return React.createElement('main', { style: { maxWidth: 860, margin: '0 auto', padding: 12, border: '1px solid var(--border-base)', borderRadius: 8 } }, [
      React.createElement('h1', { key: 'title', style: { fontSize: 18, margin: '0 0 8px' } }, 'Canvas runtime error'),
      React.createElement('pre', { key: 'error', style: { whiteSpace: 'pre-wrap', color: 'var(--bitfun-canvas-danger)' } }, errorText(error)),
    ]);
  }

  class RuntimeErrorBoundary extends React.Component {
    constructor(props: any) {
      super(props);
      this.state = { error: null };
    }

    static getDerivedStateFromError(error: unknown) {
      return { error };
    }

    componentDidCatch(error: unknown) {
      postRuntimeError(error);
    }

    render() {
      if (this.state.error) return React.createElement(ErrorPanel, { error: this.state.error });
      return this.props.children;
    }
  }

  function RuntimeRoot() {
    React.useEffect(() => {
      postReady();
      const timeout = window.setTimeout(postReady, 0);
      return () => window.clearTimeout(timeout);
    }, []);
    return React.createElement(RuntimeErrorBoundary, null, renderComponent ? React.createElement(renderComponent) : null);
  }

  function rerender(): void {
    if (!renderComponent || !rootElement) return;
    try {
      if (!reactRoot) reactRoot = ReactDOM.createRoot(rootElement);
      reactRoot.render(React.createElement(RuntimeRoot));
    } catch (error) {
      reportRuntimeError(error);
    }
  }

  function reportRuntimeError(error: unknown): void {
    if (rootElement) {
      rootElement.innerHTML =
        '<main style="max-width:860px;margin:0 auto;padding:12px;border:1px solid var(--border-base);border-radius:8px"><h1 style="font-size:18px;margin:0 0 8px">Canvas runtime error</h1><pre style="white-space:pre-wrap;color:var(--bitfun-canvas-danger)"></pre></main>';
      const pre = rootElement.querySelector('pre');
      if (pre) pre.textContent = errorText(error);
    }
    postRuntimeError(error);
  }

  window.addEventListener('message', event => {
    const data = event.data;
    if (!data || typeof data !== 'object') return;
    if (data.type === 'bitfun-canvas-theme') {
      applyTheme(data.theme);
      stateListeners.forEach(listener => listener());
    } else if (data.type === 'bitfun-canvas-design-mode') {
      setDesignMode(Boolean(data.enabled));
    } else if (
      data.type === 'bitfun-canvas-state' ||
      data.type === 'bitfun-canvas-load-state-result' ||
      data.type === 'bitfun-canvas-save-state-result'
    ) {
      if (data.state && typeof data.state === 'object' && data.state.values && typeof data.state.values === 'object') {
        hostStateValues = { ...data.state.values };
        if (data.state.sourceRevisionSeen) sourceRevision = data.state.sourceRevisionSeen;
        stateListeners.forEach(listener => listener());
      }
    } else if (data.type === 'bitfun-canvas-action-result' || data.type === 'bitfun-canvas-error') {
      const request = data.requestId ? pendingRequests.get(data.requestId) : null;
      if (!request) return;
      pendingRequests.delete(data.requestId);
      if (data.error) request.reject(new Error(String(data.error)));
      else request.resolve(data.result);
    }
  });

  window.addEventListener('error', event => reportRuntimeError(event.error || event.message || 'Canvas runtime error'));
  window.addEventListener('unhandledrejection', event => reportRuntimeError(event.reason || 'Canvas runtime promise rejection'));
  document.addEventListener('pointermove', handleDesignPointerMove, true);
  document.addEventListener('pointerleave', handleDesignPointerLeave, true);
  document.addEventListener('click', handleDesignClick, true);

  function installSdkAdapters(): void {
    if (!runtimeWindow.BitfunCanvasSDK || !runtimeWindow.BitfunCanvasSDKAdapters) return;
    runtimeWindow.BitfunCanvasSDK = {
      ...runtimeWindow.BitfunCanvasSDK,
      ...runtimeWindow.BitfunCanvasSDKAdapters,
    };
  }

  runtimeWindow.BitfunCanvasRuntimeHooks = {
    useHostTheme,
    useCanvasState<T>(key: string, defaultValue: T) {
      return useCanvasState(key, defaultValue) as [T, (value: T | ((previous: T) => T)) => void];
    },
    useCanvasAction,
    useState: React.useState,
    useRef: React.useRef,
    useEffect: React.useEffect,
    useCallback: React.useCallback,
    useMemo: React.useMemo,
  };

  runtimeWindow.BitfunCanvasSDK = {
    ...runtimeWindow.BitfunCanvasRuntimeHooks,
  };

  runtimeWindow.BitfunCanvasRuntime = {
    h: React.createElement,
    Fragment: React.Fragment,
    moduleStarted() {
      installSdkAdapters();
      window.parent?.postMessage({ type: 'bitfun-canvas-module-started', sourceRevisionSeen: sourceRevision }, '*');
    },
    reportRuntimeError,
    mount(component: any) {
      renderComponent = component;
      rerender();
      postReady();
    },
  };
}
