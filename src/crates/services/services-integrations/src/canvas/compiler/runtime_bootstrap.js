(() => {
  const root = document.getElementById('bitfun-canvas-root');
  let theme = makeTheme({
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
  function makeTheme(tokens) {
    const readToken = (value, fallback) => value === undefined || value === null || value === '' ? fallback : String(value);
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
    const token = (value, fields = {}) => Object.assign(new String(value), {
      toString() { return value; },
      valueOf() { return value; },
      ...fields,
    });
    return {
      ...tokens,
      bg: token(bg, {
        canvas: bg,
        elevated: panel,
        editor: '#ffffff',
        chrome: 'rgba(127,127,127,0.04)',
      }),
      panel,
      fg,
      muted,
      border,
      accent: token(accent, {
        primary: accent,
        success,
        warning,
        danger,
        info,
      }),
      success,
      warning,
      danger,
      info,
      text: {
        primary: fg,
        secondary: muted,
        tertiary: muted,
        quaternary: 'rgba(102,112,133,0.72)',
        onAccent: '#ffffff',
      },
      fill: {
        primary: panel,
        secondary: 'rgba(127,127,127,0.10)',
        tertiary: 'rgba(127,127,127,0.06)',
        quaternary: 'rgba(127,127,127,0.04)',
      },
      stroke: {
        primary: border,
        secondary: 'rgba(127,127,127,0.18)',
        tertiary: 'rgba(127,127,127,0.10)',
      },
      category: {
        gray: '#7a8087',
        purple: '#8b5cf6',
        green: '#49a66a',
        yellow: '#d88938',
        cyan: '#49a8bd',
        pink: '#c774a7',
        blue: '#2f8ac4',
        orange: '#d88938',
      },
      status: { success, warning, danger, info },
    };
  }
  function applyHostTheme(nextTheme) {
    if (!nextTheme || typeof nextTheme !== 'object') return;
    const allowed = ['bg', 'panel', 'fg', 'muted', 'border', 'accent', 'success', 'warning', 'danger', 'info'];
    const rootStyle = document.documentElement.style;
    for (const key of allowed) {
      const value = nextTheme[key];
      if (typeof value === 'string' && value.trim()) {
        rootStyle.setProperty(`--bitfun-canvas-${key}`, value.trim());
      }
    }
    const type = nextTheme.type === 'dark' || nextTheme.type === 'light' ? nextTheme.type : 'auto';
    document.documentElement.style.colorScheme = type === 'auto' ? 'light dark' : type;
    theme = makeTheme({
      ...theme,
      ...Object.fromEntries(allowed.map(key => [key, getComputedStyle(document.documentElement).getPropertyValue(`--bitfun-canvas-${key}`).trim() || theme[key]])),
      type,
    });
  }
  const state = new Map();
  let hostStateReady = false;
  let readySent = false;
  let renderFn = null;
  let nodeSeq = 0;
  let designMode = false;
  let inspectElement = null;
  const inspectStyleSnapshot = new WeakMap();
  let hookIndex = 0;
  const hookValues = [];
  const hookEffects = [];
  let renderQueued = false;

  function toArray(value) {
    return Array.isArray(value) ? value.flat(Infinity) : [value];
  }
  function applyStyle(node, style) {
    if (style && typeof style === 'object') Object.assign(node.style, style);
  }
  function appendChildren(node, children) {
    for (const child of toArray(children)) {
      if (child === null || child === undefined || child === false) continue;
      if (Array.isArray(child)) {
        appendChildren(node, child);
      } else if (child instanceof Node) {
        node.appendChild(child);
      } else {
        node.appendChild(document.createTextNode(String(child)));
      }
    }
  }
  function el(tag, props = {}, children = []) {
    const node = document.createElement(tag);
    props = props || {};
    for (const [key, value] of Object.entries(props)) {
      if (key === 'children' || key === 'key' || value === undefined || value === null || value === false) continue;
      if (key === 'style') applyStyle(node, value);
      else if (key === 'className') node.className = value;
      else if (key === 'htmlFor') node.htmlFor = String(value);
      else if (key === 'ref' && typeof value === 'function') value(node);
      else if (key === 'ref' && value && typeof value === 'object') value.current = node;
      else if (key.startsWith('on') && typeof value === 'function') node.addEventListener(key.slice(2).toLowerCase(), value);
      else if (key === 'checked' || key === 'selected' || key === 'disabled' || key === 'open') node[key] = Boolean(value);
      else if (key === 'value') node.value = value;
      else node.setAttribute(key, String(value));
    }
    appendChildren(node, children);
    return node;
  }
  const SVG_TAGS = new Set(['svg', 'g', 'defs', 'marker', 'polygon', 'path', 'rect', 'circle', 'ellipse', 'line', 'polyline', 'text', 'tspan']);
  function svgAttrName(key) {
    return key.replace(/[A-Z]/g, match => `-${match.toLowerCase()}`);
  }
  function markCanvasNode(value, component) {
    for (const item of toArray(value)) {
      if (item instanceof Element) {
        if (!item.dataset.bitfunCanvasNode) item.dataset.bitfunCanvasNode = `node-${++nodeSeq}`;
        if (!item.dataset.bitfunCanvasComponent) item.dataset.bitfunCanvasComponent = component;
      }
    }
    return value;
  }
  function cssIdent(value) {
    if (window.CSS && typeof window.CSS.escape === 'function') return window.CSS.escape(value);
    return String(value).replace(/[^a-zA-Z0-9_-]/g, '\\$&');
  }
  function elementSelector(element) {
    if (element.id) return `#${cssIdent(element.id)}`;
    const parts = [];
    let current = element;
    while (current && current instanceof Element && current !== root && current !== document.body && current !== document.documentElement) {
      const tag = current.tagName.toLowerCase();
      const parent = current.parentElement;
      if (!parent) {
        parts.unshift(tag);
        break;
      }
      const siblings = Array.from(parent.children).filter(child => child.tagName === current.tagName);
      const index = siblings.indexOf(current) + 1;
      parts.unshift(siblings.length > 1 ? `${tag}:nth-of-type(${index})` : tag);
      current = parent;
    }
    return parts.join(' > ') || element.tagName.toLowerCase();
  }
  function elementReference(element) {
    const text = (element.innerText || element.textContent || '').replace(/\s+/g, ' ').trim();
    const rect = element.getBoundingClientRect();
    return {
      nodeId: element.dataset.bitfunCanvasNode || null,
      component: element.dataset.bitfunCanvasComponent || element.tagName.toLowerCase(),
      tagName: element.tagName.toLowerCase(),
      selector: elementSelector(element),
      text: text.length > 180 ? `${text.slice(0, 180)}...` : text,
      bounds: {
        x: Math.round(rect.x),
        y: Math.round(rect.y),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
      },
    };
  }
  function inspectableElement(target) {
    if (!(target instanceof Element) || !root || !root.contains(target) || target === root) return null;
    return target.closest('[data-bitfun-canvas-node]') || target;
  }
  function clearInspectHighlight() {
    if (!inspectElement) return;
    const snapshot = inspectStyleSnapshot.get(inspectElement);
    if (snapshot) {
      inspectElement.style.outline = snapshot.outline;
      inspectElement.style.outlineOffset = snapshot.outlineOffset;
      inspectElement.style.cursor = snapshot.cursor;
    }
    inspectElement = null;
  }
  function highlightInspectElement(element) {
    if (inspectElement === element) return;
    clearInspectHighlight();
    inspectElement = element;
    inspectStyleSnapshot.set(element, {
      outline: element.style.outline,
      outlineOffset: element.style.outlineOffset,
      cursor: element.style.cursor,
    });
    element.style.outline = '2px solid var(--bitfun-canvas-accent)';
    element.style.outlineOffset = '2px';
    element.style.cursor = 'crosshair';
  }
  function setDesignMode(enabled) {
    designMode = Boolean(enabled);
    document.body.dataset.bitfunCanvasDesignMode = designMode ? 'true' : 'false';
    document.body.style.cursor = designMode ? 'crosshair' : '';
    if (!designMode) clearInspectHighlight();
  }
  document.addEventListener('mouseover', event => {
    if (!designMode) return;
    const element = inspectableElement(event.target);
    if (element) highlightInspectElement(element);
  }, true);
  document.addEventListener('click', event => {
    if (!designMode) return;
    const element = inspectableElement(event.target);
    if (!element) return;
    event.preventDefault();
    event.stopPropagation();
    highlightInspectElement(element);
    window.parent?.postMessage({
      type: 'bitfun-canvas-element-selected',
      reference: elementReference(element),
    }, '*');
  }, true);
  function h(type, props, ...children) {
    props = props || {};
    props.children = children;
    const result = typeof type === 'function' ? type(props) : SVG_TAGS.has(String(type)) ? svg(String(type), props, children) : el(type, props, children);
    return markCanvasNode(result, typeof type === 'function' ? type.name || 'Component' : String(type));
  }
  function rerender() {
    if (!renderFn || !root) return;
    root.replaceChildren();
    try {
      hookIndex = 0;
      const result = renderFn();
      appendChildren(root, result);
      flushEffects();
      if (!readySent) {
        readySent = true;
        window.parent?.postMessage({ type: 'bitfun-canvas-ready' }, '*');
      }
    } catch (error) {
      reportRuntimeError(error);
    }
  }
  function reportRuntimeError(error) {
    if (root) root.replaceChildren(errorView(error));
    window.parent?.postMessage({
      type: 'bitfun-canvas-runtime-error',
      message: String(error?.message || error),
      name: error?.name ? String(error.name) : undefined,
      stack: error?.stack ? String(error.stack) : undefined,
    }, '*');
  }
  function errorView(error) {
    return el('main', { style: { maxWidth: '860px', margin: '0 auto', padding: '12px', border: '1px solid var(--bitfun-canvas-border)', borderRadius: '8px' } }, [
      el('h1', { style: { margin: '0 0 8px', fontSize: '18px' } }, ['Canvas runtime error']),
      el('pre', { style: { whiteSpace: 'pre-wrap', margin: 0, color: 'var(--bitfun-canvas-danger)' } }, [String(error?.stack || error?.message || error)])
    ]);
  }
  window.addEventListener('error', event => {
    reportRuntimeError(event.error || event.message || 'Canvas runtime error');
  });
  window.addEventListener('unhandledrejection', event => {
    reportRuntimeError(event.reason || 'Canvas runtime promise rejection');
  });
  function component(tag, baseStyle = {}) {
    return ({ children, style, ...props } = {}) => el(tag, { ...props, style: { ...baseStyle, ...style } }, children);
  }
  function spacingValue(value) {
    if (typeof value === 'number') return `${value}px`;
    return value;
  }
  function sizeValue(size) {
    if (typeof size === 'number') return `${size}px`;
    return size === 'small' || size === 'sm' ? '12px' : size === 'lg' ? '16px' : size === 'body' || size === 'md' || !size ? '14px' : size;
  }
  function flexAlign(value) {
    return value === 'start' ? 'flex-start' : value === 'end' ? 'flex-end' : value || 'center';
  }
  function flexJustify(value) {
    return value === 'start' ? 'flex-start' : value === 'end' ? 'flex-end' : value || 'flex-start';
  }
  function spacingStyle(value, property) {
    if (value === undefined || value === null) return {};
    if (typeof value === 'object') {
      const result = {};
      if (value.x !== undefined) {
        result[`${property}Left`] = spacingValue(value.x);
        result[`${property}Right`] = spacingValue(value.x);
      }
      if (value.y !== undefined) {
        result[`${property}Top`] = spacingValue(value.y);
        result[`${property}Bottom`] = spacingValue(value.y);
      }
      for (const key of ['top', 'right', 'bottom', 'left']) {
        if (value[key] !== undefined) result[`${property}${key[0].toUpperCase()}${key.slice(1)}`] = spacingValue(value[key]);
      }
      return result;
    }
    return { [property]: spacingValue(value) };
  }
  function commonStyle(props = {}, style = {}) {
    return {
      ...spacingStyle(props.padding, 'padding'),
      ...spacingStyle(props.margin, 'margin'),
      ...(props.background !== undefined ? { background: props.background } : {}),
      ...(props.border !== undefined ? { border: props.border } : {}),
      ...(props.borderTop !== undefined ? { borderTop: props.borderTop } : {}),
      ...(props.borderRight !== undefined ? { borderRight: props.borderRight } : {}),
      ...(props.borderBottom !== undefined ? { borderBottom: props.borderBottom } : {}),
      ...(props.borderLeft !== undefined ? { borderLeft: props.borderLeft } : {}),
      ...(props.borderRadius !== undefined ? { borderRadius: spacingValue(props.borderRadius) } : {}),
      ...(props.width !== undefined ? { width: spacingValue(props.width) } : {}),
      ...(props.height !== undefined ? { height: spacingValue(props.height) } : {}),
      ...(props.flex !== undefined ? { flex: props.flex } : {}),
      ...(props.display !== undefined ? { display: props.display } : {}),
      ...(props.color !== undefined ? { color: props.color } : {}),
      ...(props.opacity !== undefined ? { opacity: props.opacity } : {}),
      ...style,
    };
  }
  const colorPalette = ['gray', 'purple', 'green', 'yellow', 'cyan', 'pink', 'blue', 'orange'];
  const usageColorSequence = ['gray', 'purple', 'green', 'yellow', 'pink', 'blue', 'orange'];
  const categoryPaletteLight = {
    gray: 'var(--bitfun-canvas-muted)',
    purple: 'var(--bitfun-canvas-accent)',
    green: 'var(--bitfun-canvas-success)',
    yellow: 'var(--bitfun-canvas-warning)',
    cyan: 'var(--bitfun-canvas-info)',
    pink: 'var(--bitfun-canvas-danger)',
    blue: 'var(--bitfun-canvas-accent)',
    orange: 'var(--bitfun-canvas-warning)',
  };
  const categoryPaletteDark = categoryPaletteLight;
  const canvasTokensLight = {
    bg: 'var(--bitfun-canvas-panel)',
    panel: 'var(--bitfun-canvas-bg)',
    elevated: 'var(--bitfun-canvas-bg)',
    chrome: 'var(--bitfun-canvas-panel)',
    text: 'var(--bitfun-canvas-fg)',
    textSecondary: 'var(--bitfun-canvas-muted)',
    textMuted: 'var(--bitfun-canvas-muted)',
    border: 'var(--bitfun-canvas-border)',
    accent: 'var(--bitfun-canvas-accent)',
    success: 'var(--bitfun-canvas-success)',
    warning: 'var(--bitfun-canvas-warning)',
    danger: 'var(--bitfun-canvas-danger)',
    info: 'var(--bitfun-canvas-info)',
  };
  const canvasTokens = canvasTokensLight;
  const canvasPaletteLight = categoryPaletteLight;
  const canvasPaletteDark = categoryPaletteDark;
  function mergeStyle(base = {}, override = {}) {
    return { ...base, ...(override || {}) };
  }
  function categoryColor(color, index = 0) {
    const resolved = color || usageColorSequence[index % usageColorSequence.length] || 'gray';
    return categoryPaletteLight[resolved] || categoryPaletteLight.gray;
  }
  const Stack = ({ children, gap = 12, style, ...props } = {}) => el('div', { style: { display: 'flex', flexDirection: 'column', gap: `${gap}px`, ...commonStyle(props, style) } }, children);
  const Row = ({ children, gap = 8, align = 'center', justify = 'start', wrap = false, style, ...props } = {}) => el('div', { style: { display: 'flex', flexDirection: 'row', gap: `${gap}px`, alignItems: flexAlign(align), justifyContent: flexJustify(justify), flexWrap: wrap ? 'wrap' : 'nowrap', ...commonStyle(props, style) } }, children);
  const Grid = ({ children, columns = 2, gap = 12, align = 'stretch', style, ...props } = {}) => el('div', { style: { display: 'grid', gridTemplateColumns: typeof columns === 'number' ? `repeat(${columns}, minmax(0, 1fr))` : columns, gap: `${gap}px`, alignItems: flexAlign(align), ...commonStyle(props, style) } }, children);
  const Spacer = () => el('div', { style: { flex: '1 1 auto', minWidth: 0, minHeight: 0 } });
  const Box = ({
    children,
    style,
    padding,
    margin,
    background,
    border,
    borderTop,
    borderRight,
    borderBottom,
    borderLeft,
    borderRadius,
    width,
    height,
    flex,
    display,
    ...props
  } = {}) => el('div', {
    ...props,
    style: commonStyle({ padding, margin, background, border, borderTop, borderRight, borderBottom, borderLeft, borderRadius, width, height, flex, display, ...props }, style),
  }, children);
  const Divider = ({ style } = {}) => el('hr', { style: { border: 0, borderTop: '1px solid rgba(127,127,127,0.18)', width: '100%', margin: '4px 0', ...style } });
  const H1 = ({ children, style } = {}) => el('h1', { style: { fontSize: '26px', lineHeight: '1.14', margin: 0, fontWeight: 720, letterSpacing: 0, ...style } }, children);
  const H2 = ({ children, style } = {}) => el('h2', { style: { fontSize: '18px', lineHeight: '1.3', margin: 0, fontWeight: 650, letterSpacing: 0, ...style } }, children);
  const H3 = ({ children, style } = {}) => el('h3', { style: { fontSize: '15px', lineHeight: '1.35', margin: 0, fontWeight: 650, letterSpacing: 0, ...style } }, children);
  const Text = ({ children, tone = 'primary', size = 'body', weight = 'normal', italic = false, as = 'p', truncate = false, style, color, ...props } = {}) => {
    const truncateStyle = truncate ? {
      overflow: 'hidden',
      whiteSpace: 'nowrap',
      textOverflow: 'ellipsis',
      direction: truncate === 'start' ? 'rtl' : undefined,
      textAlign: truncate === 'start' ? 'left' : undefined,
    } : {};
    return el(as, { style: { margin: 0, color: color || toneColor(tone), fontSize: sizeValue(size), fontWeight: weightValue(weight), fontStyle: italic ? 'italic' : undefined, ...truncateStyle, ...commonStyle(props, style) } }, children);
  };
  const Code = component('code', { fontFamily: 'ui-monospace,SFMono-Regular,Menlo,monospace', fontSize: '12px', background: 'rgba(127,127,127,0.12)', borderRadius: '4px', padding: '1px 4px' });
  const Link = ({ children, href, style } = {}) => el('a', { href, target: '_blank', rel: 'noreferrer', style: { color: 'var(--bitfun-canvas-accent)', textDecoration: 'none', ...style } }, children);
  const Card = ({ children, variant = 'default', size = 'base', style, ...props } = {}) => el('section', {
    ...props,
    style: {
      border: variant === 'borderless' ? '0' : '1px solid rgba(127,127,127,0.20)',
      borderRadius: variant === 'borderless' ? 0 : '8px',
      background: variant === 'borderless' ? 'transparent' : 'var(--bitfun-canvas-bg)',
      overflow: 'hidden',
      ...style,
    }
  }, children);
  const CardHeader = ({ children, trailing, style } = {}) => el('header', { style: { minHeight: '34px', display: 'flex', alignItems: 'center', gap: '10px', justifyContent: 'space-between', padding: '9px 12px', borderBottom: '1px solid rgba(127,127,127,0.16)', fontSize: '12px', fontWeight: 650, lineHeight: 1.25, ...style } }, [
    el('div', { style: { minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' } }, children),
    trailing ? el('div', { style: { flexShrink: 0, color: 'var(--bitfun-canvas-muted)' } }, trailing) : null
  ]);
  const CardBody = ({ children, style } = {}) => el('div', { style: { padding: '12px', ...style } }, children);
  const Empty = ({ description = 'No data', children, style } = {}) => el('div', { style: { display: 'grid', placeItems: 'center', gap: '6px', minHeight: '96px', padding: '18px', border: '1px dashed rgba(127,127,127,0.24)', borderRadius: '8px', color: 'var(--bitfun-canvas-muted)', textAlign: 'center', ...style } }, [
    el('div', { style: { fontSize: '13px' } }, [description]),
    children || null,
  ]);
  const Tabs = ({ items = [], children, activeKey, defaultActiveKey, onChange, style } = {}) => {
    const list = Array.isArray(items) ? items : [];
    const selectedKey = activeKey ?? defaultActiveKey ?? list[0]?.key;
    const selected = list.find(item => item.key === selectedKey) ?? list[0];
    return el('div', { style: { display: 'grid', gap: '10px', ...style } }, [
      list.length ? el('div', { role: 'tablist', style: { display: 'flex', gap: '6px', borderBottom: '1px solid rgba(127,127,127,0.16)' } }, list.map(item =>
        el('button', { role: 'tab', 'aria-selected': item.key === selected?.key, disabled: item.disabled, onClick: () => onChange?.(item.key), style: { border: 0, borderBottom: item.key === selected?.key ? '2px solid var(--bitfun-canvas-accent)' : '2px solid transparent', background: 'transparent', color: item.key === selected?.key ? 'var(--bitfun-canvas-fg)' : 'var(--bitfun-canvas-muted)', padding: '6px 8px', font: 'inherit', cursor: item.disabled ? 'default' : 'pointer' } }, [item.label])
      )) : null,
      selected ? el('div', { role: 'tabpanel' }, selected.children) : children,
    ]);
  };
  function alertTone(type, tone) {
    if (tone) return tone;
    if (type === 'error') return 'danger';
    return type || 'info';
  }
  function alertIcon(type) {
    if (type === 'success') return '✓';
    if (type === 'warning' || type === 'error') return '!';
    return 'i';
  }
  const Alert = ({ children, type = 'info', tone, title, message, description, showIcon = true, style } = {}) => {
    const color = toneColor(alertTone(type, tone));
    return el('div', { role: 'alert', 'aria-live': type === 'error' ? 'assertive' : 'polite', style: { display: 'grid', gridTemplateColumns: showIcon ? '18px minmax(0, 1fr)' : 'minmax(0, 1fr)', gap: '9px', border: '1px solid rgba(127,127,127,0.20)', borderLeft: `3px solid ${color}`, borderRadius: '8px', padding: '10px 12px', background: 'rgba(127,127,127,0.04)', ...style } }, [
      showIcon ? el('span', { 'aria-hidden': true, style: { display: 'grid', placeItems: 'center', width: 18, height: 18, borderRadius: 999, color, fontSize: '11px', fontWeight: 700 } }, [alertIcon(type)]) : null,
      el('span', { style: { minWidth: 0, display: 'grid', gap: '3px' } }, [
        title ? el('strong', { style: { color: 'var(--bitfun-canvas-fg)', fontSize: '13px', lineHeight: 1.35 } }, [title]) : null,
        message || children ? el('span', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px', overflowWrap: 'anywhere' } }, [message ?? children]) : null,
        description ? el('span', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px', overflowWrap: 'anywhere' } }, [description]) : null,
      ]),
    ]);
  };
  const Callout = ({ children, tone = 'info', title, style } = {}) => el('section', { style: { border: '1px solid rgba(127,127,127,0.20)', borderLeft: `3px solid ${toneColor(tone)}`, borderRadius: '6px', padding: '10px', background: 'rgba(127,127,127,0.04)', ...style } }, [title ? el('div', { style: { fontWeight: 650, marginBottom: '4px', fontSize: '13px' } }, [title]) : null, children]);
  const Pill = ({ children, active = false, size = 'md', leadingContent, keyboardHint, disabled, title, onClick, style } = {}) => {
    const isButton = typeof onClick === 'function';
    const tag = isButton ? 'button' : 'span';
    const compact = size === 'sm';
    return el(tag, { title, disabled, onClick, style: { display: 'inline-flex', alignItems: 'center', gap: '5px', border: compact ? '0' : '1px solid rgba(127,127,127,0.22)', borderRadius: '999px', padding: compact ? '1px 6px' : '2px 8px', background: active ? 'rgba(52,120,246,0.16)' : 'rgba(127,127,127,0.05)', color: 'var(--bitfun-canvas-fg)', font: 'inherit', fontSize: compact ? '11px' : '12px', lineHeight: '18px', cursor: isButton && !disabled ? 'pointer' : 'default', opacity: disabled ? 0.55 : 1, ...style } }, [leadingContent, children, keyboardHint ? el('span', { style: { color: 'var(--bitfun-canvas-muted)', marginLeft: 2 } }, [keyboardHint]) : null]);
  };
  function Chevron({ expanded } = {}) {
    return svg('svg', { width: 12, height: 12, viewBox: '0 0 12 12', fill: 'none', style: { transform: expanded ? 'rotate(90deg)' : 'rotate(0deg)', transition: 'transform 120ms ease', flexShrink: 0 } }, [
      svg('path', { d: 'M4.5 2.5L8 6l-3.5 3.5', stroke: 'currentColor', 'stroke-width': 1.4, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' })
    ]);
  }
  function CollapsibleSection({ title, leading, count, trailing, children, defaultOpen = false, style } = {}) {
    const key = `collapsible:${title || ''}`;
    const [open, setOpen] = useCanvasState(key, Boolean(defaultOpen));
    return el('section', { style: { ...style } }, [
      el('button', { onClick: () => setOpen(!open), style: { width: '100%', minHeight: '28px', display: 'flex', alignItems: 'center', gap: '7px', border: 0, padding: '4px 0', background: 'transparent', color: 'var(--bitfun-canvas-fg)', font: 'inherit', cursor: 'pointer', textAlign: 'left' } }, [
        Chevron({ expanded: open }),
        leading || null,
        el('span', { style: { fontSize: '13px', fontWeight: 650, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' } }, [title]),
        count !== undefined ? el('span', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, [String(count)]) : null,
        el('span', { style: { flex: 1 } }),
        trailing ? el('span', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px', flexShrink: 0 } }, trailing) : null,
      ]),
      open ? el('div', { style: { marginLeft: '18px', paddingTop: '6px', paddingBottom: '4px' } }, children) : null,
    ]);
  }
  function normalizeDAGEdges(edges) {
    if (!Array.isArray(edges)) return [];
    return edges
      .map(edge => ({ ...edge, from: edge.from ?? edge.source, to: edge.to ?? edge.target }))
      .filter(edge => edge.from !== undefined && edge.to !== undefined);
  }
  function layoutEdgePath(edge, direction) {
    if (direction === 'horizontal') {
      const midX = edge.sourceX + (edge.targetX - edge.sourceX) / 2;
      return `M ${edge.sourceX} ${edge.sourceY} C ${midX} ${edge.sourceY}, ${midX} ${edge.targetY}, ${edge.targetX} ${edge.targetY}`;
    }
    const midY = edge.sourceY + (edge.targetY - edge.sourceY) / 2;
    return `M ${edge.sourceX} ${edge.sourceY} C ${edge.sourceX} ${midY}, ${edge.targetX} ${midY}, ${edge.targetX} ${edge.targetY}`;
  }
  function computeDAGLayout(options = {}) {
    const nodes = Array.isArray(options.nodes) ? options.nodes : [];
    const edges = normalizeDAGEdges(options.edges);
    const direction = options.direction === 'horizontal' ? 'horizontal' : 'vertical';
    const nodeWidth = Number(options.nodeWidth) || 160;
    const nodeHeight = Number(options.nodeHeight) || 40;
    const rankGap = Number(options.rankGap) || 64;
    const nodeGap = Number(options.nodeGap) || 48;
    const padding = Number(options.padding) || 24;
    const nodeMetaById = new Map(nodes.map(node => [String(node.id), node]));
    const ids = nodes.map(node => String(node.id));
    const idSet = new Set(ids);
    const outgoing = new Map(ids.map(id => [id, []]));
    const incoming = new Map(ids.map(id => [id, []]));
    for (const edge of edges) {
      const from = String(edge.from);
      const to = String(edge.to);
      if (!idSet.has(from) || !idSet.has(to)) continue;
      outgoing.get(from).push(to);
      incoming.get(to).push(from);
    }
    const backEdges = new Set();
    const visiting = new Set();
    const visited = new Set();
    function visit(id) {
      if (visiting.has(id)) return;
      if (visited.has(id)) return;
      visiting.add(id);
      for (const next of outgoing.get(id) || []) {
        const key = `${id}\u0000${next}`;
        if (visiting.has(next)) {
          backEdges.add(key);
          continue;
        }
        visit(next);
      }
      visiting.delete(id);
      visited.add(id);
    }
    ids.forEach(visit);
    const rank = new Map(ids.map(id => [id, 0]));
    for (let pass = 0; pass < ids.length; pass++) {
      let changed = false;
      for (const edge of edges) {
        const from = String(edge.from);
        const to = String(edge.to);
        if (!idSet.has(from) || !idSet.has(to) || backEdges.has(`${from}\u0000${to}`)) continue;
        const nextRank = (rank.get(from) || 0) + 1;
        if (nextRank > (rank.get(to) || 0)) {
          rank.set(to, nextRank);
          changed = true;
        }
      }
      if (!changed) break;
    }
    const grouped = new Map();
    ids.forEach(id => {
      const value = rank.get(id) || 0;
      if (!grouped.has(value)) grouped.set(value, []);
      grouped.get(value).push(id);
    });
    const rankKeys = Array.from(grouped.keys()).sort((a, b) => a - b);
    const maxRankWidth = Math.max(0, ...rankKeys.map(key => grouped.get(key).length * nodeWidth + Math.max(0, grouped.get(key).length - 1) * nodeGap));
    const maxRankHeight = Math.max(0, ...rankKeys.map(key => grouped.get(key).length * nodeHeight + Math.max(0, grouped.get(key).length - 1) * nodeGap));
    const positioned = [];
    const ranks = [];
    rankKeys.forEach((rankKey, rankIndex) => {
      const rankIds = grouped.get(rankKey);
      const rankWidth = direction === 'vertical' ? rankIds.length * nodeWidth + Math.max(0, rankIds.length - 1) * nodeGap : nodeWidth;
      const rankHeight = direction === 'vertical' ? nodeHeight : rankIds.length * nodeHeight + Math.max(0, rankIds.length - 1) * nodeGap;
      const rankX = direction === 'vertical' ? padding + Math.max(0, (maxRankWidth - rankWidth) / 2) : padding + rankIndex * (nodeWidth + rankGap);
      const rankY = direction === 'vertical' ? padding + rankIndex * (nodeHeight + rankGap) : padding + Math.max(0, (maxRankHeight - rankHeight) / 2);
      ranks.push({ rank: rankKey, x: rankX, y: rankY, width: rankWidth, height: rankHeight, nodeIds: rankIds.slice() });
      rankIds.forEach((id, order) => {
        const meta = nodeMetaById.get(id) || {};
        const x = direction === 'vertical' ? rankX + order * (nodeWidth + nodeGap) : rankX;
        const y = direction === 'vertical' ? rankY : rankY + order * (nodeHeight + nodeGap);
        positioned.push({
          ...meta,
          id,
          meta,
          source: meta,
          x,
          y,
          centerX: x + nodeWidth / 2,
          centerY: y + nodeHeight / 2,
          width: nodeWidth,
          height: nodeHeight,
          rank: rankKey,
          order,
        });
      });
    });
    const posMap = new Map(positioned.map(node => [node.id, node]));
    ranks.forEach(rankItem => {
      const rankNodes = positioned.filter(node => node.rank === rankItem.rank);
      rankItem.nodeIds = rankNodes.map(node => node.id);
      rankItem.nodes = rankNodes;
    });
    const layoutEdges = edges.flatMap(edge => {
      const from = String(edge.from);
      const to = String(edge.to);
      const source = posMap.get(from);
      const target = posMap.get(to);
      if (!source || !target) return [];
      const isBackEdge = backEdges.has(`${from}\u0000${to}`) || (rank.get(to) || 0) <= (rank.get(from) || 0);
      if (direction === 'vertical') {
        const layoutEdge = { ...edge, from, to, sourceX: source.x + nodeWidth / 2, sourceY: source.y + nodeHeight, targetX: target.x + nodeWidth / 2, targetY: target.y, isBackEdge };
        return [{ ...layoutEdge, path: layoutEdgePath(layoutEdge, direction) }];
      }
      const layoutEdge = { ...edge, from, to, sourceX: source.x + nodeWidth, sourceY: source.y + nodeHeight / 2, targetX: target.x, targetY: target.y + nodeHeight / 2, isBackEdge };
      return [{ ...layoutEdge, path: layoutEdgePath(layoutEdge, direction) }];
    });
    const width = direction === 'vertical' ? padding * 2 + maxRankWidth : padding * 2 + rankKeys.length * nodeWidth + Math.max(0, rankKeys.length - 1) * rankGap;
    const height = direction === 'vertical' ? padding * 2 + rankKeys.length * nodeHeight + Math.max(0, rankKeys.length - 1) * rankGap : padding * 2 + maxRankHeight;
    return withLayoutNodeArrayCompat({ nodes: positioned, edges: layoutEdges, ranks, direction, width, height });
  }
  function withLayoutNodeArrayCompat(layout) {
    layout[Symbol.iterator] = () => layout.nodes[Symbol.iterator]();
    layout.find = layout.nodes.find.bind(layout.nodes);
    layout.filter = layout.nodes.filter.bind(layout.nodes);
    layout.forEach = layout.nodes.forEach.bind(layout.nodes);
    layout.map = layout.nodes.map.bind(layout.nodes);
    return layout;
  }
  const Stat = ({ value, label, tone, style } = {}) => el('div', { style: { display: 'grid', gap: '2px', ...style } }, [el('strong', { style: { color: toneColor(tone), fontSize: '22px', lineHeight: 1.1, fontVariantNumeric: 'tabular-nums' } }, [value]), el('span', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, [label])]);
  const Table = ({ headers = [], rows = [], columnAlign = [], rowTone = [], framed = true, striped = false, stickyHeader = false, style, emptyMessage = 'No rows' } = {}) => {
    const bodyRows = rows.length ? rows.map((row, rowIndex) => el('tr', { style: { background: striped && rowIndex % 2 === 1 ? 'rgba(127,127,127,0.04)' : 'transparent' } }, headers.map((_, index) => {
      const content = row[index] ?? '';
      const tone = index === 0 ? rowTone[rowIndex] : undefined;
      return el('td', { style: cellStyle(false, columnAlign[index]) }, [
        tone ? el('span', { style: { display: 'inline-block', width: 6, height: 6, borderRadius: 99, marginRight: 7, background: toneColor(tone), verticalAlign: 'middle' } }) : null,
        content,
      ]);
    }))) : [el('tr', {}, [el('td', { colspan: headers.length || 1, style: { ...cellStyle(false), color: 'var(--bitfun-canvas-muted)' } }, [emptyMessage])])];
    return el('div', { style: { overflow: 'auto', border: framed ? '1px solid rgba(127,127,127,0.20)' : 0, borderRadius: framed ? '8px' : 0, background: 'var(--bitfun-canvas-bg)', ...style } }, [el('table', { style: { width: '100%', borderCollapse: 'collapse', fontSize: '12px' } }, [
      el('thead', {}, [el('tr', {}, headers.map((h, index) => el('th', { style: { ...cellStyle(true, columnAlign[index]), position: stickyHeader ? 'sticky' : undefined, top: stickyHeader ? 0 : undefined, background: 'var(--bitfun-canvas-panel)' } }, [h])))]),
      el('tbody', {}, bodyRows)
    ])]);
  };
  const KeyValueList = ({ items = [], columns = 1, compact = false, emptyMessage = 'No details', style } = {}) => {
    const entries = Array.isArray(items) ? items : Object.entries(items || {}).map(([label, value]) => ({ label, value }));
    const columnCount = Math.max(1, Math.min(4, Math.floor(Number(columns) || 1)));
    return el('dl', { style: { display: 'grid', gridTemplateColumns: `repeat(${columnCount}, minmax(0, 1fr))`, gap: compact ? '6px' : '10px', margin: 0, ...style } }, entries.length ? entries.map((item, index) => el('div', { key: item.key || index, style: { minWidth: 0, padding: compact ? '0 0 6px' : '8px 0', borderBottom: '1px solid rgba(127,127,127,0.16)' } }, [
      el('dt', { style: { margin: 0, color: 'var(--bitfun-canvas-muted)', fontSize: '11px', lineHeight: 1.35 } }, [item.label]),
      el('dd', { style: { margin: '2px 0 0', color: toneColor(item.tone), fontSize: compact ? '12px' : '13px', fontWeight: 560, overflowWrap: 'anywhere' } }, [item.value]),
    ])) : [el('div', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, [emptyMessage])]);
  };
  const Timeline = ({ items = [], emptyMessage = 'No events', style } = {}) => el('ol', { style: { display: 'grid', gap: '10px', margin: 0, padding: 0, listStyle: 'none', ...style } }, items.length ? items.map((item, index) => el('li', { key: item.key || index, style: { display: 'grid', gridTemplateColumns: '18px minmax(0, 1fr)', gap: '9px', minWidth: 0 } }, [
    el('span', { style: { display: 'grid', placeItems: 'center', width: 18, height: 18, marginTop: 1, borderRadius: 999, background: 'rgba(127,127,127,0.12)', color: toneColor(item.tone), fontSize: '10px', fontWeight: 700 } }, [item.icon || '']),
    el('span', { style: { minWidth: 0, display: 'grid', gap: '2px' } }, [
      el('span', { style: { display: 'flex', gap: '8px', alignItems: 'baseline', justifyContent: 'space-between', minWidth: 0 } }, [
        el('strong', { style: { minWidth: 0, color: 'var(--bitfun-canvas-fg)', fontSize: '13px' } }, [item.title]),
        item.time ? el('time', { style: { flex: '0 0 auto', color: 'var(--bitfun-canvas-muted)', fontSize: '11px' } }, [item.time]) : null,
      ]),
      item.description ? el('span', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px', overflowWrap: 'anywhere' } }, [item.description]) : null,
    ]),
  ])) : [el('li', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, [emptyMessage])]);
  function fileTreeKey(item, index, depth) {
    return item.key || item.path || `${depth}-${index}-${String(item.name || '')}`;
  }
  function renderFileTreeItems(items, depth, defaultExpanded) {
    return (items || []).map((item, index) => {
      const children = Array.isArray(item.children) ? item.children : [];
      const isFolder = item.type === 'folder' || children.length > 0;
      const row = el('span', { style: { display: 'flex', alignItems: 'center', gap: '7px', minWidth: 0, padding: '3px 0', paddingLeft: `${depth * 16}px` } }, [
        el('span', { style: { flex: '0 0 auto', width: 14, color: isFolder ? 'var(--bitfun-canvas-accent)' : 'var(--bitfun-canvas-muted)' } }, [isFolder ? '▸' : '•']),
        el('span', { style: { minWidth: 0, color: toneColor(item.tone), fontFamily: 'ui-monospace,SFMono-Regular,Menlo,monospace', fontSize: '12px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' } }, [item.name || item.path]),
        item.meta ? el('span', { style: { flex: '0 0 auto', marginLeft: 'auto', color: 'var(--bitfun-canvas-muted)', fontSize: '11px' } }, [item.meta]) : null,
      ]);
      if (!isFolder) return el('div', { key: fileTreeKey(item, index, depth) }, [row]);
      return el('details', { key: fileTreeKey(item, index, depth), open: defaultExpanded }, [
        el('summary', { style: { display: 'block', cursor: 'default', listStyle: 'none' } }, [row]),
        ...renderFileTreeItems(children, depth + 1, defaultExpanded),
      ]);
    });
  }
  const FileTree = ({ items = [], defaultExpanded = true, emptyMessage = 'No files', style } = {}) => el('div', { style: { minWidth: 0, overflow: 'auto', border: '1px solid rgba(127,127,127,0.20)', borderRadius: '8px', padding: '8px 10px', background: 'rgba(127,127,127,0.04)', ...style } }, items.length ? renderFileTreeItems(items, 0, defaultExpanded) : [el('div', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, [emptyMessage])]);
  const ProgressBar = ({ value = 0, max = 100, label, tone = 'primary', showValue = true, style } = {}) => {
    const safeMax = Math.max(1, Number(max) || 100);
    const safeValue = Math.max(0, Math.min(safeMax, Number(value) || 0));
    const percent = Math.round(safeValue / safeMax * 100);
    return el('div', { style }, [
      label || showValue ? el('div', { style: { display: 'flex', justifyContent: 'space-between', gap: '10px', marginBottom: '5px', color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, [
        el('span', {}, [label]),
        showValue ? el('span', { style: { fontVariantNumeric: 'tabular-nums' } }, [`${percent}%`]) : null,
      ]) : null,
      el('div', { role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': safeMax, 'aria-valuenow': safeValue, style: { height: 8, overflow: 'hidden', borderRadius: 999, background: 'rgba(127,127,127,0.20)' } }, [
        el('div', { style: { width: `${percent}%`, height: '100%', borderRadius: 999, background: toneColor(tone) } }),
      ]),
    ]);
  };
  const Swatch = ({ color = 'gray', style, title } = {}) => el('span', {
    title,
    'aria-hidden': title ? undefined : true,
    style: {
      display: 'inline-block',
      width: 12,
      height: 12,
      borderRadius: 3,
      background: categoryColor(color),
      border: '1px solid var(--bitfun-canvas-border)',
      flex: '0 0 auto',
      ...style,
    },
  });
  function positiveSegmentValue(value) {
    const next = typeof value === 'number' ? value : Number(value);
    return Number.isFinite(next) && next > 0 ? next : 0;
  }
  const UsageBar = ({ segments = [], total = 0, topLeftLabel, topRightLabel, style } = {}) => {
    const normalized = (Array.isArray(segments) ? segments : []).map((segment, index) => ({
      ...segment,
      value: positiveSegmentValue(segment.value),
      color: segment.color || usageColorSequence[index % usageColorSequence.length],
    }));
    const segmentTotal = normalized.reduce((sum, segment) => sum + segment.value, 0);
    const safeTotal = Math.max(positiveSegmentValue(total), segmentTotal, 1);
    const remainder = Math.max(0, safeTotal - segmentTotal);
    return el('div', { style }, [
      topLeftLabel || topRightLabel ? el('div', { style: { display: 'flex', justifyContent: 'space-between', gap: 12, marginBottom: 6, color: 'var(--bitfun-canvas-muted)', fontSize: '12px', lineHeight: 1.35 } }, [
        el('span', {}, [topLeftLabel]),
        el('span', { style: { marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' } }, [topRightLabel]),
      ]) : null,
      el('div', { role: 'progressbar', 'aria-valuemin': 0, 'aria-valuemax': safeTotal, 'aria-valuenow': Math.min(segmentTotal, safeTotal), style: { display: 'flex', gap: 2, height: 10, overflow: 'hidden', borderRadius: 999, background: 'rgba(127,127,127,0.20)', padding: 1 } }, [
        ...normalized.map((segment, index) => segment.value > 0 ? el('span', { key: segment.id || index, title: `${segment.id}: ${segment.value}`, style: { flex: `${segment.value} 1 0`, minWidth: 2, borderRadius: 999, background: categoryColor(segment.color, index) } }) : null),
        remainder > 0 ? el('span', { 'aria-hidden': true, style: { flex: `${remainder} 1 0`, minWidth: 2, borderRadius: 999, background: 'rgba(127,127,127,0.10)' } }) : null,
      ]),
    ]);
  };
  function todoStatusColor(status) {
    if (status === 'completed') return 'var(--bitfun-canvas-success)';
    if (status === 'in_progress') return 'var(--bitfun-canvas-warning)';
    return 'var(--bitfun-canvas-muted)';
  }
  function todoStatusLabel(status) {
    if (status === 'completed') return 'completed';
    if (status === 'in_progress') return 'in progress';
    if (status === 'cancelled') return 'cancelled';
    return 'pending';
  }
  function dimmedTodoSet(value) {
    if (!value) return new Set();
    if (value instanceof Set) return value;
    return new Set(Array.isArray(value) ? value : []);
  }
  function TodoMarker({ status } = {}) {
    const color = todoStatusColor(status);
    const completed = status === 'completed';
    return el('span', { 'aria-hidden': true, style: { width: 14, height: 14, marginTop: 2, flex: '0 0 auto', display: 'inline-grid', placeItems: 'center', borderRadius: status === 'in_progress' ? 999 : 3, border: `1.5px solid ${color}`, background: completed ? color : 'transparent', color: 'var(--bitfun-canvas-panel)', fontSize: '10px', lineHeight: 1, fontWeight: 800 } }, [completed ? '✓' : '']);
  }
  const TodoList = ({ todos = [], dimmedTodoIds, onTodoClick, style } = {}) => {
    const list = Array.isArray(todos) ? todos : [];
    if (!list.length) return null;
    const dimmed = dimmedTodoSet(dimmedTodoIds);
    return el('div', { style: { display: 'grid', gap: 4, ...style } }, list.map(todo => {
      const content = todo.content || todo.id;
      const isDimmed = dimmed.has(todo.id);
      const rowStyle = {
        width: '100%',
        display: 'grid',
        gridTemplateColumns: '18px minmax(0, 1fr)',
        gap: 8,
        alignItems: 'start',
        border: 0,
        borderRadius: 6,
        padding: '6px 7px',
        background: 'transparent',
        color: 'var(--bitfun-canvas-fg)',
        font: 'inherit',
        textAlign: 'left',
        opacity: isDimmed ? 0.5 : 1,
        cursor: onTodoClick ? 'pointer' : 'default',
      };
      const body = [
        TodoMarker({ status: todo.status }),
        el('span', { style: { minWidth: 0, display: 'grid', gap: 2 } }, [
          el('span', { style: { color: todo.status === 'completed' ? 'var(--bitfun-canvas-muted)' : 'var(--bitfun-canvas-fg)', fontSize: '12px', lineHeight: 1.45, textDecoration: todo.status === 'completed' ? 'line-through' : undefined, overflowWrap: 'anywhere' } }, [content]),
          el('span', { style: { color: todoStatusColor(todo.status), fontSize: '10px', lineHeight: 1.2 } }, [todoStatusLabel(todo.status)]),
        ]),
      ];
      return onTodoClick
        ? el('button', { key: todo.id, type: 'button', onClick: () => onTodoClick(todo), style: rowStyle }, body)
        : el('div', { key: todo.id, style: rowStyle }, body);
    }));
  };
  const TodoListCard = ({ todos = [], dimmedTodoIds, defaultExpanded = false, onTodoClick, style } = {}) => {
    const list = Array.isArray(todos) ? todos : [];
    if (!list.length) return null;
    const completed = list.filter(todo => todo.status === 'completed').length;
    const key = `todo-list-card:${list.map(todo => todo.id).join('|')}`;
    const [open, setOpen] = useCanvasState(key, Boolean(defaultExpanded));
    return el('section', { style: { border: '1px solid var(--bitfun-canvas-border)', borderRadius: '8px', background: 'var(--bitfun-canvas-bg)', overflow: 'hidden', ...style } }, [
      el('button', { type: 'button', 'aria-expanded': open, onClick: () => setOpen(!open), style: { width: '100%', minHeight: 34, display: 'flex', alignItems: 'center', gap: 8, border: 0, borderBottom: open ? '1px solid var(--bitfun-canvas-border)' : 0, background: 'transparent', color: 'var(--bitfun-canvas-fg)', padding: '8px 10px', font: 'inherit', cursor: 'pointer', textAlign: 'left' } }, [
        el('span', { 'aria-hidden': true, style: { color: 'var(--bitfun-canvas-muted)', transform: open ? 'rotate(90deg)' : 'rotate(0deg)' } }, ['›']),
        el('span', { style: { fontWeight: 650, fontSize: '12px' } }, ['Tasks']),
        el('span', { style: { marginLeft: 'auto', color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, [`${completed}/${list.length} done`]),
      ]),
      open ? el('div', { style: { padding: 8 } }, [TodoList({ todos: list, dimmedTodoIds, onTodoClick })]) : null,
    ]);
  };
  const Button = ({ children, variant = 'secondary', onClick, disabled, type = 'button', style } = {}) => {
    const primary = variant === 'primary';
    const ghost = variant === 'ghost';
    return el('button', { type, onClick, disabled, style: { border: ghost ? '1px solid transparent' : '1px solid rgba(127,127,127,0.22)', borderRadius: '6px', background: primary ? 'var(--bitfun-canvas-accent)' : ghost ? 'transparent' : 'rgba(127,127,127,0.06)', color: primary ? '#fff' : 'var(--bitfun-canvas-fg)', padding: '4px 10px', minHeight: '24px', font: 'inherit', fontSize: '12px', cursor: disabled ? 'default' : 'pointer', opacity: disabled ? 0.55 : 1, ...style } }, children);
  };
  const Toggle = ({ checked, onChange, label, disabled, size = 'sm', style } = {}) => {
    const width = size === 'md' ? 34 : 28;
    const height = size === 'md' ? 20 : 16;
    const knob = height - 4;
    return el('button', { disabled, onClick: () => onChange?.(!checked), style: { width, height, border: 0, borderRadius: 999, background: checked ? 'var(--bitfun-canvas-accent)' : 'rgba(127,127,127,0.20)', padding: 2, cursor: disabled ? 'default' : 'pointer', opacity: disabled ? 0.55 : 1, ...style } }, [
      el('span', { style: { display: 'block', width: knob, height: knob, borderRadius: 999, background: '#fff', transform: `translateX(${checked ? width - height : 0}px)` } }),
      label ? el('span', {}, [label]) : null,
    ]);
  };
  const Checkbox = ({ checked, onChange, disabled, label, style } = {}) => el('label', { style: { display: 'inline-flex', gap: '6px', alignItems: 'center', fontSize: '12px', opacity: disabled ? 0.55 : 1, ...style } }, [el('input', { type: 'checkbox', checked, disabled, onChange: event => onChange?.(event.target.checked), style: { accentColor: 'var(--bitfun-canvas-accent)' } }), label]);
  const Select = ({ value, options = [], placeholder, disabled, onChange, style } = {}) => el('select', { value, disabled, onChange: event => onChange?.(event.target.value), style: { border: '1px solid rgba(127,127,127,0.22)', borderRadius: '6px', minHeight: '28px', padding: '4px 8px', background: 'var(--bitfun-canvas-panel)', color: 'var(--bitfun-canvas-fg)', ...style } }, [placeholder ? el('option', { value: '' }, [placeholder]) : null, ...options.map(option => typeof option === 'string' ? el('option', { value: option }, [option]) : el('option', { value: option.value, disabled: option.disabled }, [option.label]))]);
  const TextInput = ({ value, placeholder, disabled, type = 'text', onChange, style } = {}) => el('input', { value, placeholder, disabled, type, onInput: event => onChange?.(event.target.value), style: { border: '1px solid rgba(127,127,127,0.22)', borderRadius: '6px', minHeight: '28px', padding: '4px 8px', background: 'var(--bitfun-canvas-panel)', color: 'var(--bitfun-canvas-fg)', ...style } });
  const Input = ({ value, placeholder, disabled, type = 'text', onChange, label, hint, prefix, suffix, error, errorMessage, style } = {}) => el('label', { style: { display: 'grid', gap: '5px', color: 'var(--bitfun-canvas-fg)', fontSize: '12px', ...style } }, [
    label ? el('span', { style: { fontWeight: 600 } }, [label]) : null,
    el('span', { style: { display: 'flex', alignItems: 'center', gap: '6px', border: `1px solid ${error ? 'var(--bitfun-canvas-danger)' : 'rgba(127,127,127,0.22)'}`, borderRadius: '6px', minHeight: '30px', padding: '0 8px', background: 'var(--bitfun-canvas-panel)' } }, [
      prefix || null,
      el('input', { value, placeholder, disabled, type, onInput: event => onChange?.(event.target.value), style: { flex: 1, minWidth: 0, border: 0, outline: 0, background: 'transparent', color: 'var(--bitfun-canvas-fg)', font: 'inherit' } }),
      suffix || null,
    ]),
    error && errorMessage ? el('span', { style: { color: 'var(--bitfun-canvas-danger)' } }, [errorMessage]) : hint ? el('span', { style: { color: 'var(--bitfun-canvas-muted)' } }, [hint]) : null,
  ]);
  const TextArea = ({ value, placeholder, disabled, rows = 3, onChange, style } = {}) => el('textarea', { value, placeholder, disabled, rows, onInput: event => onChange?.(event.target.value), style: { border: '1px solid rgba(127,127,127,0.22)', borderRadius: '6px', padding: '7px 8px', background: 'var(--bitfun-canvas-panel)', color: 'var(--bitfun-canvas-fg)', font: 'inherit', fontSize: '13px', resize: 'vertical', width: '100%', boxSizing: 'border-box', ...style } });
  const IconButton = ({ children, onClick, disabled, title, variant = 'default', size = 'md', style } = {}) => {
    const px = size === 'sm' ? 18 : 24;
    return el('button', { title, onClick, disabled, style: { width: px, height: px, display: 'inline-grid', placeItems: 'center', border: 0, borderRadius: variant === 'circle' ? 999 : 5, background: variant === 'circle' ? 'rgba(127,127,127,0.12)' : 'transparent', color: 'var(--bitfun-canvas-muted)', cursor: disabled ? 'default' : 'pointer', opacity: disabled ? 0.55 : 1, ...style } }, children);
  };
  const DiffStats = ({ additions = 0, deletions = 0, style } = {}) => {
    if (!additions && !deletions) return null;
    return el('span', { style: { display: 'inline-flex', gap: '6px', alignItems: 'center', fontSize: '12px', fontVariantNumeric: 'tabular-nums', ...style } }, [
      additions ? el('span', { style: { color: 'var(--bitfun-canvas-success)' } }, [`+${additions}`]) : null,
      deletions ? el('span', { style: { color: 'var(--bitfun-canvas-danger)' } }, [`-${deletions}`]) : null,
    ]);
  };
  function diffLineStyle(type) {
    if (type === 'added') return { background: 'rgba(36,138,61,0.12)', color: 'var(--bitfun-canvas-fg)', accent: 'var(--bitfun-canvas-success)', sign: '+' };
    if (type === 'removed') return { background: 'rgba(209,36,47,0.12)', color: 'var(--bitfun-canvas-fg)', accent: 'var(--bitfun-canvas-danger)', sign: '-' };
    return { background: 'transparent', color: 'var(--bitfun-canvas-fg)', accent: 'transparent', sign: ' ' };
  }
  const DiffView = ({ lines = [], showLineNumbers = true, coloredLineNumbers = true, showAccentStrip = true, style } = {}) => el('div', { style: { overflow: 'auto', fontFamily: 'ui-monospace,SFMono-Regular,Menlo,monospace', fontSize: '12px', lineHeight: 1.55, background: 'rgba(127,127,127,0.035)', ...style } }, lines.map((line, index) => {
    const meta = diffLineStyle(line.type);
    return el('div', { style: { display: 'grid', gridTemplateColumns: `${showAccentStrip ? '3px ' : ''}${showLineNumbers ? '52px ' : ''}18px minmax(0,1fr)`, minWidth: '100%', background: meta.background, color: meta.color, whiteSpace: 'pre' } }, [
      showAccentStrip ? el('span', { style: { background: meta.accent } }) : null,
      showLineNumbers ? el('span', { style: { color: coloredLineNumbers && line.type !== 'unchanged' ? meta.accent : 'var(--bitfun-canvas-muted)', textAlign: 'right', padding: '0 8px', userSelect: 'none' } }, [line.lineNumber ?? index + 1]) : null,
      el('span', { style: { color: meta.accent === 'transparent' ? 'var(--bitfun-canvas-muted)' : meta.accent, userSelect: 'none' } }, [meta.sign]),
      el('span', { style: { paddingRight: '10px' } }, [line.content || '']),
    ]);
  }));
  const chartPalette = ['#3478f6', '#248a3d', '#b54708', '#d1242f', '#8250df', '#0a7ea4', '#bf3989'];
  function svg(tag, props = {}, children = []) {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    props = props || {};
    for (const [key, value] of Object.entries(props)) {
      if (key === 'children' || key === 'key' || value === undefined || value === null || value === false) continue;
      if (key === 'style') applyStyle(node, value);
      else if (key === 'className') node.setAttribute('class', String(value));
      else if (key === 'ref' && typeof value === 'function') value(node);
      else if (key === 'ref' && value && typeof value === 'object') value.current = node;
      else if (key.startsWith('on') && typeof value === 'function') node.addEventListener(key.slice(2).toLowerCase(), value);
      else node.setAttribute(svgAttrName(key), String(value));
    }
    appendChildren(node, children);
    return node;
  }
  function finiteNumber(value) {
    const number = Number(value);
    return Number.isFinite(number) ? number : 0;
  }
  function chartLabel(item, index, categories, labelKey) {
    if (categories[index] !== undefined) return String(categories[index]);
    if (item && typeof item === 'object') return String(item[labelKey] ?? item.label ?? item.name ?? item.category ?? index + 1);
    return String(index + 1);
  }
  function numericKeys(rows, labelKey, valueKey) {
    const blocked = new Set([labelKey, valueKey, 'label', 'name', 'category', 'color']);
    const keys = [];
    for (const row of rows) {
      if (!row || typeof row !== 'object' || Array.isArray(row)) continue;
      for (const [key, value] of Object.entries(row)) {
        if (blocked.has(key) || keys.includes(key)) continue;
        if (Number.isFinite(Number(value))) keys.push(key);
      }
    }
    return keys;
  }
  function normalizeChart(props = {}) {
    const categories = Array.isArray(props.categories) ? props.categories : [];
    const data = Array.isArray(props.data) ? props.data : [];
    const rawSeries = Array.isArray(props.series) ? props.series : [];
    const labelKey = props.labelKey || props.xKey || 'label';
    const valueKey = props.valueKey || props.yKey || 'value';
    let labels = categories.map(String);
    let series = [];
    if (rawSeries.length && rawSeries.every(item => item && typeof item === 'object' && Array.isArray(item.data))) {
      series = rawSeries.map((entry, index) => ({
        name: String(entry.name ?? entry.label ?? `Series ${index + 1}`),
        color: entry.color || chartPalette[index % chartPalette.length],
        values: entry.data.map((item, itemIndex) => finiteNumber(item && typeof item === 'object' ? item[valueKey] ?? item.value : item)),
      }));
      const maxLength = Math.max(labels.length, ...series.map(entry => entry.values.length));
      labels = Array.from({ length: maxLength }, (_, index) => labels[index] ?? String(index + 1));
    } else if (rawSeries.length && rawSeries.every(item => Number.isFinite(Number(item)))) {
      labels = labels.length ? labels : rawSeries.map((_, index) => String(index + 1));
      series = [{ name: props.name || 'Value', color: props.color || chartPalette[0], values: rawSeries.map(finiteNumber) }];
    } else if (data.length && data.every(item => Number.isFinite(Number(item)))) {
      labels = labels.length ? labels : data.map((_, index) => String(index + 1));
      series = [{ name: props.name || 'Value', color: props.color || chartPalette[0], values: data.map(finiteNumber) }];
    } else if (data.length) {
      const keys = numericKeys(data, labelKey, valueKey);
      labels = data.map((item, index) => chartLabel(item, index, categories, labelKey));
      if (keys.length) {
        series = keys.map((key, index) => ({
          name: key,
          color: chartPalette[index % chartPalette.length],
          values: data.map(item => finiteNumber(item?.[key])),
        }));
      } else {
        series = [{
          name: props.name || 'Value',
          color: props.color || chartPalette[0],
          values: data.map(item => finiteNumber(item?.[valueKey] ?? item?.value)),
        }];
      }
    }
    const maxLength = Math.max(labels.length, ...series.map(entry => entry.values.length), 0);
    labels = Array.from({ length: maxLength }, (_, index) => labels[index] ?? String(index + 1));
    series = series.map((entry, index) => ({
      ...entry,
      color: entry.color || chartPalette[index % chartPalette.length],
      values: Array.from({ length: maxLength }, (_, itemIndex) => finiteNumber(entry.values[itemIndex])),
    }));
    return { labels, series };
  }
  function chartShell(title, height, style, child) {
    return el('div', { style: { border: '1px solid var(--bitfun-canvas-border)', borderRadius: '8px', padding: '10px', background: 'rgba(127,127,127,0.04)', ...style } }, [
      title ? el('div', { style: { fontWeight: 650, marginBottom: '8px' } }, [title]) : null,
      child || el('div', { style: { minHeight: `${height}px`, display: 'grid', placeItems: 'center', color: 'var(--bitfun-canvas-muted)' } }, ['No chart data'])
    ]);
  }
  function chartLegend(series) {
    return el('div', { style: { display: 'flex', gap: '10px', flexWrap: 'wrap', marginTop: '8px', color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, series.map(entry => el('span', { style: { display: 'inline-flex', alignItems: 'center', gap: '5px' } }, [
      el('span', { style: { width: '9px', height: '9px', borderRadius: '999px', background: entry.color } }),
      entry.name
    ])));
  }
  function chartMax(series) {
    return Math.max(1, ...series.flatMap(entry => entry.values).map(value => Math.abs(value)));
  }
  function BarChart(props = {}) {
    const { labels, series } = normalizeChart(props);
    const height = Number(props.height) || 220;
    if (!labels.length || !series.length) return chartShell(props.title || 'Bar chart', height, props.style);
    const width = 720;
    const padding = { top: 12, right: 18, bottom: 42, left: 44 };
    const innerWidth = width - padding.left - padding.right;
    const innerHeight = height - padding.top - padding.bottom;
    const max = chartMax(series);
    const groupWidth = innerWidth / labels.length;
    const barWidth = Math.max(3, (groupWidth - 8) / series.length);
    const bars = [];
    labels.forEach((label, labelIndex) => {
      series.forEach((entry, seriesIndex) => {
        const value = entry.values[labelIndex] || 0;
        const barHeight = Math.abs(value) / max * innerHeight;
        const x = padding.left + labelIndex * groupWidth + 4 + seriesIndex * barWidth;
        const y = padding.top + innerHeight - barHeight;
        bars.push(svg('rect', { x, y, width: Math.max(1, barWidth - 2), height: barHeight, rx: 3, fill: entry.color }));
      });
    });
    const axis = [
      svg('line', { x1: padding.left, y1: padding.top + innerHeight, x2: width - padding.right, y2: padding.top + innerHeight, stroke: 'var(--bitfun-canvas-border)' }),
      svg('line', { x1: padding.left, y1: padding.top, x2: padding.left, y2: padding.top + innerHeight, stroke: 'var(--bitfun-canvas-border)' }),
      svg('text', { x: padding.left - 8, y: padding.top + 10, 'text-anchor': 'end', fill: 'var(--bitfun-canvas-muted)', 'font-size': 11 }, [String(max)]),
      ...labels.map((label, index) => svg('text', { x: padding.left + index * groupWidth + groupWidth / 2, y: height - 12, 'text-anchor': 'middle', fill: 'var(--bitfun-canvas-muted)', 'font-size': 11 }, [label])),
    ];
    return chartShell(props.title || 'Bar chart', height, props.style, el('div', {}, [
      svg('svg', { viewBox: `0 0 ${width} ${height}`, role: 'img', 'aria-label': props.title || 'Bar chart', style: { width: '100%', height: `${height}px`, display: 'block' } }, [...axis, ...bars]),
      chartLegend(series)
    ]));
  }
  function LineChart(props = {}) {
    const { labels, series } = normalizeChart(props);
    const height = Number(props.height) || 220;
    if (!labels.length || !series.length) return chartShell(props.title || 'Line chart', height, props.style);
    const width = 720;
    const padding = { top: 12, right: 18, bottom: 42, left: 44 };
    const innerWidth = width - padding.left - padding.right;
    const innerHeight = height - padding.top - padding.bottom;
    const max = chartMax(series);
    const step = labels.length > 1 ? innerWidth / (labels.length - 1) : innerWidth;
    const lines = series.flatMap(entry => {
      const points = entry.values.map((value, index) => {
        const x = padding.left + (labels.length > 1 ? index * step : innerWidth / 2);
        const y = padding.top + innerHeight - (Math.abs(value) / max * innerHeight);
        return [x, y];
      });
      return [
        svg('polyline', { points: points.map(point => point.join(',')).join(' '), fill: 'none', stroke: entry.color, 'stroke-width': 2.4, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }),
        ...points.map(point => svg('circle', { cx: point[0], cy: point[1], r: 3.4, fill: entry.color })),
      ];
    });
    const axis = [
      svg('line', { x1: padding.left, y1: padding.top + innerHeight, x2: width - padding.right, y2: padding.top + innerHeight, stroke: 'var(--bitfun-canvas-border)' }),
      svg('line', { x1: padding.left, y1: padding.top, x2: padding.left, y2: padding.top + innerHeight, stroke: 'var(--bitfun-canvas-border)' }),
      svg('text', { x: padding.left - 8, y: padding.top + 10, 'text-anchor': 'end', fill: 'var(--bitfun-canvas-muted)', 'font-size': 11 }, [String(max)]),
      ...labels.map((label, index) => svg('text', { x: padding.left + (labels.length > 1 ? index * step : innerWidth / 2), y: height - 12, 'text-anchor': 'middle', fill: 'var(--bitfun-canvas-muted)', 'font-size': 11 }, [label])),
    ];
    return chartShell(props.title || 'Line chart', height, props.style, el('div', {}, [
      svg('svg', { viewBox: `0 0 ${width} ${height}`, role: 'img', 'aria-label': props.title || 'Line chart', style: { width: '100%', height: `${height}px`, display: 'block' } }, [...axis, ...lines]),
      chartLegend(series)
    ]));
  }
  function polarPoint(cx, cy, radius, angle) {
    return [cx + radius * Math.cos(angle), cy + radius * Math.sin(angle)];
  }
  function piePath(cx, cy, radius, startAngle, endAngle) {
    const start = polarPoint(cx, cy, radius, startAngle);
    const end = polarPoint(cx, cy, radius, endAngle);
    const largeArc = endAngle - startAngle > Math.PI ? 1 : 0;
    return `M ${cx} ${cy} L ${start[0]} ${start[1]} A ${radius} ${radius} 0 ${largeArc} 1 ${end[0]} ${end[1]} Z`;
  }
  function PieChart(props = {}) {
    const normalized = normalizeChart(props);
    const values = normalized.series[0]?.values || [];
    const labels = normalized.labels;
    const height = Number(props.height) || 240;
    const entries = values.map((value, index) => ({ label: labels[index] || String(index + 1), value: Math.max(0, finiteNumber(value)), color: chartPalette[index % chartPalette.length] })).filter(entry => entry.value > 0);
    if (!entries.length) return chartShell(props.title || 'Pie chart', height, props.style);
    const width = 720;
    const cx = 160;
    const cy = height / 2;
    const radius = Math.max(48, Math.min(90, height / 2 - 18));
    const total = entries.reduce((sum, entry) => sum + entry.value, 0);
    let angle = -Math.PI / 2;
    const slices = entries.map(entry => {
      const nextAngle = angle + (entry.value / total) * Math.PI * 2;
      const path = svg('path', { d: piePath(cx, cy, radius, angle, nextAngle), fill: entry.color, stroke: 'var(--bitfun-canvas-bg)', 'stroke-width': 2 });
      angle = nextAngle;
      return path;
    });
    const legend = entries.map((entry, index) => {
      const y = 40 + index * 24;
      const percent = Math.round(entry.value / total * 100);
      return [
        svg('rect', { x: 330, y: y - 10, width: 10, height: 10, rx: 2, fill: entry.color }),
        svg('text', { x: 348, y, fill: 'var(--bitfun-canvas-fg)', 'font-size': 12 }, [`${entry.label} (${percent}%)`]),
      ];
    }).flat();
    return chartShell(props.title || 'Pie chart', height, props.style, svg('svg', { viewBox: `0 0 ${width} ${height}`, role: 'img', 'aria-label': props.title || 'Pie chart', style: { width: '100%', height: `${height}px`, display: 'block' } }, [...slices, ...legend]));
  }
  function diagramNodeLabel(node, fallback) {
    return node?.label ?? node?.title ?? fallback;
  }
  function diagramNodeDescription(node) {
    const meta = node?.meta;
    return node?.description ?? node?.subtitle ?? node?.sub ?? (typeof meta === 'string' || typeof meta === 'number' ? meta : undefined);
  }
  function diagramEdgePath(edge, direction) {
    if (direction === 'horizontal') {
      const midX = edge.sourceX + (edge.targetX - edge.sourceX) / 2;
      return `M ${edge.sourceX} ${edge.sourceY} C ${midX} ${edge.sourceY}, ${midX} ${edge.targetY}, ${edge.targetX} ${edge.targetY}`;
    }
    const midY = edge.sourceY + (edge.targetY - edge.sourceY) / 2;
    return `M ${edge.sourceX} ${edge.sourceY} C ${edge.sourceX} ${midY}, ${edge.targetX} ${midY}, ${edge.targetX} ${edge.targetY}`;
  }
  function renderDiagramSvg(layout, nodes, edges, label) {
    const nodeById = new Map((nodes || []).map(node => [String(node.id), node]));
    const edgeByKey = new Map(normalizeDAGEdges(edges || []).map(edge => [`${String(edge.from)}\u0000${String(edge.to)}`, edge]));
    const edgeEls = layout.edges.map((edge, index) => {
      const meta = edgeByKey.get(`${edge.from}\u0000${edge.to}`) || {};
      const color = toneColor(meta.tone);
      return svg('g', {}, [
        svg('path', { d: diagramEdgePath(edge, layout.direction), stroke: color, 'stroke-width': 1.5, opacity: edge.isBackEdge ? 0.5 : 0.75, fill: 'none' }),
        svg('circle', { cx: edge.targetX, cy: edge.targetY, r: 3, fill: color, opacity: 0.8 }),
        meta.label ? svg('text', { x: (edge.sourceX + edge.targetX) / 2, y: (edge.sourceY + edge.targetY) / 2 - 4, 'text-anchor': 'middle', fill: 'var(--bitfun-canvas-muted)', 'font-size': 10 }, [String(meta.label).slice(0, 18)]) : null,
      ]);
    });
    const nodeEls = layout.nodes.map(layoutNode => {
      const node = nodeById.get(layoutNode.id) || {};
      const title = diagramNodeLabel(node, layoutNode.id);
      const description = diagramNodeDescription(node);
      const color = toneColor(node.tone);
      return svg('g', { transform: `translate(${layoutNode.x} ${layoutNode.y})` }, [
        svg('rect', { width: layoutNode.width, height: layoutNode.height, rx: 6, fill: 'var(--bitfun-canvas-bg)', stroke: color, 'stroke-width': 1.25 }),
        svg('text', { x: 12, y: description ? 18 : layoutNode.height / 2 + 4, fill: 'var(--bitfun-canvas-fg)', 'font-size': 12, 'font-weight': 650 }, [String(title).slice(0, 22)]),
        description ? svg('text', { x: 12, y: 34, fill: 'var(--bitfun-canvas-muted)', 'font-size': 10 }, [String(description).slice(0, 26)]) : null,
      ]);
    });
    return svg('svg', { viewBox: `0 0 ${Math.max(layout.width, 1)} ${Math.max(layout.height, 1)}`, role: 'img', 'aria-label': label, style: { width: '100%', minWidth: `${layout.width}px`, height: `${layout.height}px`, display: 'block' } }, [...edgeEls, ...nodeEls]);
  }
  function diagramShell(title, height, style, child) {
    return el('div', { style: { minWidth: 0, overflow: 'auto', border: '1px solid var(--bitfun-canvas-border)', borderRadius: '8px', padding: '10px', background: 'rgba(127,127,127,0.04)', ...style } }, [
      title ? el('div', { style: { marginBottom: '10px', color: 'var(--bitfun-canvas-fg)', fontSize: '12px', fontWeight: 650 } }, [title]) : null,
      el('div', { style: { minHeight: `${height}px` } }, [child])
    ]);
  }
  function DependencyGraph(props = {}) {
    const nodes = Array.isArray(props.nodes) ? props.nodes : [];
    const edges = normalizeDAGEdges(props.edges);
    const layout = computeDAGLayout({ nodes, edges, direction: props.direction, nodeWidth: props.nodeWidth || 160, nodeHeight: props.nodeHeight || 46, rankGap: props.rankGap || 64, nodeGap: props.nodeGap || 48, padding: props.padding || 24 });
    return diagramShell(props.title, props.height || layout.height, props.style, nodes.length ? renderDiagramSvg(layout, nodes, edges, props.title || 'Dependency graph') : el('div', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, ['No graph nodes']));
  }
  function flowNodes(steps) {
    if (!Array.isArray(steps)) return [];
    return steps.map((step, index) => typeof step === 'string' ? { id: `step-${index + 1}`, label: step } : { id: step.id || `step-${index + 1}`, label: diagramNodeLabel(step, `Step ${index + 1}`), description: step.description ?? step.subtitle ?? step.sub, tone: step.tone, meta: step.meta });
  }
  function flowEdges(nodes) {
    return nodes.slice(0, -1).map((node, index) => ({ from: node.id, to: nodes[index + 1].id }));
  }
  function FlowDiagram(props = {}) {
    const stepNodes = flowNodes(props.steps);
    const nodes = Array.isArray(props.nodes) && props.nodes.length ? props.nodes : stepNodes;
    const edges = Array.isArray(props.edges) && props.edges.length ? normalizeDAGEdges(props.edges) : flowEdges(nodes);
    const layout = computeDAGLayout({ nodes, edges, direction: props.direction || 'horizontal', nodeWidth: props.nodeWidth || 150, nodeHeight: props.nodeHeight || 46, rankGap: props.rankGap || 54, nodeGap: props.nodeGap || 36, padding: props.padding || 20 });
    return diagramShell(props.title, props.height || layout.height, props.style, nodes.length ? renderDiagramSvg(layout, nodes, edges, props.title || 'Flow diagram') : el('div', { style: { color: 'var(--bitfun-canvas-muted)', fontSize: '12px' } }, ['No flow steps']));
  }
  function toneColor(tone) {
    return tone === 'muted' || tone === 'secondary' || tone === 'tertiary' || tone === 'quaternary' ? 'var(--bitfun-canvas-muted)' : tone === 'success' ? 'var(--bitfun-canvas-success)' : tone === 'warning' ? 'var(--bitfun-canvas-warning)' : tone === 'danger' ? 'var(--bitfun-canvas-danger)' : tone === 'info' ? 'var(--bitfun-canvas-info)' : tone === 'neutral' ? 'var(--bitfun-canvas-muted)' : 'var(--bitfun-canvas-fg)';
  }
  function weightValue(weight) { return weight === 'medium' ? 500 : weight === 'semibold' ? 650 : weight === 'bold' ? 700 : 400; }
  function cellStyle(head, align = 'left') { return { textAlign: align, padding: '7px 9px', borderBottom: '1px solid rgba(127,127,127,0.16)', fontWeight: head ? 650 : 400, color: head ? 'var(--bitfun-canvas-fg)' : undefined }; }
  function useHostTheme() { return { ...theme, tokens: theme }; }
  function depsChanged(previous, next) {
    if (!Array.isArray(next)) return true;
    if (!Array.isArray(previous)) return true;
    if (previous.length !== next.length) return true;
    return next.some((value, index) => !Object.is(value, previous[index]));
  }
  function queueRender() {
    if (renderQueued) return;
    renderQueued = true;
    setTimeout(() => {
      renderQueued = false;
      rerender();
    }, 0);
  }
  function useState(defaultValue) {
    const index = hookIndex++;
    if (index >= hookValues.length) {
      hookValues[index] = typeof defaultValue === 'function' ? defaultValue() : defaultValue;
    }
    return [hookValues[index], value => {
      const previous = hookValues[index];
      const next = typeof value === 'function' ? value(previous) : value;
      if (Object.is(previous, next)) return;
      hookValues[index] = next;
      queueRender();
    }];
  }
  function useRef(defaultValue) {
    const [ref] = useState(() => ({ current: defaultValue }));
    return ref;
  }
  function useMemo(factory, deps) {
    const index = hookIndex++;
    const previous = hookValues[index];
    if (!previous || depsChanged(previous.deps, deps)) {
      const value = factory();
      hookValues[index] = { deps: Array.isArray(deps) ? deps.slice() : deps, value };
      return value;
    }
    return previous.value;
  }
  function useCallback(callback, deps) {
    return useMemo(() => callback, deps);
  }
  function useEffect(effect, deps) {
    const index = hookIndex++;
    const previous = hookEffects[index];
    if (!previous || depsChanged(previous.deps, deps)) {
      hookEffects[index] = {
        deps: Array.isArray(deps) ? deps.slice() : deps,
        effect,
        cleanup: previous?.cleanup,
        pending: true,
      };
    }
  }
  function flushEffects() {
    hookEffects.forEach(entry => {
      if (!entry || !entry.pending) return;
      entry.pending = false;
      setTimeout(() => {
        if (typeof entry.cleanup === 'function') {
          try { entry.cleanup(); } catch (error) { reportRuntimeError(error); }
        }
        try {
          const cleanup = entry.effect();
          entry.cleanup = typeof cleanup === 'function' ? cleanup : undefined;
        } catch (error) {
          reportRuntimeError(error);
        }
      }, 0);
    });
  }
  function useCanvasState(key, defaultValue) {
    if (!state.has(key)) state.set(key, defaultValue);
    return [state.get(key), value => {
      state.set(key, typeof value === 'function' ? value(state.get(key)) : value);
      persistState();
      rerender();
    }];
  }
  let actionSeq = 0;
  const pendingActions = new Map();
  function useCanvasAction() {
    return action => new Promise((resolve, reject) => {
      const requestId = `action-${++actionSeq}`;
      pendingActions.set(requestId, { resolve, reject });
      window.parent?.postMessage({ type: 'bitfun-canvas-action', requestId, action }, '*');
    });
  }
  function persistState() {
    if (!hostStateReady) return;
    window.parent?.postMessage({
      type: 'bitfun-canvas-save-state',
      values: Object.fromEntries(state.entries())
    }, '*');
  }
  window.addEventListener('message', event => {
    const data = event.data || {};
    if (data.type === 'bitfun-canvas-action-result') {
      const pending = pendingActions.get(data.requestId);
      if (!pending) return;
      pendingActions.delete(data.requestId);
      if (data.error) pending.reject(new Error(String(data.error)));
      else pending.resolve(data.result ?? null);
      return;
    }
    if (data.type === 'bitfun-canvas-theme') {
      applyHostTheme(data.theme);
      rerender();
      return;
    }
    if (data.type === 'bitfun-canvas-design-mode') {
      setDesignMode(data.enabled);
      return;
    }
    if (data.type !== 'bitfun-canvas-state' && data.type !== 'bitfun-canvas-load-state-result' && data.type !== 'bitfun-canvas-save-state-result') return;
    const values = data.state && typeof data.state === 'object' && data.state.values && typeof data.state.values === 'object'
      ? data.state.values
      : {};
    for (const [key, value] of Object.entries(values)) {
      state.set(key, value);
    }
    hostStateReady = true;
    rerender();
  });
  const Fragment = ({ children } = {}) => toArray(children);
  window.BitfunCanvasSDK = { Stack, Row, Grid, Box, Divider, Spacer, H1, H2, H3, Text, Code, Link, Card, CardHeader, CardBody, Alert, Callout, CollapsibleSection, Empty, Tabs, Pill, Stat, Table, KeyValueList, Timeline, FileTree, ProgressBar, Swatch, UsageBar, TodoList, TodoListCard, DependencyGraph, FlowDiagram, BarChart, LineChart, PieChart, Button, Toggle, Checkbox, Select, Input, TextInput, TextArea, IconButton, DiffStats, DiffView, computeDAGLayout, mergeStyle, colorPalette, usageColorSequence, categoryPaletteLight, categoryPaletteDark, canvasPaletteLight, canvasPaletteDark, canvasTokensLight, canvasTokens, useHostTheme, useCanvasState, useCanvasAction, useState, useRef, useEffect, useCallback, useMemo };
  window.BitfunCanvasRuntime = { h, Fragment, mount(component) { renderFn = component; rerender(); } };
})();
