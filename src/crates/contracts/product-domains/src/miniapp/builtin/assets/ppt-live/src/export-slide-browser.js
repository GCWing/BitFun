// ─────────────────────────────────────────────────────────────────────────────
// Slide Preparation Orchestrator
//
// prepareEditableSlides() is the entry point for the PPTX export pipeline:
//   1. Mount slide HTML in an off-screen shadow-DOM div (1280×720)
//   2. sanitizeSlideDocumentRoot() — normalize/repair the HTML for export
//   3. normalizeDocumentToEditableScene() — rewrite DOM into EditableSlideScene
//   4. Reject any input that cannot be represented as editable native objects
//
// The scenes are passed to export-deck-browser.js → pptx-html-build.js.
// ─────────────────────────────────────────────────────────────────────────────
import { normalizeSlideDocument, scopeSlideAuthorStyles } from './render.js';
import { sanitizeSlideDocument, sanitizeSlideMarkup } from './sanitize-slide-markup.js';
import { sanitizeSlideDocumentRoot } from './sanitize-slide-html.js';
import { extractSlideDataFromDocument, measureBodyDimensions } from './html2pptx-dom-core.js';
import { buildElementSlideHtml } from './element-model-html.js';
import { normalizeDocumentToEditableScene } from './editable-slide-normalize.js';
import { normalizeElementSlideToEditableScene } from './pptx-element-export.js';
import { auditRawSlideForEditableExport } from './preflight-slide-audit.js';

export { buildElementSlideHtml };

export const EXPORT_VIEWPORT = { width: 1280, height: 720 };

let exportSessionHost = null;

function getExportSessionHost() {
  if (!exportSessionHost?.isConnected) {
    exportSessionHost = document.createElement('div');
    exportSessionHost.id = 'ppt-export-session-host';
    exportSessionHost.setAttribute('aria-hidden', 'true');
    exportSessionHost.style.cssText = [
      'position:fixed',
      'left:-24000px',
      'top:0',
      'width:1px',
      'height:1px',
      'overflow:hidden',
      'opacity:0',
      'pointer-events:none',
      'z-index:-1',
      'contain:strict',
    ].join(';');
    document.body.appendChild(exportSessionHost);
  }
  return exportSessionHost;
}

export function clearExportSessionHost() {
  if (exportSessionHost?.isConnected) {
    exportSessionHost.replaceChildren('');
  }
}

function scopeAuthorStyles(cssText) {
  return scopeSlideAuthorStyles(cssText, '.ppt-export-root', '.ppt-export-body');
}

function wrapExportDocument(root, body) {
  return {
    body,
    documentElement: root,
    defaultView: window,
    querySelector: (sel) => root.querySelector(sel),
    querySelectorAll: (sel) => root.querySelectorAll(sel),
    createElement: (tag) => document.createElement(tag),
    createTreeWalker: (...args) => document.createTreeWalker(...args),
    getElementById: (id) => root.querySelector(`#${id}`),
    head: root.querySelector('style')?.parentElement || root,
    _exportRoot: root,
    _pptxSecurityDiagnostics: body._pptxSecurityDiagnostics || [],
  };
}

function createExportRoot() {
  // Mount the slide inside a shadow root so its author styles (e.g. `* { ... }`,
  // `p { ... }`, `table { ... }`) cannot leak into the app document. Leaked rules
  // used to restyle the whole UI for a frame on every exported page, which made
  // the export modal visibly jump.
  const host = document.createElement('div');
  host.className = 'ppt-export-root-host';
  host.setAttribute('aria-hidden', 'true');
  host.style.cssText = [
    `width:${EXPORT_VIEWPORT.width}px`,
    `height:${EXPORT_VIEWPORT.height}px`,
    'overflow:hidden',
  ].join(';');
  getExportSessionHost().appendChild(host);
  const shadow = host.attachShadow({ mode: 'open' });
  const root = document.createElement('div');
  root.className = 'ppt-export-root';
  root.style.cssText = [
    `width:${EXPORT_VIEWPORT.width}px`,
    `height:${EXPORT_VIEWPORT.height}px`,
    'overflow:hidden',
  ].join(';');
  shadow.appendChild(root);
  root._exportHost = host;
  return root;
}

function removeExportRoot(root) {
  const host = root?._exportHost || root;
  if (host?.isConnected) host.remove();
}

async function waitForExportPaint() {
  await new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(resolve));
  });
}

function mountMarkupOnRoot(root, markup) {
  const parsed = sanitizeSlideDocument(new DOMParser().parseFromString(markup, 'text/html'));
  root.replaceChildren();

  parsed.querySelectorAll('style').forEach((node) => {
    const style = document.createElement('style');
    style.textContent = scopeAuthorStyles(node.textContent || '');
    root.appendChild(style);
  });

  const body = document.createElement('div');
  body._pptxSecurityDiagnostics = parsed._pptxSecurityDiagnostics || [];
  body.className = 'ppt-export-body';
  if (parsed.body) {
    for (const attr of parsed.body.attributes) {
      if (attr.name === 'class') {
        body.classList.add(...attr.value.split(/\s+/).filter(Boolean));
      } else if (attr.name === 'style') {
        body.style.cssText += `;${attr.value}`;
      } else {
        body.setAttribute(attr.name, attr.value);
      }
    }
    body.innerHTML = parsed.body.innerHTML;
  }
  body.style.boxSizing = 'border-box';
  if (!/\bwidth\s*:/i.test(body.style.cssText)) {
    body.style.width = `${EXPORT_VIEWPORT.width}px`;
  }
  if (!/\bheight\s*:/i.test(body.style.cssText)) {
    body.style.height = `${EXPORT_VIEWPORT.height}px`;
  }
  root.appendChild(body);
  return body;
}

async function loadHtmlInExportRoot(html) {
  const markup = normalizeSlideDocument(html);
  const root = createExportRoot();
  const body = mountMarkupOnRoot(root, markup);
  await waitForExportPaint();
  return wrapExportDocument(root, body);
}

function hasVisibleBorder(computed) {
  return ['Top', 'Right', 'Bottom', 'Left'].some(
    (side) => parseFloat(computed[`border${side}Width`] || 0) > 0,
  );
}

function isTransparentColor(value) {
  return !value || value === 'transparent' || value === 'rgba(0, 0, 0, 0)';
}

function elementLabel(element) {
  const id = element.id ? `#${element.id}` : '';
  const className = typeof element.className === 'string'
    ? element.className.trim().split(/\s+/).filter(Boolean).slice(0, 2).map((name) => `.${name}`).join('')
    : '';
  return `${element.tagName.toLowerCase()}${id}${className}`;
}

/**
 * Validate the authored slide before export sanitization. Generation treats
 * these findings as repair requirements rather than flattening unsupported HTML.
 */
export function analyzeMountedSlideForPptx(doc, source = '') {
  if (!doc?.body) {
    return {
      valid: false,
      issues: [{
        severity: 'blocking',
        kind: 'blocking',
        code: 'unreadable_document',
        message: 'The slide document could not be read.',
        sourceId: 'slide-document',
      }],
    };
  }
  const issues = [...(doc._pptxSecurityDiagnostics || [])];
  const seen = new Set(issues.map((item) => `${item.code}:${item.sourceId || ''}`));
  const add = (code, message, element = null, severity = 'blocking') => {
    const sourceId = element?.dataset?.pptxSourceId || element?.id || null;
    const key = `${code}:${sourceId || ''}`;
    if (seen.has(key)) return;
    seen.add(key);
    issues.push({
      severity,
      kind: severity === 'blocking' ? 'blocking' : undefined,
      code,
      message,
      sourceId,
      tag: element?.tagName?.toLowerCase?.() || null,
    });
  };
  const body = doc.body;
  if (body.querySelector('script,iframe,object,embed,base,meta[http-equiv="refresh" i],foreignObject,maction')) {
    add('active_content_residual', 'Active content remained after sanitization.', body, 'blocking');
  }
  if (!String(source || '').trim() || !/<\/html>\s*$/i.test(String(source || '').trim())) {
    add('incomplete_html', 'The slide document is incomplete.', body, 'blocking');
  }
  let bodyRect;
  try {
    bodyRect = body.getBoundingClientRect();
    if (!(bodyRect.width > 0) || !(bodyRect.height > 0)) {
      add('unmeasurable_canvas', 'The slide canvas could not be measured.', body, 'blocking');
    }
  } catch {
    add('unmeasurable_canvas', 'The slide canvas could not be measured.', body, 'blocking');
  }
  if (bodyRect) {
    if (Math.abs(bodyRect.width - EXPORT_VIEWPORT.width) > 2
      || Math.abs(bodyRect.height - EXPORT_VIEWPORT.height) > 2) {
      add('canvas_size', 'The slide canvas size does not match the editable export canvas.', body);
    }
    const dimensions = measureBodyDimensions(doc);
    if (dimensions.errors?.length) {
      add('canvas_overflow', 'Slide content exceeds the canvas.', body);
    }
    const view = doc.defaultView || window;
    body.querySelectorAll('p,h1,h2,h3,h4,h5,h6,li').forEach((element) => {
      const rect = element.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return;
      if (rect.left < bodyRect.left - 1 || rect.top < bodyRect.top - 1
        || rect.right > bodyRect.right + 1 || rect.bottom > bodyRect.bottom + 1) {
        add('text_out_of_bounds', 'Text extends outside the slide canvas.', element);
      }
      const computed = view.getComputedStyle(element);
      if (parseFloat(computed.fontSize || 0) > 12 && rect.bottom > bodyRect.bottom - 48) {
        add('bottom_safety_margin', 'Text enters the bottom safety margin.', element);
      }
    });
  }
  return { valid: issues.length === 0, issues: issues.slice(0, 32) };
}

export async function validateSlideForPptxGeneration(html) {
  let exportRoot = null;
  try {
    const doc = await loadHtmlInExportRoot(html);
    exportRoot = doc._exportRoot;
    sanitizeSlideDocumentRoot(doc);
    await waitForExportPaint();
    return analyzeMountedSlideForPptx(doc, html);
  } finally {
    if (exportRoot) removeExportRoot(exportRoot);
  }
}

async function prepareHtmlSlide(html, slideNumber) {
  let exportRoot = null;
  try {
    auditRawSlideForEditableExport(html, slideNumber);
    const mountedDoc = await loadHtmlInExportRoot(html);
    exportRoot = mountedDoc._exportRoot;
    sanitizeSlideDocumentRoot(mountedDoc);
    await waitForExportPaint();
    extractSlideDataFromDocument(mountedDoc);
    return normalizeDocumentToEditableScene(mountedDoc, {
      slideNumber,
      width: EXPORT_VIEWPORT.width / 96,
      height: EXPORT_VIEWPORT.height / 96,
    });
  } finally {
    if (exportRoot) removeExportRoot(exportRoot);
  }
}

export async function prepareEditableSlides(slides, options = {}) {
  const scenes = [];
  try {
    for (const [index, slide] of slides.entries()) {
      if (typeof options.onSlideProgress === 'function') {
        options.onSlideProgress(index + 1, slide);
      }
      scenes.push(slide?.html
        ? await prepareHtmlSlide(slide.html, index + 1)
        : normalizeElementSlideToEditableScene(slide, { slideNumber: index + 1 }));
    }
    return scenes;
  } catch (error) {
    if (error?.diagnostic && !error.diagnostics) {
      error.diagnostic.severity = 'blocking';
      error.diagnostic.kind = 'blocking';
      error.diagnostics = [error.diagnostic];
    }
    throw error;
  } finally {
    clearExportSessionHost();
  }
}

export function slideExportHtml(slide) {
  if (slide?.html) return sanitizeSlideMarkup(normalizeSlideDocument(slide.html));
  return sanitizeSlideMarkup(buildElementSlideHtml(slide));
}
