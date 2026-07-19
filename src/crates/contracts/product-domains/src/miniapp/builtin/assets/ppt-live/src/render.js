import { escapeHtml, extractHtmlSlideBackground, getActiveIndex, getActiveSlide, getSelectedElement, densityToIndex, indexToDensity, normalizeDensity } from './state.js';
import { translate as t, getLocale } from './i18n.js';
import { refreshFlatSelect } from './flat-select.js';
import { DEFAULT_STYLE_PRESET } from './style-presets.js';
import {
  elementModelElementHtml,
  resolveElementColor,
} from './element-model-html.js';
import { sanitizeSlideMarkup } from './sanitize-slide-markup.js';

export { resolveElementColor as resolveColor };

export function applyI18n() {
  document.documentElement.lang = getLocale();
  document.querySelectorAll('[data-i18n]').forEach((node) => {
    node.textContent = t(node.dataset.i18n);
  });
  document.querySelectorAll('[data-i18n-placeholder]').forEach((node) => {
    node.placeholder = t(node.dataset.i18nPlaceholder);
  });
  document.querySelectorAll('[data-i18n-aria]').forEach((node) => {
    node.setAttribute('aria-label', t(node.dataset.i18nAria));
  });
}

export function renderAll(state, handlers) {
  syncInputs(state);
  renderGeneration(state);
  renderGenerationOverlay(state);
  renderOutline(state, handlers);
  renderThumbs(state, handlers);
  renderSlideCanvas(state, handlers);
  renderInspector(state, handlers);
  ensureCanvasFitted();
  document.querySelectorAll('.segment').forEach((button) => {
    button.classList.toggle('is-active', button.dataset.mode === state.mode);
  });
  /* Update status bar slide position */
  const activeIndex = getActiveIndex(state);
  const slidePos = byId('slidePosition');
  if (slidePos) {
    slidePos.textContent = `${activeIndex + 1} / ${state.slides?.length || 1}`;
  }
}

let lastCanvasFitKey = '';
let canvasFitRetryId = 0;

const CANVAS_FIT_MIN_HEIGHT = 120;
const CANVAS_FIT_MAX_ATTEMPTS = 20;

function readPadding(styles) {
  return {
    x: (parseFloat(styles.paddingLeft) || 0) + (parseFloat(styles.paddingRight) || 0),
    y: (parseFloat(styles.paddingTop) || 0) + (parseFloat(styles.paddingBottom) || 0),
  };
}

/** Prefer client box; fall back to border box when WebView/grid reports 0 (common in Live App iframe). */
function readContentBoxSize(element, options = {}) {
  if (!element) return { width: 0, height: 0 };
  const styles = getComputedStyle(element);
  const pad = readPadding(styles);
  let width = Math.max(0, (element.clientWidth || 0) - pad.x);
  let height = Math.max(0, (element.clientHeight || 0) - pad.y);
  const rect = element.getBoundingClientRect();
  if (rect.width > 0) width = Math.max(width, rect.width - pad.x);
  if (rect.height > 0) height = Math.max(height, rect.height - pad.y);
  const minHeight = options.minHeight ?? CANVAS_FIT_MIN_HEIGHT;
  const fallback = options.fallback;
  if (fallback && height < minHeight) {
    const fbRect = fallback.getBoundingClientRect();
    if (fbRect.width > 0) width = Math.max(width, fbRect.width - pad.x);
    if (fbRect.height > 0) height = Math.max(height, fbRect.height - pad.y);
  }
  return { width: Math.max(0, width), height: Math.max(0, height) };
}

function readPreviewWidth(preview) {
  if (!preview) return 0;
  const rect = preview.getBoundingClientRect();
  return Math.max(preview.clientWidth || 0, preview.offsetWidth || 0, rect.width || 0);
}

export function ensureCanvasFitted() {
  if (canvasFitRetryId) cancelAnimationFrame(canvasFitRetryId);
  const tick = (attempt = 0) => {
    canvasFitRetryId = 0;
    fitSlideCanvas();
    fitThumbPreviews();
    observeThumbPreviews();
    observeSlidePreviewHosts();
    const area = byId('slideCanvas')?.closest('.canvas-area');
    if (!area || attempt >= CANVAS_FIT_MAX_ATTEMPTS) return;
    const { height } = readContentBoxSize(area, {
      fallback: area.closest('.stage-shell'),
    });
    if (height < CANVAS_FIT_MIN_HEIGHT) {
      canvasFitRetryId = requestAnimationFrame(() => tick(attempt + 1));
    }
  };
  canvasFitRetryId = requestAnimationFrame(() => tick(0));
}

export function fitSlideCanvas() {
  const canvas = byId('slideCanvas');
  const area = canvas?.closest('.canvas-area');
  const stage = canvas?.closest('.canvas-stage');
  if (!canvas || !area || !stage) return;

  const { width: innerW, height: innerH } = readContentBoxSize(area, {
    fallback: area.closest('.stage-shell'),
  });
  const maxW = Math.max(160, innerW);
  const maxH = Math.max(90, innerH);
  let width = maxW;
  let height = width * 9 / 16;
  if (height > maxH) {
    height = maxH;
    width = height * 16 / 9;
  }
  const w = Math.floor(width);
  const h = Math.floor(height);
  const fitKey = `${maxW}x${maxH}`;
  if (fitKey === lastCanvasFitKey && stage.style.width === `${w}px` && stage.style.height === `${h}px`) {
    const host = canvas.querySelector(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`);
    if (host) fitHtmlSlidePreviewSurface(host);
    return;
  }
  lastCanvasFitKey = fitKey;
  stage.style.width = `${w}px`;
  stage.style.height = `${h}px`;
  canvas.style.width = '100%';
  canvas.style.height = '100%';
  const present = byId('presentSlide');
  if (present) {
    present.style.width = `${w}px`;
    present.style.height = `${h}px`;
  }
  const host = canvas.querySelector(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`);
  if (host) fitHtmlSlidePreviewSurface(host);
}

export function positionFloatingToolbar(element) {
  const toolbar = byId('floatingToolbar');
  const canvas = byId('slideCanvas');
  if (!toolbar || !canvas || !element) {
    if (toolbar) toolbar.classList.remove('is-visible');
    return;
  }
  const rect = canvas.getBoundingClientRect();
  const elX = (element.x / 100) * rect.width;
  const elY = (element.y / 100) * rect.height;
  const elW = (element.w / 100) * rect.width;
  toolbar.style.left = `${rect.left + elX + elW / 2 - toolbar.offsetWidth / 2}px`;
  toolbar.style.top = `${rect.top + elY - toolbar.offsetHeight - 8}px`;
  toolbar.classList.add('is-visible');
}

function cssLengthToPx(raw, fallback) {
  const text = String(raw || '').trim();
  const num = parseFloat(text);
  if (!Number.isFinite(num)) return fallback;
  if (text.endsWith('pt')) return num * (96 / 72);
  if (text.endsWith('px')) return num;
  return num;
}

export const EXPORT_PREVIEW_WIDTH = 1280;
export const EXPORT_PREVIEW_HEIGHT = 720;

/** Map author html/body/:root rules onto preview/export wrapper selectors. */
export function scopeSlideAuthorStyles(cssText, rootSelector, bodySelector) {
  return String(cssText || '')
    .replace(/(^|[^-.\w]):root(?![\w-])/gi, `$1${rootSelector}`)
    .replace(/(^|[^-.\w])html(?![\w-])/gi, `$1${rootSelector}`)
    .replace(/(^|[^-.\w])body(?![\w-])/gi, `$1${bodySelector}`);
}

const HTML_SLIDE_PREVIEW_HOST_CLASS = 'html-slide-preview-host';
const HTML_SLIDE_PREVIEW_SCALER_CLASS = 'html-slide-preview-scaler';
const DEFAULT_SLIDE_DESIGN = { width: 1280, height: 720 };

function writeSandboxIframeDocument(frame, sanitizedHtml) {
  const doc = frame.contentDocument;
  if (!doc) return false;
  doc.open();
  doc.write(sanitizedHtml);
  doc.close();
  return true;
}

function mountSandboxIframeHtml(frame, html, onMounted) {
  const sanitizedHtml = sanitizeSlideMarkup(normalizeSlideDocument(html));
  frame.setAttribute('sandbox', 'allow-same-origin');
  // Never use srcdoc here: sandboxed srcdoc iframes render blank in Tauri
  // WebKit, and the srcdoc navigation would replace the written document.
  frame.src = 'about:blank';

  const mount = () => {
    if (!writeSandboxIframeDocument(frame, sanitizedHtml)) return false;
    onMounted?.();
    return true;
  };

  if (mount()) return;
  frame.addEventListener('load', () => {
    mount();
  }, { once: true });
}

/** Parse author slide canvas from inline CSS (960pt×540pt → 1280×720px). */
export function parseSlideDesignSizeFromHtml(html) {
  const text = String(html || '');
  const ptW = text.match(/(?:^|[{;\s])width\s*:\s*([\d.]+)\s*pt/i);
  const ptH = text.match(/(?:^|[{;\s])height\s*:\s*([\d.]+)\s*pt/i);
  if (ptW?.[1] && ptH?.[1]) {
    return {
      width: Math.round(parseFloat(ptW[1]) * (96 / 72)),
      height: Math.round(parseFloat(ptH[1]) * (96 / 72)),
    };
  }
  const pxW = text.match(/(?:^|[{;\s])width\s*:\s*([\d.]+)\s*px/i);
  const pxH = text.match(/(?:^|[{;\s])height\s*:\s*([\d.]+)\s*px/i);
  if (pxW?.[1] && pxH?.[1]) {
    return {
      width: Math.round(parseFloat(pxW[1])),
      height: Math.round(parseFloat(pxH[1])),
    };
  }
  return { ...DEFAULT_SLIDE_DESIGN };
}

function readFrameDesignSize(frame) {
  const w = Number(frame?.dataset?.designW);
  const h = Number(frame?.dataset?.designH);
  if (Number.isFinite(w) && w >= 320 && Number.isFinite(h) && h >= 180) {
    return { width: w, height: h };
  }
  return null;
}

function measureSlideDocumentSize(doc) {
  const body = doc?.body;
  const root = doc?.documentElement;
  const view = doc?.defaultView;
  if (!body || !root || !view) return { ...DEFAULT_SLIDE_DESIGN };
  const bodyStyle = view.getComputedStyle(body);
  const declaredW = cssLengthToPx(bodyStyle.width, 0);
  const declaredH = cssLengthToPx(bodyStyle.height, 0)
    || cssLengthToPx(bodyStyle.minHeight, 0);
  if (declaredW >= 320 && declaredH >= 180) {
    return { width: declaredW, height: declaredH };
  }
  let width = Math.max(body.scrollWidth || 0, body.offsetWidth || 0, root.clientWidth || 0, 1280);
  let height = Math.max(body.scrollHeight || 0, body.offsetHeight || 0, root.clientHeight || 0, 720);
  width = Math.min(Math.max(width, 320), 3840);
  height = Math.min(Math.max(height, 180), 3840);
  return { width, height };
}

function resolveSlideDesignSize(doc, frame) {
  const fromFrame = readFrameDesignSize(frame);
  if (fromFrame) return fromFrame;
  if (doc?.body) return measureSlideDocumentSize(doc);
  return { ...DEFAULT_SLIDE_DESIGN };
}

function readPreviewHostSize(host) {
  if (!host) return { width: 0, height: 0 };
  const rect = host.getBoundingClientRect();
  return {
    width: Math.max(host.clientWidth || 0, host.offsetWidth || 0, rect.width || 0),
    height: Math.max(host.clientHeight || 0, host.offsetHeight || 0, rect.height || 0),
  };
}

function stampFrameDesignSize(frame, html) {
  const design = parseSlideDesignSizeFromHtml(html);
  frame.dataset.designW = String(design.width);
  frame.dataset.designH = String(design.height);
  return design;
}

const EDITING_STYLE_ATTR = 'data-ppt-live-editing-style';
const EDITABLE_MARK_ATTR = 'data-ppt-live-editable';

/**
 * Snapshot the author's original inline styles on <html>/<body> before
 * lockSlideDocumentViewport overwrites them, so edited slides can be saved
 * without the preview-only sizing mutations.
 */
function captureOriginalInlineStyles(frame, doc) {
  if (!frame || frame._pptLiveOriginalInline) return;
  if (!doc?.documentElement || !doc.body) return;
  frame._pptLiveOriginalInline = {
    root: doc.documentElement.getAttribute('style'),
    body: doc.body.getAttribute('style'),
  };
}

function lockSlideDocumentViewport(doc, designW, designH) {
  if (!doc?.documentElement || !doc.body) return;
  const root = doc.documentElement;
  const body = doc.body;
  root.style.margin = '0';
  root.style.padding = '0';
  root.style.width = `${designW}px`;
  root.style.height = `${designH}px`;
  root.style.overflow = 'hidden';
  body.style.margin = '0';
  body.style.boxSizing = 'border-box';
  body.style.width = `${designW}px`;
  body.style.height = `${designH}px`;
  body.style.minHeight = '';
  body.style.maxWidth = '';
  body.style.overflow = 'hidden';
  body.style.transform = 'none';
}

/**
 * Fit a fixed-size HTML slide into a preview host using the iframe+scaler pattern:
 * iframe renders at design resolution; scaler clips to the scaled visual box (contain).
 */
export function fitHtmlSlidePreviewSurface(host) {
  const surface = host?.classList?.contains(HTML_SLIDE_PREVIEW_HOST_CLASS)
    ? host
    : host?.closest?.(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`);
  if (!surface) return false;

  const scaler = surface.querySelector(`.${HTML_SLIDE_PREVIEW_SCALER_CLASS}`);
  const frame = scaler?.querySelector('iframe, [data-slide-stage]');
  if (!scaler || !frame) return false;

  const { width: hostW, height: hostH } = readPreviewHostSize(surface);
  if (!hostW || !hostH) return false;

  const isIframe = frame.tagName === 'IFRAME';
  let doc = null;
  if (isIframe) {
    try {
      doc = frame.contentDocument;
    } catch {
      doc = null;
    }
  }

  const { width: designW, height: designH } = resolveSlideDesignSize(doc, frame);
  const scale = Math.min(hostW / designW, hostH / designH);
  const scaledW = designW * scale;
  const scaledH = designH * scale;

  if (doc) {
    captureOriginalInlineStyles(frame, doc);
    lockSlideDocumentViewport(doc, designW, designH);
  }

  scaler.style.width = `${scaledW}px`;
  scaler.style.height = `${scaledH}px`;
  scaler.style.overflow = 'hidden';
  scaler.style.position = 'relative';
  scaler.style.flexShrink = '0';

  frame.style.display = 'block';
  frame.style.width = `${designW}px`;
  frame.style.height = `${designH}px`;
  frame.style.border = '0';
  frame.style.margin = '0';
  frame.style.padding = '0';
  frame.style.maxWidth = 'none';
  frame.style.maxHeight = 'none';
  frame.style.transformOrigin = 'top left';
  frame.style.transform = `scale(${scale})`;

  return true;
}

const SLIDE_SHADOW_ROOT_CLASS = 'ppt-slide-shadow-root';
const SLIDE_SHADOW_BODY_CLASS = 'ppt-slide-shadow-body';

/**
 * Build the in-document editable slide stage. The PPT Live app document
 * itself lives in a sandboxed host iframe without `allow-same-origin`
 * (opaque origin), and sandbox flags propagate to nested iframes, so any
 * slide iframe is cross-origin by construction and `contentDocument` is
 * always null — in-place editing through an iframe is impossible. Instead,
 * the editable preview renders the slide inside a shadow root in the app
 * document: styles stay isolated and contenteditable works natively.
 */
function createEditableSlideStage(html, frameClass) {
  const stage = document.createElement('div');
  stage.className = frameClass;
  stage.dataset.slideStage = 'true';
  stampFrameDesignSize(stage, html);
  const designW = Number(stage.dataset.designW);
  const designH = Number(stage.dataset.designH);

  const sanitizedMarkup = sanitizeSlideMarkup(normalizeSlideDocument(html));
  const parsed = new DOMParser().parseFromString(sanitizedMarkup, 'text/html');

  const shadow = stage.attachShadow({ mode: 'open' });
  const rootEl = document.createElement('div');
  rootEl.className = SLIDE_SHADOW_ROOT_CLASS;
  // `all:initial` cuts inherited app-document styles at the shadow boundary
  // so the slide renders like a standalone document; later declarations in
  // the same block re-establish the layout box and browser-like defaults.
  rootEl.style.cssText = [
    'all:initial',
    'display:block',
    'position:relative',
    `width:${designW}px`,
    `height:${designH}px`,
    'margin:0',
    'padding:0',
    'overflow:hidden',
    'font-family:system-ui, -apple-system, "PingFang SC", "Source Han Sans SC", sans-serif',
    'font-size:16px',
    'line-height:normal',
    'color:#000',
    'background:#fff',
  ].join(';');

  parsed.querySelectorAll('style').forEach((node) => {
    const style = document.createElement('style');
    style.textContent = scopeSlideAuthorStyles(
      node.textContent || '',
      `.${SLIDE_SHADOW_ROOT_CLASS}`,
      `.${SLIDE_SHADOW_BODY_CLASS}`,
    );
    shadow.appendChild(style);
  });

  const bodyEl = document.createElement('div');
  bodyEl.className = SLIDE_SHADOW_BODY_CLASS;
  if (parsed.body) {
    for (const attr of parsed.body.attributes) {
      if (attr.name === 'class') {
        bodyEl.classList.add(...attr.value.split(/\s+/).filter(Boolean));
      } else if (attr.name === 'style') {
        bodyEl.style.cssText += `;${attr.value}`;
      } else if (!attr.name.toLowerCase().startsWith('on')) {
        bodyEl.setAttribute(attr.name, attr.value);
      }
    }
    bodyEl.innerHTML = parsed.body.innerHTML;
  }
  bodyEl.style.boxSizing = 'border-box';
  if (!/\bwidth\s*:/i.test(bodyEl.style.cssText)) bodyEl.style.width = `${designW}px`;
  if (!/\bheight\s*:/i.test(bodyEl.style.cssText)) bodyEl.style.height = `${designH}px`;
  bodyEl.style.overflow = 'hidden';
  bodyEl.style.margin = '0';

  rootEl.appendChild(bodyEl);
  shadow.appendChild(rootEl);
  stage._pptLiveSourceHtml = sanitizedMarkup;
  return stage;
}

function createHtmlSlidePreviewSurface({ hostClass = '', frameClass, html, onReady, interactive = false }) {
  const host = document.createElement('div');
  host.className = [HTML_SLIDE_PREVIEW_HOST_CLASS, hostClass].filter(Boolean).join(' ');

  const scaler = document.createElement('div');
  scaler.className = HTML_SLIDE_PREVIEW_SCALER_CLASS;

  if (interactive) {
    const stage = createEditableSlideStage(html, frameClass);
    scaler.appendChild(stage);
    host.appendChild(scaler);
    requestAnimationFrame(() => {
      fitHtmlSlidePreviewSurface(host);
      onReady?.(stage, host);
    });
    return { host, scaler, frame: stage };
  }

  const frame = document.createElement('iframe');
  frame.className = frameClass;
  frame.setAttribute('loading', 'lazy');
  stampFrameDesignSize(frame, html);
  mountSandboxIframeHtml(frame, html, () => {
    fitHtmlSlidePreviewSurface(host);
    onReady?.(frame, host);
  });

  scaler.appendChild(frame);
  host.appendChild(scaler);
  return { host, scaler, frame };
}

export function buildExportPreviewStage(html) {
  const { host } = createHtmlSlidePreviewSurface({
    hostClass: 'export-preview__html-stage',
    frameClass: 'export-preview__html-frame',
    html,
    onReady: () => {
      const viewport = host.closest('.export-preview__viewport');
      if (viewport) fitHtmlSlidePreviewSurface(host);
    },
  });
  return host;
}

export function fitExportPreviewFrame(container) {
  if (!container) return;
  const viewport = container.querySelector('.export-preview__viewport') || container;
  const scaleWrap = viewport.querySelector('.export-preview__scale');
  if (!scaleWrap) return;

  const htmlHost = scaleWrap.querySelector(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`);
  if (htmlHost) {
    scaleWrap.style.width = '100%';
    scaleWrap.style.height = '100%';
    fitHtmlSlidePreviewSurface(htmlHost);
    return;
  }

  const content = scaleWrap.querySelector('.export-preview__element-stage');
  if (!content) return;

  const { width: hostW, height: hostH } = readPreviewHostSize(viewport);
  if (!hostW || !hostH) return;

  const designW = 960;
  const designH = 540;
  const scale = Math.min(hostW / designW, hostH / designH);

  content.style.width = `${designW}px`;
  content.style.height = `${designH}px`;
  content.style.transform = `scale(${scale})`;
  content.style.transformOrigin = 'top left';
  scaleWrap.style.width = `${Math.floor(designW * scale)}px`;
  scaleWrap.style.height = `${Math.floor(designH * scale)}px`;
}

export function fitHtmlSlideFrame(frame) {
  if (!frame) return;
  const host = frame.closest(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`);
  if (host) fitHtmlSlidePreviewSurface(host);
}

function userFacingEventDetail(item) {
  if (!item) return '';
  const hiddenKinds = new Set(['turn', 'round', 'round-done', 'tokens', 'text', 'thinking']);
  if (hiddenKinds.has(item.kind || '')) return '';
  const detail = String(item.detail || '').trim();
  if (!detail) return '';
  if (/^[0-9a-f-]{8,}/i.test(detail)) return '';
  return detail;
}

function scrollGenerationListToLatest(list) {
  if (!list) return;
  const schedule = typeof requestAnimationFrame === 'function'
    ? requestAnimationFrame
    : (callback) => setTimeout(callback, 0);
  schedule(() => {
    list.scrollTop = list.scrollHeight;
  });
}

export function renderGeneration(state) {
  const list = byId('generationSteps');
  const steps = state.generation?.steps || [];
  const events = Array.isArray(state.generation?.events) ? state.generation.events : [];
  const stream = Array.isArray(state.generation?.agentStream) ? state.generation.agentStream : [];
  const isActive = Boolean(state.generation?.active || steps.some((step) => step.status === 'running'));
  const hasError = steps.some((step) => step.status === 'error');

  document.querySelector('.ppt-live')?.classList.toggle('is-generating', isActive);
  document.querySelector('.ppt-live')?.classList.toggle('has-generation-error', hasError);

  renderGenerationProgress(state, { isActive, hasError });

  if (!list) return;

  // Merge high-level phase events and granular agent-stream entries into a
  // single chronological timeline so users see exactly what the agent is
  // doing — no separate "Agent stream" panel, no frozen-looking status.
  const merged = mergeTimeline(events, stream);

  list.innerHTML = '';
  if (!merged.length) {
    const row = document.createElement('li');
    row.className = 'generation-event is-empty';
    row.innerHTML = `
      <span class="generation-index">--</span>
      <span class="generation-copy">
        <strong>${escapeHtml(t('processWaitingForEventsTitle'))}</strong>
        <small>${escapeHtml(t('processWaitingForEvents'))}</small>
      </span>
    `;
    list.append(row);
  } else {
    for (const item of merged) {
      const row = item.source === 'event'
        ? renderTimelineEvent(item)
        : renderTimelineStreamEntry(item);
      if (row) list.append(row);
    }
    // Live activity indicator — shows the agent is still working and what
    // it is currently doing, so the panel never looks frozen.
    if (isActive && !hasError) {
      const liveRow = renderLiveIndicator(state, merged);
      if (liveRow) list.append(liveRow);
    }
  }
  scrollGenerationListToLatest(list);
}

function renderGenerationProgress(state, { isActive, hasError }) {
  const panel = byId('generationProgress');
  const labelEl = byId('generationProgressLabel');
  const countEl = byId('generationProgressCount');
  const fillEl = byId('generationProgressFill');
  const phaseList = byId('generationPhaseList');
  if (!panel || !labelEl || !countEl || !fillEl || !phaseList) return;

  const steps = state.generation?.steps || [];
  const show = Boolean(isActive || hasError || steps.some((step) => step.status === 'done'));
  panel.hidden = !show;
  if (!show) return;

  const drafted = Number(state.generation?.draftedCount) || 0;
  const target = Number(state.generation?.slideTarget) || 0;
  const running = steps.find((step) => step.status === 'running');
  const ratio = target > 0
    ? Math.max(0, Math.min(1, drafted / target))
    : (running ? Math.max(0.08, steps.filter((step) => step.status === 'done').length / Math.max(steps.length, 1)) : (isActive ? 0.08 : 1));

  labelEl.textContent = running?.label
    || (hasError ? t('eventTurnFailed') : t('processEventDone'));
  countEl.textContent = target > 0
    ? `${Math.min(drafted, target)} / ${target}`
    : (drafted > 0 ? String(drafted) : '');
  fillEl.style.width = `${Math.round(ratio * 100)}%`;
  panel.classList.toggle('is-error', hasError);
  panel.classList.toggle('is-active', isActive && !hasError);

  phaseList.innerHTML = '';
  for (const step of steps) {
    const li = document.createElement('li');
    li.className = `generation-phase is-${step.status || 'pending'}`;
    li.textContent = step.label || step.id;
    phaseList.append(li);
  }
}

function mergeTimeline(events, stream) {
  const eventItems = events.map((e) => ({ ...e, source: 'event' }));
  const streamItems = stream.map((s) => ({ ...s, source: 'stream' }));
  return [...eventItems, ...streamItems]
    .sort((a, b) => (a.timestamp || 0) - (b.timestamp || 0))
    .slice(-120);
}

function renderTimelineEvent(event) {
  const detail = userFacingEventDetail(event);
  const li = document.createElement('li');
  li.className = `generation-event is-${event.kind || 'info'}`;
  li.innerHTML = `
    <span class="generation-index">${Number(event.seq) || '·'}</span>
    <span class="generation-copy">
      <strong>${escapeHtml(event.title || t('processEventUnknown'))}</strong>
      ${detail ? `<small>${escapeHtml(detail)}</small>` : ''}
    </span>
  `;
  return li;
}

function renderTimelineStreamEntry(entry) {
  const li = document.createElement('li');
  const kind = String(entry.kind || 'system');
  li.className = `generation-event is-stream is-stream-${kind}`;
  const prefix = entry.isSubagent ? '↳ ' : '';

  if (kind === 'text') {
    const text = truncateText(String(entry.text || '').trim(), 120);
    if (!text) return null;
    li.innerHTML = `
      <span class="generation-index generation-index--label">${escapeHtml(t('agentStreamAssistant'))}</span>
      <span class="generation-copy">
        <strong>${escapeHtml(prefix + text)}</strong>
      </span>
    `;
    return li;
  }
  if (kind === 'tool-start') {
    const label = friendlyStreamToolName(entry.toolName);
    li.innerHTML = `
      <span class="generation-index generation-index--label">${escapeHtml(label)}</span>
      <span class="generation-copy">
        <strong>${escapeHtml(prefix + truncateText(String(entry.text || ''), 120))}</strong>
      </span>
    `;
    return li;
  }
  if (kind === 'tool-done') {
    const label = friendlyStreamToolName(entry.toolName);
    const text = truncateText(String(entry.text || '').trim(), 120);
    li.innerHTML = `
      <span class="generation-index generation-index--label">${escapeHtml(label)} ✓</span>
      <span class="generation-copy">
        ${text ? `<strong>${escapeHtml(prefix + text)}</strong>` : '<small>✓</small>'}
      </span>
    `;
    return li;
  }
  if (kind === 'tool-error') {
    const label = friendlyStreamToolName(entry.toolName);
    li.innerHTML = `
      <span class="generation-index generation-index--label">${escapeHtml(label)} ✗</span>
      <span class="generation-copy">
        <strong>${escapeHtml(prefix + truncateText(String(entry.text || ''), 120))}</strong>
      </span>
    `;
    return li;
  }
  // system
  const text = truncateText(String(entry.text || '').trim(), 200);
  if (!text) return null;
  li.className = 'generation-event is-stream is-stream-system';
  li.innerHTML = `
    <span class="generation-index generation-index--dot">·</span>
    <span class="generation-copy">
      <small>${escapeHtml(prefix + text)}</small>
    </span>
  `;
  return li;
}

function renderLiveIndicator(state, merged) {
  const label = currentActivityLabel(state, merged);
  const li = document.createElement('li');
  li.className = 'generation-event is-live';
  li.innerHTML = `
    <span class="generation-index generation-index--live">
      <span class="live-dot" aria-hidden="true"></span>
    </span>
    <span class="generation-copy">
      <strong>${escapeHtml(label)}</strong>
    </span>
  `;
  return li;
}

function currentActivityLabel(state, merged) {
  const drafted = Number(state.generation?.draftedCount) || 0;
  const target = Number(state.generation?.slideTarget) || 0;
  const currentStep = (state.generation?.steps || []).find((s) => s.status === 'running');
  if (currentStep?.id === 'slides' && (drafted > 0 || target > 0)) {
    return t('generationWritingSlideProgress', {
      done: drafted,
      total: target || Math.max(drafted, 1),
    });
  }
  // Derive the most specific "what is happening right now" label.
  for (let i = merged.length - 1; i >= 0; i--) {
    const item = merged[i];
    if (item.source === 'stream') {
      if (item.kind === 'tool-start') {
        const tool = String(item.toolName || '').toLowerCase();
        const detail = truncateText(String(item.text || ''), 80);
        if ((tool === 'write' || tool === 'edit') && detail) {
          return `${friendlyStreamToolName(item.toolName)} ${detail}…`;
        }
        return `${friendlyStreamToolName(item.toolName)}…`;
      }
      if (item.kind === 'text') {
        return t('processEventText');
      }
    }
    if (item.source === 'event' && item.title && item.kind !== 'pulse') {
      return item.title;
    }
  }
  if (currentStep?.label) return `${currentStep.label}…`;
  return t('generationProgressPulse');
}

function truncateText(value, limit = 200) {
  const text = String(value || '').replace(/\s+/g, ' ').trim();
  if (!text) return '';
  return text.length > limit ? `${text.slice(0, limit - 1)}…` : text;
}

function friendlyStreamToolName(name) {
  const raw = String(name || '').trim();
  if (!raw) return t('eventUnknownTool');
  const lower = raw.toLowerCase();
  if (lower === 'websearch') return t('eventToolWebSearchName');
  if (lower === 'webfetch' || lower === 'mcp__web_reader__webreader') return t('eventToolWebFetchName');
  if (lower === 'skill') return t('eventToolSkillName');
  if (lower === 'read') return t('eventToolReadName');
  if (lower === 'write') return t('eventToolWriteName');
  if (lower === 'edit') return t('eventToolEditName');
  if (lower === 'grep') return 'Grep';
  if (lower === 'glob') return 'Glob';
  if (lower === 'task') return t('eventToolTaskName');
  if (lower === 'todowrite' || lower === 'todo_write') return 'TodoWrite';
  return raw;
}

export function renderGenerationOverlay(state) {
  const steps = state.generation?.steps || [];
  const isActive = Boolean(state.generation?.active || steps.some((step) => step.status === 'running'));
  const spinner = byId('statusSpinner');
  if (!spinner) return;
  spinner.hidden = !isActive;
  spinner.setAttribute('aria-hidden', isActive ? 'false' : 'true');
}

export function syncFontFamilyToggle(fontFamily = 'sans') {
  const value = fontFamily === 'serif' ? 'serif' : 'sans';
  document.querySelectorAll('[data-font-family]').forEach((button) => {
    const active = button.dataset.fontFamily === value;
    button.classList.toggle('is-active', active);
    button.setAttribute('aria-pressed', active ? 'true' : 'false');
  });
}

export function syncColorModeToggle(colorMode = 'light') {
  const value = colorMode === 'dark' ? 'dark' : 'light';
  document.querySelectorAll('[data-color-mode]').forEach((button) => {
    const active = button.dataset.colorMode === value;
    button.classList.toggle('is-active', active);
    button.setAttribute('aria-pressed', active ? 'true' : 'false');
  });
}

export function syncDensitySlider(density = 'standard') {
  const value = normalizeDensity(density);
  const index = densityToIndex(value);
  const root = document.getElementById('densitySlider');
  if (root) {
    root.style.setProperty('--density-index', String(index));
    root.dataset.index = String(index);
    root.setAttribute('aria-valuenow', String(index));
    const labelKey = `density${value.charAt(0).toUpperCase()}${value.slice(1)}`;
    root.setAttribute('aria-valuetext', t(labelKey));
    root.querySelectorAll('[data-density-index]').forEach((tick) => {
      const active = Number(tick.dataset.densityIndex) === index;
      tick.classList.toggle('is-active', active);
    });
  }
  document.querySelector('.ppt-live')?.setAttribute('data-density', value);
}

export function syncInputs(state) {
  const promptDraft = typeof state.promptDraft === 'string' ? state.promptDraft : '';
  const hasDeck = Array.isArray(state.slides) && state.slides.length > 0;
  value('topicInput', hasDeck ? promptDraft : (promptDraft || state.brief.topic));
  text('deckTitle', state.title || t('defaultDeckTitle'));
  text('deckMeta', t('slidesMeta', { count: state.slides.length }));
  text('currentSlideIndex', String(getActiveIndex(state) + 1));
}

/** Push property-panel controls into state immediately before generation. */
export function readStyleInputs(state) {
  if (!state) return;
  const stylePresetSelect = document.getElementById('stylePresetSelect');
  const stylePreset = stylePresetSelect?.value || DEFAULT_STYLE_PRESET;
  const activeFont = document.querySelector('[data-font-family].is-active');
  const activeColor = document.querySelector('[data-color-mode].is-active');
  const densitySlider = document.getElementById('densitySlider');
  const densityIndex = Math.max(0, Math.min(2, Number(densitySlider?.dataset.index ?? 1)));
  state.style = {
    ...(state.style || {}),
    stylePreset,
    fontFamily: activeFont?.dataset.fontFamily === 'serif' ? 'serif' : 'sans',
    colorMode: activeColor?.dataset.colorMode === 'dark' ? 'dark' : 'light',
    density: indexToDensity(densityIndex),
  };
}

/** Initialize or reset the property panel from persisted deck state. */
export function syncStylePanelFromState(state) {
  if (!state?.style) return;
  syncFontFamilyToggle(state.style.fontFamily);
  syncColorModeToggle(state.style.colorMode);
  syncDensitySlider(state.style.density);
  const stylePresetSelect = document.getElementById('stylePresetSelect');
  if (stylePresetSelect) {
    stylePresetSelect.value = state.style.stylePreset || DEFAULT_STYLE_PRESET;
    refreshFlatSelect(stylePresetSelect);
  }
}

export function readInputs(state, options = {}) {
  const includeTopic = options.includeTopic !== false;
  if (includeTopic) {
    state.brief.topic = val('topicInput');
    state.promptDraft = state.brief.topic;
    inferBriefFromPrompt(state);
  }
}

function inferBriefFromPrompt(state) {
  const prompt = String(state.brief.topic || '');
  const slideMatch = prompt.match(/(\d{1,2})\s*(?:页|页面|张|slides?|pages?)/i)
    || prompt.match(/(?:页数|slides?|pages?)\D{0,8}(\d{1,2})/i);
  if (slideMatch) state.brief.slideTarget = Math.max(3, Math.min(24, Number(slideMatch[1])));
  else state.brief.slideTarget = 0;
}

export function renderOutline(state, handlers) {
  const list = byId('outlineList');
  if (!list) return;
  list.innerHTML = '';
  state.outline.forEach((item, index) => {
    const row = document.createElement('li');
    const slide = state.slides[index];
    row.className = `outline-row${slide?.id === state.activeSlideId ? ' is-active' : ''}`;
    row.innerHTML = `
      <span class="outline-index">${index + 1}</span>
      <button class="outline-card" type="button">
        <strong>${escapeHtml(item)}</strong>
        <small>${escapeHtml(slide?.proofObject || '')}</small>
      </button>
    `;
    row.querySelector('.outline-card').addEventListener('click', () => {
      if (slide?.id) handlers.selectSlide(slide.id);
    });
    list.append(row);
  });
}

export function renderThumbs(state, handlers) {
  const holder = byId('slideThumbs');
  if (!holder) return;
  // Full rebuild resets scrollTop; keep the filmstrip viewport stable when
  // the user clicks a non-visible thumb or when selection only changes.
  const previousScrollTop = holder.scrollTop;
  holder.innerHTML = '';
  if (!state.slides.length) {
    const empty = document.createElement('div');
    empty.className = 'thumbs-empty';
    empty.textContent = t('slidesEmptyHint');
    holder.append(empty);
    return;
  }
  state.slides.forEach((slide, index) => {
    const extractedBackground = slide.html ? extractHtmlSlideBackground(slide.html) : null;
    const theme = slide.theme || {};
    const thumbBackground = extractedBackground || theme.background || 'var(--studio-slide-chrome)';
    const button = document.createElement('button');
    button.className = `thumb${slide.id === state.activeSlideId ? ' is-active' : ''}`;
    button.type = 'button';
    button.style.setProperty('--thumb-bg', thumbBackground);
    button.style.setProperty('--thumb-primary', theme.primary || 'var(--studio-accent)');

    const preview = document.createElement('div');
    preview.className = 'thumb-preview';
    preview.style.background = thumbBackground;
    if (slide.html) {
      preview.appendChild(buildHtmlThumbStage(slide.html));
    } else {
      const slideNode = document.createElement('div');
      slideNode.className = 'thumb-preview-slide';
      slideNode.innerHTML = slideHtml(slide);
      hydrateHtmlSlideIframes(slideNode);
      preview.appendChild(slideNode);
    }
    button.appendChild(preview);

    const copy = document.createElement('div');
    copy.className = 'thumb-copy';
    copy.innerHTML = `
      <span class="thumb-kicker">${escapeHtml(slide.kicker || '')}</span>
      <span class="thumb-title">${escapeHtml(slide.title)}</span>
    `;
    button.appendChild(copy);

    const number = document.createElement('span');
    number.className = 'thumb-number';
    number.textContent = String(index + 1);
    button.appendChild(number);

    button.addEventListener('click', () => handlers.selectSlide(slide.id));
    holder.append(button);
  });
  holder.scrollTop = previousScrollTop;
  requestAnimationFrame(() => {
    holder.scrollTop = previousScrollTop;
    fitThumbPreviews();
    observeThumbPreviews();
  });
}

const THUMB_BASE_WIDTH = 960;
const THUMB_BASE_HEIGHT = 540;

function buildHtmlThumbStage(html) {
  const { host } = createHtmlSlidePreviewSurface({
    hostClass: 'thumb-preview-html',
    frameClass: 'thumb-preview-frame',
    html,
    onReady: () => fitHtmlSlidePreviewSurface(host),
  });
  return host;
}

export function fitThumbPreviewFrame(frame, preview) {
  const host = frame?.closest?.(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`)
    || preview?.querySelector?.(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`);
  if (host) {
    fitHtmlSlidePreviewSurface(host);
    return;
  }
  if (!frame || !preview) return;
  const { width: hostW, height: hostH } = readPreviewHostSize(preview);
  if (!hostW || !hostH) return;
  const scale = Math.min(hostW / THUMB_BASE_WIDTH, hostH / THUMB_BASE_HEIGHT);
  frame.style.width = `${THUMB_BASE_WIDTH}px`;
  frame.style.height = `${THUMB_BASE_HEIGHT}px`;
  frame.style.transform = `scale(${scale})`;
  frame.style.transformOrigin = 'top left';
}

function fitThumbPreviewContent(preview) {
  const frame = preview.querySelector('.thumb-preview-frame');
  if (frame) {
    fitThumbPreviewFrame(frame, preview);
    return;
  }
  const content = preview.querySelector('.thumb-preview-html, .thumb-preview-slide');
  if (!content) return;
  const width = readPreviewWidth(preview);
  if (!width) return;
  const scale = width / THUMB_BASE_WIDTH;
  const scaledH = THUMB_BASE_HEIGHT * scale;
  content.style.width = `${THUMB_BASE_WIDTH}px`;
  content.style.height = `${THUMB_BASE_HEIGHT}px`;
  content.style.transform = `scale(${scale})`;
  content.style.transformOrigin = 'top left';
  preview.style.height = `${scaledH}px`;
}

function fitThumbPreviewSlide(preview) {
  fitThumbPreviewContent(preview);
}

export function fitThumbPreviews() {
  const holder = byId('slideThumbs');
  if (!holder) return;
  holder.querySelectorAll(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`).forEach((host) => {
    fitHtmlSlidePreviewSurface(host);
  });
  holder.querySelectorAll('.thumb-preview').forEach((preview) => {
    if (!preview.querySelector(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`)) {
      fitThumbPreviewContent(preview);
    }
  });
}

let thumbPreviewObserver = null;
let slidePreviewResizeObserver = null;

export function observeSlidePreviewHosts() {
  if (typeof ResizeObserver === 'undefined') return;
  if (!slidePreviewResizeObserver) {
    slidePreviewResizeObserver = new ResizeObserver(() => {
      document.querySelectorAll(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`).forEach((host) => {
        fitHtmlSlidePreviewSurface(host);
      });
    });
  }
  slidePreviewResizeObserver.disconnect();
  document.querySelectorAll(`.${HTML_SLIDE_PREVIEW_HOST_CLASS}`).forEach((host) => {
    slidePreviewResizeObserver.observe(host);
  });
  const canvas = byId('slideCanvas');
  if (canvas) slidePreviewResizeObserver.observe(canvas);
}

export function observeThumbPreviews() {
  const holder = byId('slideThumbs');
  if (!holder || typeof ResizeObserver === 'undefined') return;
  if (!thumbPreviewObserver) {
    thumbPreviewObserver = new ResizeObserver(() => fitThumbPreviews());
  }
  thumbPreviewObserver.disconnect();
  thumbPreviewObserver.observe(holder);
  holder.querySelectorAll('.thumb-preview').forEach((preview) => {
    thumbPreviewObserver.observe(preview);
  });
  observeSlidePreviewHosts();
}

function isStarterDeck(state) {
  if (!String(state.brief?.topic || '').trim()) {
    const title = String(state.title || '').trim();
    if (title === t('blankDeckTitle') || title === t('defaultDeckTitle') || title === t('newSlideTitle')) {
      return true;
    }
  }
  if (!state.slides?.length) return true;
  const title = String(state.title || '').trim();
  const onlyStarterSlide = state.slides.length === 1
    && state.outline.length === 1
    && state.outline[0] === t('newSlideTitle');
  return onlyStarterSlide
    && (title === t('blankDeckTitle') || title === t('newSlideTitle'));
}

function applySlideCanvasBackground(canvas, slide) {
  if (!canvas) return;
  if (!slide) {
    canvas.style.background = '';
    return;
  }
  const theme = slide.theme || {};
  const extracted = slide.html ? extractHtmlSlideBackground(slide.html) : null;
  const background = extracted || theme.background || '';
  canvas.style.background = background || '';
}

export function renderSlideCanvas(state, handlers) {
  const canvas = byId('slideCanvas');
  if (!canvas) return;
  const slide = getActiveSlide(state);
  const isGenerating = Boolean(state.generation?.active || state.generation?.steps?.some((step) => step.status === 'running'));
  if (!slide) {
    canvas.classList.remove('is-html-slide');
    canvas.classList.add('is-empty');
    canvas.innerHTML = isGenerating
      ? `<div class="slide-empty-state"><span aria-hidden="true">PL</span><strong>${escapeHtml(t('generationAgentWorking'))}</strong><p>${escapeHtml(t('agentWorkingDetail'))}</p></div>`
      : `<div class="welcome-hero"><span class="welcome-hero__icon" aria-hidden="true">PL</span><h2>${escapeHtml(t('welcomeTitle'))}</h2><p>${escapeHtml(t('welcomeSubcopy'))}</p><div class="welcome-hero__tips"><button type="button" class="welcome-tip" data-welcome-prompt="${escapeHtml(t('welcomeTip1'))}">${escapeHtml(t('welcomeTip1'))}</button><button type="button" class="welcome-tip" data-welcome-prompt="${escapeHtml(t('welcomeTip2'))}">${escapeHtml(t('welcomeTip2'))}</button><button type="button" class="welcome-tip" data-welcome-prompt="${escapeHtml(t('welcomeTip3'))}">${escapeHtml(t('welcomeTip3'))}</button></div></div>`;
    bindWelcomeTips(canvas);
    applySlideCanvasBackground(canvas, null);
    fitSlideCanvas();
    return;
  }
  if (isStarterDeck(state) && !slide.html && !isGenerating) {
    canvas.classList.remove('is-html-slide');
    canvas.classList.add('is-empty');
    canvas.innerHTML = `
      <div class="welcome-hero">
        <span class="welcome-hero__icon" aria-hidden="true">PL</span>
        <h2>${escapeHtml(t('welcomeTitle'))}</h2>
        <p>${escapeHtml(t('welcomeSubcopy'))}</p>
        <div class="welcome-hero__tips">
          <button type="button" class="welcome-tip" data-welcome-prompt="${escapeHtml(t('welcomeTip1'))}">${escapeHtml(t('welcomeTip1'))}</button>
          <button type="button" class="welcome-tip" data-welcome-prompt="${escapeHtml(t('welcomeTip2'))}">${escapeHtml(t('welcomeTip2'))}</button>
          <button type="button" class="welcome-tip" data-welcome-prompt="${escapeHtml(t('welcomeTip3'))}">${escapeHtml(t('welcomeTip3'))}</button>
        </div>
      </div>
    `;
    bindWelcomeTips(canvas);
    applySlideCanvasBackground(canvas, null);
    fitSlideCanvas();
    return;
  }
  canvas.classList.remove('is-empty');
  if (slide?.html) {
    canvas.innerHTML = '';
    canvas.classList.add('is-html-slide');
    const { host, frame } = createHtmlSlidePreviewSurface({
      frameClass: 'html-slide-frame',
      html: slide.html,
      interactive: true,
      onReady: (loadedFrame) => {
        bindHtmlSlideEditing(loadedFrame, slide.id, handlers);
        fitSlideCanvas();
        fitHtmlSlidePreviewSurface(host);
        requestAnimationFrame(() => fitHtmlSlidePreviewSurface(host));
      },
    });
    canvas.append(host);
    applySlideCanvasBackground(canvas, slide);
    canvas.classList.remove('is-entering');
    void canvas.offsetWidth;
    canvas.classList.add('is-entering');
    fitSlideCanvas();
    return;
  }
  canvas.classList.remove('is-html-slide');
  canvas.innerHTML = slide ? slideHtml(slide, { selectedElementId: state.selectedElementId, editable: true }) : '';
  hydrateHtmlSlideIframes(canvas);
  canvas.querySelectorAll('.slide-element').forEach((node) => {
    const elementId = node.dataset.elementId;
    node.addEventListener('click', (event) => {
      event.stopPropagation();
      handlers.selectElement(elementId);
    });
    node.addEventListener('pointerdown', (event) => {
      if (event.target?.isContentEditable && !event.target.classList.contains('resize-handle')) return;
      handlers.beginDrag(event, elementId);
    });
  });
  canvas.querySelectorAll('[data-edit-text]').forEach((node) => {
    node.addEventListener('blur', () => {
      handlers.updateElementTextDirect(node.dataset.editText, node.textContent || '');
    });
    node.addEventListener('keydown', (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') node.blur();
    });
  });
  canvas.querySelectorAll('[data-edit-list]').forEach((node) => {
    node.addEventListener('blur', () => {
      handlers.updateElementListItemDirect(node.dataset.editList, Number(node.dataset.itemIndex), node.textContent || '');
    });
    node.addEventListener('keydown', (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') node.blur();
    });
  });
  canvas.classList.remove('is-entering');
  void canvas.offsetWidth;
  canvas.classList.add('is-entering');
  /* Position floating toolbar on selected element */
  const selectedEl = getSelectedElement(state);
  if (selectedEl) {
    positionFloatingToolbar(selectedEl);
  } else {
    const toolbar = byId('floatingToolbar');
    if (toolbar) toolbar.classList.remove('is-visible');
  }
  applySlideCanvasBackground(canvas, slide);
  fitSlideCanvas();
}

export function renderInspector(state, handlers) {
  const panel = byId('elementInspector');
  const element = getSelectedElement(state);
  const slide = getActiveSlide(state);
  if (!panel || !slide) return;
  if (panel.hidden) {
    panel.innerHTML = '';
    return;
  }
  if (!element) {
    panel.innerHTML = `${slideMethodologyFields(slide)}<p class="empty-copy">${t('noSelection')}</p><label>${t('speakerNotesLabel')}<textarea id="slideNotesInput" rows="5">${escapeHtml(slide.notes || '')}</textarea></label>`;
    bindSlideFields(panel, handlers);
    panel.querySelector('#slideNotesInput')?.addEventListener('input', (event) => handlers.updateSlideNotes(event.target.value));
    return;
  }
  panel.innerHTML = `
    ${slideMethodologyFields(slide)}
    <label>${t('elementTypeLabel')}<input value="${escapeHtml(element.type)}" readonly></label>
    <label>${t('elementTextLabel')}<textarea id="elementTextInput" rows="4">${escapeHtml(element.text || '')}</textarea></label>
    <label>${t('elementItemsLabel')}<textarea id="elementItemsInput" rows="4">${escapeHtml((element.items || []).join('\n'))}</textarea></label>
    <label>${t('elementDataLabel')}<textarea id="elementDataInput" rows="4">${escapeHtml((element.data || []).map((point) => `${point.label}: ${point.value}`).join('\n'))}</textarea></label>
    <div class="field-grid dense">
      <label>X<input id="elementXInput" type="number" min="0" max="100" value="${round(element.x)}"></label>
      <label>Y<input id="elementYInput" type="number" min="0" max="100" value="${round(element.y)}"></label>
      <label>W<input id="elementWInput" type="number" min="3" max="100" value="${round(element.w)}"></label>
      <label>H<input id="elementHInput" type="number" min="3" max="100" value="${round(element.h)}"></label>
    </div>
    <div class="field-grid dense">
      <label>Font<input id="elementFontInput" type="number" min="8" max="88" value="${element.style.fontSize}"></label>
      <label>Weight<input id="elementWeightInput" type="number" min="100" max="900" step="50" value="${element.style.fontWeight}"></label>
      <label>Color<input id="elementColorInput" type="text" value="${escapeHtml(element.style.color)}"></label>
      <label>Bg<input id="elementBgInput" type="text" value="${escapeHtml(element.style.background)}"></label>
    </div>
    <label>${t('speakerNotesLabel')}<textarea id="slideNotesInput" rows="5">${escapeHtml(slide.notes || '')}</textarea></label>
  `;
  [
    'elementTextInput',
    'elementItemsInput',
    'elementDataInput',
    'elementXInput',
    'elementYInput',
    'elementWInput',
    'elementHInput',
    'elementFontInput',
    'elementWeightInput',
    'elementColorInput',
    'elementBgInput',
    'slideNotesInput',
  ].forEach((id) => panel.querySelector(`#${id}`)?.addEventListener('input', () => handlers.updateElementFromInspector()));
  bindSlideFields(panel, handlers);
}

function slideMethodologyFields(slide) {
  return `
    <div class="method-fields">
      <label>${t('kickerLabel')}<input id="slideKickerInput" value="${escapeHtml(slide.kicker || '')}"></label>
      <label>${t('claimLabel')}<textarea id="slideClaimInput" rows="3">${escapeHtml(slide.claim || '')}</textarea></label>
      <label>${t('proofObjectLabel')}<input id="slideProofInput" value="${escapeHtml(slide.proofObject || '')}"></label>
      <label>${t('supportNoteLabel')}<textarea id="slideSupportInput" rows="3">${escapeHtml(slide.supportNote || '')}</textarea></label>
      <label>${t('sourceNoteLabel')}<input id="slideSourceInput" value="${escapeHtml(slide.sourceNote || '')}"></label>
    </div>
    ${slideQualityFields(slide)}
  `;
}

function slideQualityFields(slide) {
  const issues = Array.isArray(slide.quality?.issues) ? slide.quality.issues : [];
  if (!issues.length) return '';
  return `
    <div class="method-fields quality-fields">
      <strong>${escapeHtml(t('qualityReportTitle'))}: ${Math.round(Number(slide.quality?.score ?? 100))}/100</strong>
      <ul>${issues.map((issue) => `<li data-severity="${escapeHtml(issue.severity)}">${escapeHtml(issue.message)}</li>`).join('')}</ul>
    </div>
  `;
}

function bindSlideFields(panel, handlers) {
  ['slideKickerInput', 'slideClaimInput', 'slideProofInput', 'slideSupportInput', 'slideSourceInput'].forEach((id) => {
    panel.querySelector(`#${id}`)?.addEventListener('input', () => handlers.updateSlideMethodology());
  });
}

const pendingSlideHtmlMounts = new Map();
let pendingSlideHtmlMountSeq = 0;

export function hydrateHtmlSlideIframes(root = document) {
  const scope = root?.querySelectorAll ? root : document;
  scope.querySelectorAll('iframe.html-slide-frame[data-ppt-live-mount]').forEach((frame) => {
    const mountId = frame.getAttribute('data-ppt-live-mount');
    if (!mountId) return;
    const html = pendingSlideHtmlMounts.get(mountId);
    if (!html) return;
    pendingSlideHtmlMounts.delete(mountId);
    frame.removeAttribute('data-ppt-live-mount');
    mountSandboxIframeHtml(frame, html, () => {
      fitHtmlSlideFrame(frame);
    });
  });
}

export function slideHtml(slide, options = {}) {
  if (slide?.html) {
    const mountId = `slide-${++pendingSlideHtmlMountSeq}`;
    pendingSlideHtmlMounts.set(mountId, sanitizeSlideMarkup(normalizeSlideDocument(slide.html)));
    return `<iframe class="html-slide-frame" sandbox="allow-same-origin" src="about:blank" data-ppt-live-mount="${mountId}"></iframe>`;
  }
  const editable = Boolean(options.editable);
  const selectedId = options.selectedElementId || '';
  const style = [
    `--slide-bg:${slide.theme.background}`,
    `--slide-ink:${slide.theme.ink}`,
    `--slide-muted:${slide.theme.muted}`,
    `--slide-primary:${slide.theme.primary}`,
    `--slide-accent:${slide.theme.accent}`,
    `--slide-panel:${slide.theme.panel || '#ffffff'}`,
  ].join(';');
  return `<div class="slide free-slide layout-${escapeHtml(slide.layout)}" style="${style}" data-slide-id="${escapeHtml(slide.id)}">
    ${slide.kicker ? `<div class="slide-kicker"><span></span><b>${escapeHtml(slide.kicker)}</b></div>` : ''}
    ${slide.proofObject ? `<div class="slide-proof-tag">${escapeHtml(slide.proofObject)}</div>` : ''}
    ${slideQualityBadge(slide)}
    ${(slide.elements || []).map((element) => elementModelElementHtml(element, slide.theme, {
      mode: 'editor',
      editable,
      selectedId,
      mediaPlaceholder: t('mediaPlaceholder'),
    })).join('')}
    ${slide.sourceNote ? `<div class="slide-source-note">${escapeHtml(slide.sourceNote)}</div>` : ''}
  </div>`;
}

function slideQualityBadge(slide) {
  const issues = Array.isArray(slide.quality?.issues) ? slide.quality.issues : [];
  if (!issues.length) return '';
  const highCount = issues.filter((issue) => issue.severity === 'high').length;
  const label = highCount ? t('qualityNeedsReview') : t('qualityHasWarnings');
  return `<div class="slide-quality-badge" data-severity="${highCount ? 'high' : 'medium'}">${escapeHtml(label)}</div>`;
}

function bindWelcomeTips(canvas) {
  canvas.querySelectorAll('[data-welcome-prompt]').forEach((node) => {
    node.addEventListener('click', () => {
      const input = byId('topicInput');
      if (!input) return;
      input.value = node.dataset.welcomePrompt || node.textContent || '';
      input.focus();
    });
  });
}

function bindHtmlSlideEditing(stage, slideId, handlers) {
  if (!handlers?.updateSlideHtmlDirect) return;
  const shadow = stage?.shadowRoot;
  const body = shadow?.querySelector(`.${SLIDE_SHADOW_BODY_CLASS}`);
  if (!shadow || !body) return;

  if (!shadow.querySelector(`style[${EDITING_STYLE_ATTR}]`)) {
    const style = document.createElement('style');
    style.setAttribute(EDITING_STYLE_ATTR, 'true');
    style.textContent = [
      // Slides may disable text selection globally; editing needs it back.
      `[${EDITABLE_MARK_ATTR}] { cursor: text; -webkit-user-select: text !important; user-select: text !important; }`,
      `[${EDITABLE_MARK_ATTR}]:hover { outline: 1.5px dashed rgba(37, 99, 235, 0.55); outline-offset: 1px; }`,
      `[${EDITABLE_MARK_ATTR}]:focus { outline: 2px solid rgba(37, 99, 235, 0.85); outline-offset: 1px; }`,
    ].join('\n');
    shadow.appendChild(style);
  }

  // Keep clicks inside the editable preview from navigating away.
  shadow.addEventListener('click', (event) => {
    const link = event.target?.closest?.('a[href]');
    if (link) event.preventDefault();
  }, true);

  const save = () => {
    const html = serializeEditedSlideStage(stage, body);
    if (html) handlers.updateSlideHtmlDirect(slideId, html);
  };
  const candidates = body.querySelectorAll(
    'h1,h2,h3,h4,h5,h6,p,li,span,strong,em,b,i,u,small,code,a,label,blockquote,td,th,dt,dd,figcaption,div',
  );
  let editableCount = 0;
  candidates.forEach((node) => {
    // Only nodes that directly carry text become editable; pure layout
    // wrappers stay untouched so the slide structure cannot be destroyed.
    const hasDirectText = Array.from(node.childNodes).some(
      (child) => child.nodeType === Node.TEXT_NODE && String(child.textContent || '').trim(),
    );
    if (!hasDirectText) return;
    editableCount += 1;
    node.setAttribute('contenteditable', 'true');
    node.setAttribute('spellcheck', 'false');
    node.setAttribute(EDITABLE_MARK_ATTR, 'true');
    node.addEventListener('blur', save);
    node.addEventListener('keydown', (event) => {
      if (((event.metaKey || event.ctrlKey) && event.key === 'Enter') || event.key === 'Escape') {
        event.preventDefault();
        node.blur();
      }
    });
  });
  if (!editableCount) {
    console.warn('[ppt-live] no editable nodes bound for slide', { slideId, candidates: candidates.length });
  }
}

/**
 * Serialize the edited slide by writing the shadow body's cleaned content
 * back into the original slide document. Styles, head, and html/body
 * attributes come from the untouched source HTML, so preview-only scoping
 * and editing attributes never leak into the stored slide.
 */
function serializeEditedSlideStage(stage, body) {
  const source = String(stage?._pptLiveSourceHtml || '');
  if (!source) return '';
  const parsed = new DOMParser().parseFromString(normalizeSlideDocument(source), 'text/html');
  if (!parsed.body) return '';
  const cleaned = body.cloneNode(true);
  cleaned.querySelectorAll(`[${EDITABLE_MARK_ATTR}]`).forEach((node) => {
    node.removeAttribute('contenteditable');
    node.removeAttribute('spellcheck');
    node.removeAttribute(EDITABLE_MARK_ATTR);
  });
  parsed.body.innerHTML = cleaned.innerHTML;
  const html = `<!DOCTYPE html>\n${parsed.documentElement.outerHTML}`;
  stage._pptLiveSourceHtml = html;
  return html;
}

export function normalizeSlideDocument(html) {
  const source = String(html || '').trim();
  if (!source) return '<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body></body></html>';
  if (/<!doctype|<html[\s>]/i.test(source)) return source;
  return `<!DOCTYPE html><html><head><meta charset="UTF-8"></head><body>${source}</body></html>`;
}

function byId(id) {
  return document.getElementById(id);
}

function value(id, next) {
  const node = byId(id);
  if (node && document.activeElement !== node) node.value = next ?? '';
}

function val(id) {
  return byId(id)?.value || '';
}

function text(id, value) {
  const node = byId(id);
  if (node) node.textContent = String(value ?? '');
}

function round(value) {
  return Math.round(Number(value) * 10) / 10;
}
