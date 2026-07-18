// ─────────────────────────────────────────────────────────────────────────────
// Slide Preparation Orchestrator
//
// prepareSlidesForPptxExport() is the entry point for the PPTX export pipeline:
//   1. Mount slide HTML in an off-screen shadow-DOM div (1280×720)
//   2. sanitizeSlideDocumentRoot() — normalize/repair the HTML for export
//   3. extractSlideDataFromDocument() — walk DOM → structured slideData
//   4. Rasterize isolated local visual layers when native/SVG mapping is unsafe
//   5. Escalate failed local captures to page-visual, then full-page fallback
//
// The prepared slideData is then passed to export-deck-browser.js →
// pptx-html-build.js for the actual PPTX generation.
// ─────────────────────────────────────────────────────────────────────────────
import { normalizeSlideDocument, scopeSlideAuthorStyles } from './render.js';
import { sanitizeSlideDocument, sanitizeSlideMarkup } from './sanitize-slide-markup.js';
import { sanitizeSlideDocumentRoot } from './sanitize-slide-html.js';
import { extractSlideDataFromDocument, measureBodyDimensions } from './html2pptx-dom-core.js';
import { buildElementSlideHtml } from './element-model-html.js';
import {
  buildPageVisualFallbackRequest,
  buildRasterFallbackRequests,
  buildWholePageVisualFallbackRequest,
  renderRasterFallbackPlan,
} from './fallback-layer-render.js';

export { buildElementSlideHtml };

export const EXPORT_VIEWPORT = { width: 1280, height: 720 };

const RASTER_TEXT_TYPES = new Set(['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'text', 'list', 'merged-text']);
const EDITABLE_TEXT_TYPES = new Set([...RASTER_TEXT_TYPES, 'svg-text']);

export function countVectorTextElements(slideData) {
  return (slideData?.elements || []).filter((el) => RASTER_TEXT_TYPES.has(el.type)).length;
}

/**
 * HTML for host WebView raster capture. Hides ALL text via universal CSS so
 * the raster contains only visual elements (backgrounds, borders, images).
 * The vector layer overlays editable text extracted from the slide.
 *
 * Previously used tag-based selective hiding which caused text overlap
 * (missed inline tags like b/strong/i/em) and text loss (over-broad
 * div/td selectors hid unextracted text). Universal hiding eliminates
 * both classes of bugs.
 */
export function slideHtmlForRasterBackdrop(html) {
  const markup = normalizeSlideDocument(html);
  if (markup.includes('data-pptx-raster="1"') && markup.includes('pptx-raster-hide-text')) {
    return markup;
  }
  const hideCss = `body[data-pptx-raster="1"], body[data-pptx-raster="1"] * {
  color: transparent !important;
  -webkit-text-fill-color: transparent !important;
  text-shadow: none !important;
}
body[data-pptx-raster="1"] ::marker {
  color: transparent !important;
  -webkit-text-fill-color: transparent !important;
}`;
  const styleTag = `<style id="pptx-raster-hide-text">${hideCss}</style>`;
  if (/<\/head>/i.test(markup)) {
    return markup
      .replace(/<\/head>/i, `${styleTag}</head>`)
      .replace(/<body\b/i, '<body data-pptx-raster="1"');
  }
  return `${styleTag}${markup.replace(/<body\b/i, '<body data-pptx-raster="1"')}`;
}

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
 * these findings as repair requirements rather than silently rasterizing or
 * flattening unsupported HTML.
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
  const add = (code, message, element = null, severity = 'fallback') => {
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
      add('canvas_size', 'The slide canvas size requires page visual fallback.', body);
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

async function validateSlideForPptxGenerationLegacy(html) {
  const source = String(html || '').trim();
  const issues = [];
  const seen = new Set();
  const add = (code, message, element = null, severity = 'fallback') => {
    const suffix = element ? ` (${elementLabel(element)})` : '';
    const key = `${code}:${message}${suffix}`;
    if (seen.has(key)) return;
    seen.add(key);
    issues.push({
      severity,
      code,
      message: `${message}${suffix}`,
      sourceId: element?.dataset?.pptxSourceId || element?.id || null,
      tag: element?.tagName?.toLowerCase?.() || null,
    });
  };

  if (!source || !/<\/html>\s*$/i.test(source)) {
    add('incomplete_html', 'The slide must be a complete HTML document ending with </html>.');
  }
  if (/<script\b/i.test(source)) {
    add('script_forbidden', 'Scripts are not allowed in editable slide HTML.');
  }
  if (/(?:linear|radial|conic|repeating-linear|repeating-radial)-gradient\s*\(/i.test(source)) {
    add('css_gradient', 'CSS gradients are unsupported; use solid fills and discrete shapes.');
  }
  if (/(?:src|href)\s*=\s*["']\s*(?:https?:)?\/\//i.test(source)) {
    add('remote_asset', 'Remote assets are not allowed; use self-contained data or local project assets.');
  }

  let exportRoot = null;
  try {
    const doc = await loadHtmlInExportRoot(source);
    exportRoot = doc._exportRoot;
    const view = doc.defaultView || window;
    const body = doc.body;
    const sourceElements = [body, ...body.querySelectorAll('*')];
    const usedSourceIds = new Set(
      sourceElements.map((element) => element.dataset.pptxSourceId).filter(Boolean),
    );
    let sourceSequence = 1;
    sourceElements.forEach((element) => {
      if (element.dataset.pptxSourceId) return;
      while (usedSourceIds.has(`pptx-source-${sourceSequence}`)) sourceSequence += 1;
      const sourceId = `pptx-source-${sourceSequence}`;
      element.dataset.pptxSourceId = sourceId;
      usedSourceIds.add(sourceId);
      sourceSequence += 1;
    });
    const bodyRect = body.getBoundingClientRect();
    const bodyDimensions = measureBodyDimensions(doc);
    bodyDimensions.errors.forEach((message) => add('canvas_overflow', message));

    const expectedWidth = EXPORT_VIEWPORT.width;
    const expectedHeight = EXPORT_VIEWPORT.height;
    if (Math.abs(bodyRect.width - expectedWidth) > 2 || Math.abs(bodyRect.height - expectedHeight) > 2) {
      add(
        'canvas_size',
        `Computed canvas must be 960pt x 540pt (${expectedWidth}px x ${expectedHeight}px); got ${bodyRect.width.toFixed(1)}px x ${bodyRect.height.toFixed(1)}px.`,
      );
    }

    body.querySelectorAll('div').forEach((div) => {
      const computed = view.getComputedStyle(div);
      if (computed.backgroundImage && computed.backgroundImage !== 'none') {
        add('div_background_image', 'DIV background-image is unsupported; use an img element.', div);
      }
    });

    const textSelector = 'p,h1,h2,h3,h4,h5,h6,li';
    body.querySelectorAll(textSelector).forEach((element) => {
      const computed = view.getComputedStyle(element);
      if (!isTransparentColor(computed.backgroundColor)
        || (computed.backgroundImage && computed.backgroundImage !== 'none')
        || hasVisibleBorder(computed)
        || (computed.boxShadow && computed.boxShadow !== 'none')) {
        add(
          'decorated_text_element',
          'Background, border, image, and shadow styling must be on an enclosing DIV shape.',
          element,
        );
      }

      const rect = element.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return;
      if (rect.left < bodyRect.left - 1
        || rect.top < bodyRect.top - 1
        || rect.right > bodyRect.right + 1
        || rect.bottom > bodyRect.bottom + 1) {
        add('text_out_of_bounds', 'A text element extends outside the slide canvas.', element);
      }
      if (parseFloat(computed.fontSize || 0) > 12 && rect.bottom > bodyRect.bottom - 48) {
        add('bottom_safety_margin', 'Text larger than 12px must keep a 36pt bottom safety margin.', element);
      }
    });

    body.querySelectorAll('span,em,strong,b,i,u,a,small,mark,sub,sup,code').forEach((element) => {
      const computed = view.getComputedStyle(element);
      const hasBoxSpacing = [
        'marginTop', 'marginRight', 'marginBottom', 'marginLeft',
        'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft',
      ].some((prop) => parseFloat(computed[prop] || 0) > 0);
      if (hasBoxSpacing
        || !isTransparentColor(computed.backgroundColor)
        || (computed.backgroundImage && computed.backgroundImage !== 'none')
        || hasVisibleBorder(computed)
        || (computed.boxShadow && computed.boxShadow !== 'none')) {
        add('unsafe_inline_style', 'Inline text elements cannot carry box spacing, fills, borders, or shadows.', element);
      }
    });

    body.querySelectorAll('*').forEach((element) => {
      const computed = view.getComputedStyle(element);
      if (String(computed.backgroundImage || '').includes('gradient')) {
        add('computed_gradient', 'Computed CSS contains an unsupported gradient.', element);
      }
      for (const pseudo of ['::before', '::after']) {
        try {
          const content = view.getComputedStyle(element, pseudo)?.content;
          if (content && content !== 'none' && content !== 'normal' && content !== '""') {
            add('generated_content', `${pseudo} generated text/content is unsupported for editable PPTX.`, element);
          }
        } catch {
          // Some WebViews do not expose pseudo-element computed styles.
        }
      }
    });

    try {
      const slideData = extractSlideDataFromDocument(doc);
      (slideData.diagnostics || []).forEach((diagnostic) => {
        const key = `${diagnostic.code}:${diagnostic.message}:${diagnostic.sourceId || ''}`;
        if (seen.has(key)) return;
        seen.add(key);
        issues.push(diagnostic);
      });
    } catch (error) {
      add(
        'pptx_serialization',
        String(error?.message || error || 'PPTX conversion validation failed.'),
        null,
        'blocking',
      );
    }
  } finally {
    if (exportRoot) removeExportRoot(exportRoot);
  }

  return {
    valid: issues.length === 0,
    issues: issues.slice(0, 32),
  };
}

async function prepareSlideOnce(html, aggressive, options = {}) {
  let exportRoot = null;
  try {
    const doc = await loadHtmlInExportRoot(html);
    exportRoot = doc._exportRoot;
    const repairResult = sanitizeSlideDocumentRoot(doc, aggressive);
    await waitForExportPaint();

    const bodyDimensions = measureBodyDimensions(doc);
    const slideData = extractSlideDataFromDocument(doc);
    const analysis = analyzeMountedSlideForPptx(doc, html);
    const mergedDiagnostics = [
      ...(analysis.issues || []),
      ...(repairResult?.diagnostics || []),
      ...(slideData.diagnostics || []),
    ];
    const diagnosticKeys = new Set();
    const diagnostics = mergedDiagnostics.filter((diagnostic) => {
      const key = `${diagnostic.severity}:${diagnostic.code}:${diagnostic.sourceId || ''}`;
      if (diagnosticKeys.has(key)) return false;
      diagnosticKeys.add(key);
      return true;
    });
    slideData.diagnostics = diagnostics;
    const rasterRequests = buildRasterFallbackRequests(doc, diagnostics);
    const pageFallbackCodes = new Set([
      'canvas_size', 'canvas_overflow', 'text_out_of_bounds', 'bottom_safety_margin',
    ]);
    const pageFallbackDiagnostics = diagnostics.filter((item) => pageFallbackCodes.has(item.code));
    const nativeVisualSourceIds = [...new Set(
      (slideData.elements || [])
        .filter((element) => !EDITABLE_TEXT_TYPES.has(element.type))
        .map((element) => element.sourceId)
        .filter(Boolean),
    )];
    const pageVisualRequest = pageFallbackDiagnostics.length
      ? buildWholePageVisualFallbackRequest(
        doc,
        pageFallbackDiagnostics,
        nativeVisualSourceIds,
      )
      : buildPageVisualFallbackRequest(doc, rasterRequests);
    const overflowWarnings = bodyDimensions.errors || [];
    const safeBodyDimensions = { ...bodyDimensions, errors: [] };
    const blocking = diagnostics.filter((diagnostic) => diagnostic.severity === 'blocking');
    if (!blocking.length || options.allowValidationErrors) {
      return {
        slideData,
        bodyDimensions: safeBodyDimensions,
        diagnostics,
        rasterRequests,
        pageVisualRequest,
        aggressive,
        warnings: overflowWarnings,
      };
    }
    const error = new Error(blocking.map((diagnostic) => diagnostic.message).join('\n'));
    error.diagnostics = blocking;
    return { error };
  } finally {
    if (exportRoot) removeExportRoot(exportRoot);
  }
}

export async function prepareSlideForPptxExport(html, options = {}) {
  const first = await prepareSlideOnce(html, false, options);
  if (first?.slideData) return first;

  const second = await prepareSlideOnce(html, true, options);
  if (second?.slideData) return second;
  throw second?.error || first?.error || new Error('PPT Live slide preparation failed');
}

export async function prepareSlidesForPptxExport(slides, options = {}) {
  const prepared = [];
  try {
    for (const [index, slide] of slides.entries()) {
      if (!slide?.html) continue;
      const item = await prepareSlideForPptxExport(slide.html, options);
      if (item.rasterRequests?.length && typeof options.onRasterProgress === 'function') {
        options.onRasterProgress(index, slide);
      }
      const rasterResult = await renderRasterFallbackPlan({
        localRequests: item.rasterRequests || [],
        pageVisualRequest: item.pageVisualRequest,
        fullPageRequest: {
          sourceId: `slide-${index + 1}`,
          zIndex: 0,
          paintOrder: 0,
          kind: 'raster',
          phase: 'full-page',
          bbox: { x: 0, y: 0, w: 13.333, h: 7.5 },
          buildHtml: () => slideExportHtml(slide),
          diagnostics: [],
        },
      }, options.renderRaster, index);
      if (rasterResult.blocking) {
        const error = new Error(
          rasterResult.diagnostics.map((diagnostic) => diagnostic.message).join('\n')
            || `Slide ${index + 1} fallback rendering failed`,
        );
        error.diagnostics = rasterResult.diagnostics;
        throw error;
      }
      item.slideData.fallbackLayers = [
        ...(item.slideData.fallbackLayers || []),
        ...rasterResult.layers,
      ];
      item.slideData.fullPageFallback = rasterResult.fullPageFallback;
      item.slideData.diagnostics = [
        ...(item.slideData.diagnostics || []),
        ...rasterResult.diagnostics,
      ];
      prepared.push({
        index,
        slideId: slide.id,
        notes: slide,
        ...item,
        fallbackDiagnostics: rasterResult.diagnostics,
      });
    }
    return prepared;
  } finally {
    clearExportSessionHost();
  }
}

export function slideExportHtml(slide) {
  if (slide?.html) return sanitizeSlideMarkup(normalizeSlideDocument(slide.html));
  return sanitizeSlideMarkup(buildElementSlideHtml(slide));
}
