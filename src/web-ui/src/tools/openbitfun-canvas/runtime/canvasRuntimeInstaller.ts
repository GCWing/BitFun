type CanvasRuntimeRecord = Record<string, any>;
type RuntimeReact = any;
type RuntimeReactDOM = any;
type CanvasStateRuntimeOptions<T> = {
  version?: number;
  validate?: (value: unknown) => value is T;
  migrate?: (value: unknown, fromVersion: number) => T;
};
type RuntimeWindow = Window & {
  React?: unknown;
  ReactDOM?: unknown;
  OpenBitFunCanvasSDK?: CanvasRuntimeRecord;
  OpenBitFunCanvasSDKAdapters?: CanvasRuntimeRecord;
  OpenBitFunCanvasRuntimeHooks?: Record<string, unknown>;
  OpenBitFunCanvasRuntime?: CanvasRuntimeRecord;
};

export function buildCanvasRuntimeInstallerScript(revision: string): string {
  return `(${installOpenBitFunCanvasRuntime.toString()})(${JSON.stringify(revision)});`;
}

function installOpenBitFunCanvasRuntime(initialRevision: string): void {
  const runtimeWindow = window as unknown as RuntimeWindow;
  const React = runtimeWindow.React as RuntimeReact | undefined;
  const ReactDOM = runtimeWindow.ReactDOM as RuntimeReactDOM | undefined;
  const rootElement = document.getElementById('openbitfun-canvas-root');
  let reactRoot: ReturnType<RuntimeReactDOM['createRoot']> | null = null;
  let renderComponent: any = null;
  let hostAppearance = makeAppearance({
    type: 'auto',
    bg: 'var(--openbitfun-canvas-bg)',
    panel: 'var(--openbitfun-canvas-panel)',
    fg: 'var(--openbitfun-canvas-fg)',
    muted: 'var(--openbitfun-canvas-muted)',
    border: 'var(--openbitfun-canvas-border)',
    accent: 'var(--openbitfun-canvas-accent)',
    success: 'var(--openbitfun-canvas-success)',
    warning: 'var(--openbitfun-canvas-warning)',
    danger: 'var(--openbitfun-canvas-danger)',
    info: 'var(--openbitfun-canvas-info)',
  });
  const sourceRevision = initialRevision;
  let hostStateValues: CanvasRuntimeRecord = {};
  let hostStateVersions: Record<string, number> = {};
  let stateHydrated = false;
  const stateReadyCallbacks = new Set<() => void>();
  const rejectedStateKeys = new Set<string>();
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

  function makeAppearance(tokens: CanvasRuntimeRecord): CanvasRuntimeRecord {
    const readToken = (value: unknown, fallback: string): string =>
      value === undefined || value === null || value === '' ? fallback : String(value);
    const bg = readToken(tokens.bg, 'var(--openbitfun-canvas-bg)');
    const panel = readToken(tokens.panel, 'var(--openbitfun-canvas-panel)');
    const fg = readToken(tokens.fg, 'var(--openbitfun-canvas-fg)');
    const muted = readToken(tokens.muted, 'var(--openbitfun-canvas-muted)');
    const border = readToken(tokens.border, 'var(--openbitfun-canvas-border)');
    const accent = readToken(tokens.accent, 'var(--openbitfun-canvas-accent)');
    const success = readToken(tokens.success, 'var(--openbitfun-canvas-success)');
    const warning = readToken(tokens.warning, 'var(--openbitfun-canvas-warning)');
    const danger = readToken(tokens.danger, 'var(--openbitfun-canvas-danger)');
    const info = readToken(tokens.info, 'var(--openbitfun-canvas-info)');
    const appearanceVars = tokens.vars && typeof tokens.vars === 'object' ? tokens.vars : {};
    const codeChangeAdded = readToken(
      appearanceVars['--openbitfun-color-code-change-added'],
      'var(--openbitfun-color-code-change-added)',
    );
    const codeChangeRemoved = readToken(
      appearanceVars['--openbitfun-color-code-change-removed'],
      'var(--openbitfun-color-code-change-removed)',
    );
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
      chrome: 'var(--openbitfun-color-surface-subtle)',
      elevated: panel,
    };
    const semanticText = {
      primary: fg,
      secondary: muted,
      tertiary: muted,
      quaternary: muted,
      link: accent,
      onAccent: 'var(--openbitfun-color-content-on-dark)',
    };
    const semanticFill = {
      primary: panel,
      secondary: 'var(--openbitfun-color-action-neutral-surface)',
      tertiary: 'var(--openbitfun-color-action-quiet-hover)',
      quaternary: 'var(--openbitfun-color-surface-subtle)',
    };
    const semanticStroke = {
      primary: border,
      secondary: 'var(--openbitfun-color-border-default)',
      tertiary: 'var(--openbitfun-color-border-subtle)',
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
      insertedLine: `color-mix(in srgb, ${codeChangeAdded} 12%, transparent)`,
      removedLine: `color-mix(in srgb, ${codeChangeRemoved} 12%, transparent)`,
      stripAdded: codeChangeAdded,
      stripRemoved: codeChangeRemoved,
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

  function applyAppearance(nextAppearance: CanvasRuntimeRecord): void {
    if (!nextAppearance || typeof nextAppearance !== 'object') return;
    const allowed = ['bg', 'panel', 'fg', 'muted', 'border', 'accent', 'success', 'warning', 'danger', 'info'];
    const rootStyle = document.documentElement.style;
    if (nextAppearance.vars && typeof nextAppearance.vars === 'object') {
      for (const [name, value] of Object.entries(nextAppearance.vars)) {
        if (/^--[a-zA-Z0-9_-]+$/.test(name) && typeof value === 'string' && value.trim()) {
          rootStyle.setProperty(name, value.trim());
        }
      }
    }
    for (const key of allowed) {
      const value = nextAppearance[key];
      if (typeof value === 'string' && value.trim()) {
        rootStyle.setProperty(`--openbitfun-canvas-${key}`, value.trim());
      }
    }
    if (nextAppearance.type === 'dark' || nextAppearance.type === 'light') {
      document.documentElement.style.colorScheme = nextAppearance.type;
      document.querySelectorAll('[data-openbitfun-design-system-root]').forEach(element => {
        element.setAttribute('data-color-scheme', nextAppearance.type);
      });
    }
    hostAppearance = makeAppearance({ ...hostAppearance, ...nextAppearance });
    rerender();
  }

  function markStateReady(): void {
    if (stateHydrated) return;
    stateHydrated = true;
    stateReadyCallbacks.forEach(callback => callback());
    stateReadyCallbacks.clear();
  }

  function whenStateReady(callback: () => void): () => void {
    if (stateHydrated) {
      callback();
      return () => {};
    }
    stateReadyCallbacks.add(callback);
    return () => stateReadyCallbacks.delete(callback);
  }

  function stateValueMatchesDefault(value: unknown, defaultValue: unknown): boolean {
    if (defaultValue === null || defaultValue === undefined) return true;
    if (Array.isArray(defaultValue)) return Array.isArray(value);
    if (typeof defaultValue === 'object') {
      return Boolean(value && typeof value === 'object' && !Array.isArray(value));
    }
    return typeof value === typeof defaultValue;
  }

  function stateValueForKey<T>(
    key: string,
    defaultValue: T,
    options?: CanvasStateRuntimeOptions<T>,
  ): T {
    if (!Object.prototype.hasOwnProperty.call(hostStateValues, key)) return defaultValue;
    let persistedValue = hostStateValues[key];
    const targetVersion = Math.max(1, Math.floor(options?.version || 1));
    const persistedVersion = Math.max(1, Math.floor(hostStateVersions[key] || 1));
    if (persistedVersion !== targetVersion) {
      if (!options?.migrate) return defaultValue;
      try {
        persistedValue = options.migrate(persistedValue, persistedVersion);
        hostStateValues = { ...hostStateValues, [key]: persistedValue };
        hostStateVersions = { ...hostStateVersions, [key]: targetVersion };
      } catch {
        return defaultValue;
      }
    }
    if (options?.validate && !options.validate(persistedValue)) return defaultValue;
    if (stateValueMatchesDefault(persistedValue, defaultValue)) return persistedValue as T;
    if (!rejectedStateKeys.has(key)) {
      rejectedStateKeys.add(key);
      window.parent?.postMessage({
        type: 'openbitfun-canvas-state-warning',
        sourceRevisionSeen: sourceRevision,
        key,
        message: `Ignored incompatible persisted Canvas state for key "${key}"`,
      }, '*');
    }
    return defaultValue;
  }

  function useHostAppearance(): CanvasRuntimeRecord {
    const [, force] = React.useState(0);
    React.useEffect(() => {
      const listener = () => force((value: number) => value + 1);
      stateListeners.add(listener);
      return () => {
        stateListeners.delete(listener);
      };
    }, []);
    return hostAppearance;
  }

  function useCanvasState<T>(key: string, defaultValue: T, options?: CanvasStateRuntimeOptions<T>): [T, (nextValue: T | ((value: T) => T)) => void] {
    const initialValue = stateValueForKey(key, defaultValue, options);
    const [value, setValue] = React.useState(initialValue);
    React.useEffect(() => {
      const listener = () => {
        setValue(stateValueForKey(key, defaultValue, options));
      };
      stateListeners.add(listener);
      return () => {
        stateListeners.delete(listener);
      };
    }, [key, defaultValue, options]);
    const update = React.useCallback(
      (nextValue: T | ((value: T) => T)) => {
        const currentValue = stateValueForKey(key, defaultValue, options);
        const resolved =
          typeof nextValue === 'function'
            ? (nextValue as (value: T) => T)(currentValue)
            : nextValue;
        hostStateValues = { ...hostStateValues, [key]: resolved };
        hostStateVersions = {
          ...hostStateVersions,
          [key]: Math.max(1, Math.floor(options?.version || 1)),
        };
        setValue(resolved);
        window.parent?.postMessage(
          {
            type: 'openbitfun-canvas-save-state',
            sourceRevisionSeen: sourceRevision,
            values: hostStateValues,
            valueVersions: hostStateVersions,
            updatedAt: Date.now(),
          },
          '*',
        );
      },
      [key, defaultValue, options],
    );
    return [value, update];
  }

  function useCanvasAction(): (action: unknown) => Promise<unknown> {
    return React.useCallback(
      (action: unknown) =>
        new Promise((resolve, reject) => {
          const requestId = `canvas-action-${++requestSeq}`;
          pendingRequests.set(requestId, { resolve, reject });
          window.parent?.postMessage({ type: 'openbitfun-canvas-action', requestId, action }, '*');
        }),
      [],
    );
  }

  function errorText(error: any): string {
    return String(error?.stack || error?.message || error);
  }

  function postReady(): void {
    window.parent?.postMessage({ type: 'openbitfun-canvas-ready', sourceRevisionSeen: sourceRevision }, '*');
  }

  function postRuntimeError(error: any): void {
    window.parent?.postMessage({
      type: 'openbitfun-canvas-runtime-error',
      sourceRevisionSeen: sourceRevision,
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
    const className = Array.from(element.classList).find(name => name.startsWith('openbitfun-') || name.startsWith('openbitfun-'));
    if (!className) return undefined;
    return className
      .replace(/^openbitfun-/, '')
      .replace(/^openbitfun-canvas-/, '')
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
    hoveredDesignElement?.removeAttribute('data-openbitfun-canvas-hovered');
    hoveredDesignElement = null;
  }

  function setDesignHover(element: Element | null): void {
    if (hoveredDesignElement === element) return;
    clearDesignHover();
    hoveredDesignElement = element;
    hoveredDesignElement?.setAttribute('data-openbitfun-canvas-hovered', 'true');
  }

  function setDesignMode(enabled: boolean): void {
    designModeEnabled = enabled;
    document.documentElement.toggleAttribute('data-openbitfun-canvas-design-mode', enabled);
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
      type: 'openbitfun-canvas-element-selected',
      sourceRevisionSeen: sourceRevision,
      reference: elementReference(event.target),
    }, '*');
    setDesignMode(false);
  }

  function ErrorPanel({ error }: CanvasRuntimeRecord = {}) {
    return React.createElement('main', { style: { maxWidth: 860, margin: '0 auto', padding: 12, border: '1px solid var(--openbitfun-color-border-default)', borderRadius: 8 } }, [
      React.createElement('h1', {
        key: 'title',
        style: {
          fontSize: 'var(--openbitfun-type-heading-page-font-size)',
          fontWeight: 'var(--openbitfun-type-heading-page-font-weight)',
          lineHeight: 'var(--openbitfun-type-heading-page-line-height)',
          margin: '0 0 8px',
        },
      }, 'Canvas runtime error'),
      React.createElement('pre', { key: 'error', style: { whiteSpace: 'pre-wrap', color: 'var(--openbitfun-canvas-danger)' } }, errorText(error)),
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
        '<main style="max-width:860px;margin:0 auto;padding:12px;border:1px solid var(--openbitfun-color-border-default);border-radius:8px"><h1 style="font-size:var(--openbitfun-type-heading-page-font-size);font-weight:var(--openbitfun-type-heading-page-font-weight);line-height:var(--openbitfun-type-heading-page-line-height);margin:0 0 8px">Canvas runtime error</h1><pre style="white-space:pre-wrap;color:var(--openbitfun-canvas-danger)"></pre></main>';
      const pre = rootElement.querySelector('pre');
      if (pre) pre.textContent = errorText(error);
    }
    postRuntimeError(error);
  }

  window.addEventListener('message', event => {
    const data = event.data;
    if (!data || typeof data !== 'object') return;
    if (data.type === 'openbitfun-canvas-appearance') {
      applyAppearance(data.appearance);
      stateListeners.forEach(listener => listener());
    } else if (data.type === 'openbitfun-canvas-design-mode') {
      setDesignMode(Boolean(data.enabled));
    } else if (
      data.type === 'openbitfun-canvas-state' ||
      data.type === 'openbitfun-canvas-load-state-result' ||
      data.type === 'openbitfun-canvas-save-state-result'
    ) {
      if (data.state && typeof data.state === 'object' && data.state.values && typeof data.state.values === 'object') {
        hostStateValues = { ...data.state.values };
        hostStateVersions = data.state.valueVersions && typeof data.state.valueVersions === 'object'
          ? { ...data.state.valueVersions }
          : {};
        stateListeners.forEach(listener => listener());
      }
      markStateReady();
    } else if (data.type === 'openbitfun-canvas-action-result' || data.type === 'openbitfun-canvas-error') {
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
    if (!runtimeWindow.OpenBitFunCanvasSDK || !runtimeWindow.OpenBitFunCanvasSDKAdapters) return;
    runtimeWindow.OpenBitFunCanvasSDK = {
      ...runtimeWindow.OpenBitFunCanvasSDK,
      ...runtimeWindow.OpenBitFunCanvasSDKAdapters,
    };
  }

  runtimeWindow.OpenBitFunCanvasRuntimeHooks = {
    useHostAppearance,
    useCanvasState<T>(key: string, defaultValue: T, options?: CanvasStateRuntimeOptions<T>) {
      return useCanvasState(key, defaultValue, options);
    },
    useCanvasAction,
    useState: React.useState,
    useRef: React.useRef,
    useEffect: React.useEffect,
    useCallback: React.useCallback,
    useMemo: React.useMemo,
    whenStateReady,
  };

  runtimeWindow.OpenBitFunCanvasSDK = {
    ...runtimeWindow.OpenBitFunCanvasRuntimeHooks,
  };

  runtimeWindow.OpenBitFunCanvasRuntime = {
    h: React.createElement,
    Fragment: React.Fragment,
    moduleStarted() {
      installSdkAdapters();
      window.parent?.postMessage({ type: 'openbitfun-canvas-module-started', sourceRevisionSeen: sourceRevision }, '*');
    },
    reportRuntimeError,
    mount(component: any) {
      renderComponent = component;
      rerender();
      postReady();
    },
  };

  const stateRequestId = `canvas-state-${++requestSeq}`;
  window.parent?.postMessage({
    type: 'openbitfun-canvas-load-state',
    requestId: stateRequestId,
    sourceRevisionSeen: sourceRevision,
  }, '*');
  window.setTimeout(markStateReady, 400);
}
