import { normalizeSlideDocument, scopeSlideAuthorStyles } from './render.js';
import { sanitizeSlideDocumentRoot } from './sanitize-slide-html.js';
import { extractSlideDataFromDocument, measureBodyDimensions } from './html2pptx-dom-core.js';

export const EXPORT_VIEWPORT = { width: 1280, height: 720 };

const RASTER_TEXT_SELECTOR_BY_TYPE = {
  p: ['p'],
  h1: ['h1'],
  h2: ['h2'],
  h3: ['h3'],
  h4: ['h4'],
  h5: ['h5'],
  h6: ['h6'],
  text: ['div', 'section', 'article', 'aside', 'td', 'th', 'span', 'small', 'label', 'a', 'code', 'button'],
  list: ['li'],
  'merged-text': ['span', 'em', 'strong', 'b', 'i', 'p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6'],
};

export function countVectorTextElements(slideData) {
  return (slideData?.elements || []).filter((el) => RASTER_TEXT_TYPES.has(el.type)).length;
}

function buildRasterTextHideStyle(slideData) {
  const selectors = new Set();
  for (const el of slideData?.elements || []) {
    if (!RASTER_TEXT_TYPES.has(el.type)) continue;
    for (const tag of RASTER_TEXT_SELECTOR_BY_TYPE[el.type] || [el.type]) {
      selectors.add(tag);
    }
  }
  if (!selectors.size) return '';
  const rules = [...selectors]
    .map((tag) => (
      `body[data-pptx-raster="1"] ${tag} {
  color: transparent !important;
  -webkit-text-fill-color: transparent !important;
  text-shadow: none !important;
}`
    ))
    .join('\n');
  return rules;
}

/** HTML for host WebView raster capture: hide only text that will be re-added as editable PPTX text. */
export function slideHtmlForRasterBackdrop(html, slideData = null) {
  const markup = normalizeSlideDocument(html);
  if (!slideData || countVectorTextElements(slideData) === 0) {
    return markup;
  }
  const hideCss = buildRasterTextHideStyle(slideData);
  if (!hideCss) return markup;
  if (markup.includes('data-pptx-raster="1"') && markup.includes('pptx-raster-hide-text')) {
    return markup;
  }
  const styleTag = `<style id="pptx-raster-hide-text">${hideCss}</style>`;
  if (/<\/head>/i.test(markup)) {
    return markup
      .replace(/<\/head>/i, `${styleTag}</head>`)
      .replace(/<body\b/i, '<body data-pptx-raster="1"');
  }
  return `${styleTag}${markup.replace(/<body\b/i, '<body data-pptx-raster="1"')}`;
}

const RASTER_TEXT_TYPES = new Set(['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'text', 'list', 'merged-text']);

export function filterSlideDataForRasterBackdrop(slideData) {
  return {
    ...slideData,
    elements: (slideData.elements || []).filter((el) => RASTER_TEXT_TYPES.has(el.type)),
  };
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
    querySelectorAll: (sel) => root.querySelectorAll(sel),
    createElement: (tag) => document.createElement(tag),
    getElementById: (id) => root.querySelector(`#${id}`),
    head: root.querySelector('style')?.parentElement || root,
    _exportRoot: root,
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
  const parsed = new DOMParser().parseFromString(markup, 'text/html');
  root.replaceChildren();

  parsed.querySelectorAll('style').forEach((node) => {
    const style = document.createElement('style');
    style.textContent = scopeAuthorStyles(node.textContent || '');
    root.appendChild(style);
  });

  const body = document.createElement('div');
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
export async function validateSlideForPptxGeneration(html) {
  const source = String(html || '').trim();
  const issues = [];
  const seen = new Set();
  const add = (code, message, element = null) => {
    const suffix = element ? ` (${elementLabel(element)})` : '';
    const key = `${code}:${message}${suffix}`;
    if (seen.has(key)) return;
    seen.add(key);
    issues.push({ code, message: `${message}${suffix}` });
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
      (slideData.errors || []).forEach((message) => add('pptx_conversion', message));
    } catch (error) {
      add('pptx_conversion', String(error?.message || error || 'PPTX conversion validation failed.'));
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
    sanitizeSlideDocumentRoot(doc, aggressive);
    await waitForExportPaint();

    const bodyDimensions = measureBodyDimensions(doc);
    const slideData = extractSlideDataFromDocument(doc);
    // Content overflow must never block the export: clip/off-slide content is
    // preferable to a failed run. Demote overflow findings to warnings.
    const overflowWarnings = bodyDimensions.errors || [];
    if (overflowWarnings.length) {
      console.warn('[ppt-live-export] slide overflows canvas; exporting anyway:', overflowWarnings.join('; '));
    }
    const safeBodyDimensions = { ...bodyDimensions, errors: [] };
    const errors = slideData.errors || [];
    if (!errors.length || options.allowValidationErrors) {
      return { slideData, bodyDimensions: safeBodyDimensions, aggressive, warnings: overflowWarnings };
    }
    return { error: new Error(errors.join('\n')) };
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
      let rasterBase64 = null;
      const vectorTextCount = countVectorTextElements(item.slideData);
      const rasterOnly = vectorTextCount === 0;
      if (rasterOnly && typeof options.renderRaster === 'function') {
        try {
          if (typeof options.onRasterProgress === 'function') {
            options.onRasterProgress(index, slide);
          }
          const rasterHtml = slideExportHtml(slide);
          rasterBase64 = await options.renderRaster(rasterHtml, index);
        } catch {
          rasterBase64 = null;
        }
      }
      prepared.push({
        index,
        slideId: slide.id,
        notes: slide,
        ...item,
        rasterBase64,
        rasterOnly: Boolean(rasterBase64 && rasterOnly),
      });
    }
    return prepared;
  } finally {
    clearExportSessionHost();
  }
}

export function buildElementSlideHtml(slide = {}) {
  const theme = slide.theme || {};
  const title = String(slide.title || 'Slide').replace(/[<>&]/g, (ch) => ({
    '<': '&lt;', '>': '&gt;', '&': '&amp;',
  })[ch] || ch);
  const subtitle = String(slide.subtitle || slide.claim || '').replace(/[<>&]/g, (ch) => ({
    '<': '&lt;', '>': '&gt;', '&': '&amp;',
  })[ch] || ch);
  const background = theme.background || '#ffffff';
  const ink = theme.ink || '#111111';
  const muted = theme.muted || '#666666';
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<style>
  html, body { margin: 0; padding: 0; width: ${EXPORT_VIEWPORT.width}px; height: ${EXPORT_VIEWPORT.height}px; overflow: hidden; }
  body {
    box-sizing: border-box;
    background: ${background};
    color: ${ink};
    font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    display: grid;
    align-content: center;
    gap: 16px;
    padding: 72px 96px;
  }
  h1 { margin: 0; font-size: 56px; line-height: 1.08; }
  p { margin: 0; font-size: 24px; color: ${muted}; line-height: 1.35; }
</style>
</head>
<body>
  <h1>${title}</h1>
  ${subtitle ? `<p>${subtitle}</p>` : ''}
</body>
</html>`;
}

export function slideExportHtml(slide) {
  if (slide?.html) return normalizeSlideDocument(slide.html);
  return buildElementSlideHtml(slide);
}
