// ─────────────────────────────────────────────────────────────────────────────
// Export degradation layer
//
// The editable PPTX pipeline is intentionally strict: anything that cannot be
// represented as native editable PowerPoint geometry raises a blocking
// diagnostic. Strictness is correct for *authoring-time* feedback, but at
// *export time* a single bad element must never abort the whole deck.
//
// This module converts blocking diagnostics into local repairs:
//   - unsupported styles (box-shadow, text-shadow, filter, mask, animation,
//     background images, inline margins) are stripped from the element;
//   - unrepresentable SVG constructs are simplified or removed;
//   - unrepresentable elements are removed;
//   - a slide that still fails after repair is replaced by a simplified
//     editable scene (background + text) so the exported deck keeps every page.
//
// Every applied repair is reported through `onDegrade` so the export summary
// can tell the user exactly what was adjusted.
// ─────────────────────────────────────────────────────────────────────────────
import { EditableExportError, validateEditableSlideScene } from './editable-slide-scene.js';

const MAX_REPAIRS_PER_SLIDE = 48;

function colorToHex(value, fallback = null) {
  const raw = String(value || '').trim().toLowerCase();
  if (!raw || raw === 'transparent') return fallback;
  const hex = raw.match(/^#([\da-f]{3}|[\da-f]{6})$/i)?.[1];
  if (hex) {
    return (hex.length === 3 ? hex.split('').map((c) => c + c).join('') : hex).toUpperCase();
  }
  const rgb = raw.match(/^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/i);
  if (rgb) {
    const alpha = rgb[4] == null ? 1 : (rgb[4].endsWith('%') ? Number.parseFloat(rgb[4]) / 100 : Number(rgb[4]));
    if (Number.isFinite(alpha) && alpha <= 0.05) return fallback;
    return rgb.slice(1, 4)
      .map((channel) => Math.round(Number(channel)).toString(16).padStart(2, '0'))
      .join('').toUpperCase();
  }
  return fallback;
}

/** Locate the element a blocking diagnostic refers to. */
function findElementBySourceId(doc, sourceId) {
  if (!sourceId) return null;
  const byDataId = [...doc.querySelectorAll('[data-pptx-source-id]')]
    .find((element) => element.dataset.pptxSourceId === sourceId);
  if (byDataId) return byDataId;
  try {
    return doc.querySelector(`#${(globalThis.CSS?.escape || ((v) => v))(sourceId)}`);
  } catch {
    return null;
  }
}

/** Remove one property from the sanitize-time authored-style snapshot. */
function scrubAuthoredStyle(element, properties) {
  const raw = element?.dataset?.pptxAuthoredStyle;
  if (!raw) return;
  const next = raw.split(';')
    .map((part) => part.trim())
    .filter(Boolean)
    .filter((part) => !properties.some((property) => part.toLowerCase().startsWith(`${property}:`)))
    .join('; ');
  if (next) element.dataset.pptxAuthoredStyle = next;
  else delete element.dataset.pptxAuthoredStyle;
}

/** Force a style property out with !important so class rules cannot re-apply it. */
function stripStyleProperty(element, property, properties = [property], replacement = 'none') {
  element.style?.setProperty?.(property, replacement, 'important');
  scrubAuthoredStyle(element, properties);
}

function stripStyleProperties(element, entries) {
  entries.forEach(([property, replacement]) => {
    element.style?.setProperty?.(property, replacement ?? 'none', 'important');
  });
  scrubAuthoredStyle(element, entries.map(([property]) => property));
}

/**
 * Replace an element with a repaired clone. The export pipeline re-reads
 * getComputedStyle after every repair; some WebViews (and jsdom) cache
 * computed styles per node for shadow-root content, so only a fresh node is
 * guaranteed to re-resolve. Returns the clone that replaced the original.
 */
function replaceWithRepairedClone(element, mutate) {
  const clone = element.cloneNode(true);
  mutate(clone);
  element.replaceWith(clone);
  return clone;
}

function record(code, sourceId, message) {
  return { code, sourceId: sourceId || null, message };
}

function repairSvgPaintServer(element) {
  if (!element) return false;
  let applied = false;
  for (const property of ['fill', 'stroke']) {
    if (/url\s*\(/i.test(element.getAttribute?.(property) || '')) {
      element.setAttribute(property, '#9AA3AF');
      applied = true;
    }
    if (/url\s*\(/i.test(element.style?.getPropertyValue?.(property) || '')) {
      element.style.setProperty(property, '#9AA3AF', 'important');
      applied = true;
    }
  }
  if (!applied) {
    // Stylesheet/custom-property driven paint server: detach classes and pin
    // solid fallback paint inline so the winning declaration cannot re-apply.
    element.removeAttribute('class');
    ['fill', 'stroke'].forEach((property) => {
      element.style?.setProperty?.(property, '#9AA3AF', 'important');
    });
    applied = true;
  }
  return applied;
}

function removeMatching(element, selector) {
  const matches = element?.matches?.(selector)
    ? [element]
    : [...(element?.querySelectorAll?.(selector) || [])];
  matches.forEach((node) => node.remove());
  return matches.length > 0;
}

function deleteGeneratedContentRules(doc) {
  let removed = false;
  doc.querySelectorAll('style').forEach((style) => {
    const css = style.textContent || '';
    if (!/::?(?:before|after)\b/i.test(css)) return;
    // Drop whole rules that give ::before/::after real generated content; the
    // normalizer re-reads textContent, so the text layer must be rewritten.
    const next = css.replace(/[^{}]+::?(?:before|after)\s*\{[^{}]*\}/gi, (block) => {
      if (!/\bcontent\s*:\s*(?!none\b|normal\b|"''|"")([^;}]+)/i.test(block)) return block;
      removed = true;
      return '';
    });
    if (next !== css) style.textContent = next;
  });
  return removed;
}

function stripLeadingManualBullet(element) {
  const view = element?.ownerDocument?.defaultView;
  if (!element || !view) return false;
  const walker = element.ownerDocument.createTreeWalker(element, view.NodeFilter.SHOW_TEXT);
  const textNode = walker.nextNode();
  if (!textNode) return false;
  const next = String(textNode.textContent || '').replace(/^(\s*)[•●○▪‣·▸◆◇■□]\s*/u, '$1');
  if (next === textNode.textContent) return false;
  textNode.textContent = next;
  return true;
}

function repairCanvasSize(doc, { widthPx, heightPx }) {
  const body = doc.body;
  if (!body) return false;
  body.style?.setProperty?.('width', `${widthPx}px`, 'important');
  body.style?.setProperty?.('height', `${heightPx}px`, 'important');
  body.style?.setProperty?.('max-width', `${widthPx}px`, 'important');
  body.style?.setProperty?.('max-height', `${heightPx}px`, 'important');
  scrubAuthoredStyle(body, ['width', 'height', 'max-width', 'max-height']);
  return true;
}

/**
 * Apply one local repair for a blocking diagnostic. Returns a degrade record
 * describing what changed, or null when the diagnostic is not locally
 * repairable (the caller then falls back to a simplified scene).
 */
export function applyExportDegradationRepair(doc, diagnostic, { slideNumber, widthPx, heightPx } = {}) {
  const code = String(diagnostic?.code || '');
  const sourceId = diagnostic?.sourceId || null;
  const element = findElementBySourceId(doc, sourceId)
    || (sourceId && [`slide-${slideNumber}`, 'slide-document'].includes(sourceId) ? doc.body : null);

  switch (code) {
    case 'box_shadow_unsupported': {
      if (!element) return null;
      replaceWithRepairedClone(element, (clone) => {
        stripStyleProperty(clone, 'box-shadow', ['box-shadow']);
      });
      return record('box_shadow_removed', sourceId, 'Unsupported CSS box-shadow was removed.');
    }
    case 'text_shadow_unsupported': {
      if (!element) return null;
      replaceWithRepairedClone(element, (clone) => {
        stripStyleProperty(clone, 'text-shadow', ['text-shadow']);
      });
      return record('text_shadow_removed', sourceId, 'Unsupported CSS text-shadow was removed.');
    }
    case 'css_filter': {
      if (!element) return null;
      replaceWithRepairedClone(element, (clone) => {
        stripStyleProperty(clone, 'filter', ['filter', '-webkit-filter']);
      });
      return record('css_filter_removed', sourceId, 'CSS filter was removed.');
    }
    case 'css_mask': {
      if (!element) return null;
      replaceWithRepairedClone(element, (clone) => {
        stripStyleProperties(clone, [
          ['mask'], ['mask-image'], ['-webkit-mask'], ['-webkit-mask-image'],
        ]);
        clone.removeAttribute?.('mask');
      });
      return record('css_mask_removed', sourceId, 'CSS mask was removed.');
    }
    case 'animation_unsupported': {
      if (element) {
        replaceWithRepairedClone(element, (clone) => {
          stripStyleProperties(clone, [['animation'], ['animation-name'], ['transition']]);
        });
      }
      const scope = doc;
      const removed = removeMatching(scope, 'animate, animateMotion, animateTransform, set');
      if (!element && !removed) return null;
      return record('animation_removed', sourceId, 'CSS/SVG animation was removed.');
    }
    case 'inline_margin': {
      if (!element) return null;
      replaceWithRepairedClone(element, (clone) => {
        stripStyleProperties(clone, [
          ['margin', '0'], ['margin-top', '0'], ['margin-right', '0'],
          ['margin-bottom', '0'], ['margin-left', '0'],
        ]);
      });
      return record('inline_margin_removed', sourceId, 'Unsupported inline margin was ignored.');
    }
    case 'background_image_unsupported':
    case 'merge_background_image': {
      if (element && element !== doc.body) {
        replaceWithRepairedClone(element, (clone) => {
          stripStyleProperty(clone, 'background-image', ['background-image', 'background']);
        });
        return record('background_image_removed', sourceId, 'CSS background image was removed (solid color kept).');
      }
      // Body-level background images cannot be clone-replaced (the document
      // wrapper holds a live body reference); strip in place — live WebViews
      // re-resolve computed styles, stricter DOMs fall back to the
      // simplified scene on the next attempt.
      const target = element || doc.body;
      if (!target) return null;
      stripStyleProperty(target, 'background-image', ['background-image', 'background']);
      return record('background_image_removed', sourceId || 'slide-document', 'CSS background image was removed (solid color kept).');
    }
    case 'external_resource': {
      if (!element) return null;
      if (element === doc.body) {
        stripStyleProperty(element, 'background-image', ['background-image', 'background']);
        return record('background_image_removed', sourceId || 'slide-document', 'External background resource was removed.');
      }
      element.remove();
      return record('element_removed', sourceId, 'An element referencing an external resource was removed.');
    }
    case 'svg_paint_server_unsupported': {
      if (!element) return null;
      let applied = false;
      replaceWithRepairedClone(element, (clone) => {
        applied = repairSvgPaintServer(clone);
      });
      if (!applied) return null;
      return record('svg_paint_server_removed', sourceId, 'SVG paint-server fill/stroke was replaced with a solid color.');
    }
    case 'svg_mask': {
      const svg = element?.closest?.('svg') || element;
      if (!svg) return null;
      svg.querySelectorAll('mask').forEach((node) => node.remove());
      (svg.matches?.('[mask]') ? [svg] : [...svg.querySelectorAll('[mask]')])
        .forEach((node) => node.removeAttribute('mask'));
      return record('svg_feature_removed', sourceId, 'SVG mask was removed.');
    }
    case 'svg_filter_unsupported': {
      const svg = element?.closest?.('svg') || element;
      if (!svg) return null;
      svg.querySelectorAll('filter, foreignObject, pattern, use, image')
        .forEach((node) => node.remove());
      (svg.matches?.('[filter]') ? [svg] : [...svg.querySelectorAll('[filter]')])
        .forEach((node) => node.removeAttribute('filter'));
      return record('svg_feature_removed', sourceId, 'Unsupported SVG filter/resource construct was removed.');
    }
    case 'complex_svg_unsupported': {
      const svg = element?.closest?.('svg') || element;
      if (!svg) return null;
      svg.querySelectorAll('filter, mask, foreignObject, use, pattern, textPath, clipPath, image')
        .forEach((node) => node.remove());
      return record('svg_feature_removed', sourceId, 'Unsupported SVG feature was removed.');
    }
    case 'svg_defs_geometry_unsupported': {
      if (!element) return null;
      const defs = element.closest?.('defs');
      (defs || element).remove();
      return record('svg_feature_removed', sourceId, 'SVG defs geometry was removed.');
    }
    case 'svg_transform_unsupported':
    case 'svg_polygon_unsupported':
    case 'unmeasurable_placeholder': {
      if (!element) return null;
      element.remove();
      return record('element_removed', sourceId, 'An element that cannot be represented as editable geometry was removed.');
    }
    case 'svg_round_rect_radius_unsupported': {
      if (!element) return null;
      element.removeAttribute('rx');
      element.removeAttribute('ry');
      return record('svg_feature_removed', sourceId, 'SVG rounded-rect radius was dropped to a plain rectangle.');
    }
    case 'svg_path_fill_unsupported': {
      if (!element) return null;
      const fill = element.getAttribute('fill') || element.style?.getPropertyValue?.('fill') || '#9AA3AF';
      element.setAttribute('fill', 'none');
      element.setAttribute('stroke', /^#|^rgb/i.test(fill) ? fill : '#9AA3AF');
      if (!element.getAttribute('stroke-width')) element.setAttribute('stroke-width', '1.5');
      return record('svg_path_outline', sourceId, 'A filled SVG path was converted to an editable outline.');
    }
    case 'generated_content': {
      const removedRule = deleteGeneratedContentRules(doc);
      if (element && !removedRule) element.removeAttribute('class');
      if (!removedRule && !element) return null;
      return record('generated_content_removed', sourceId, 'Pseudo-element generated content was removed.');
    }
    case 'nested_merge_container':
    case 'empty_merge_container': {
      if (!element) return null;
      element.removeAttribute('data-pptx-merge');
      element.querySelectorAll('[data-pptx-merge]').forEach((node) => node.removeAttribute('data-pptx-merge'));
      return record('merge_container_unwrapped', sourceId, 'A merge container was unwrapped into regular editable content.');
    }
    case 'manual_bullet_unrepaired': {
      if (!element || !stripLeadingManualBullet(element)) return null;
      return record('manual_bullet_removed', sourceId, 'A manual bullet character was removed.');
    }
    case 'canvas_size': {
      if (!repairCanvasSize(doc, { widthPx, heightPx })) return null;
      return record('canvas_size_adjusted', sourceId, 'The slide canvas was normalized to the editable size.');
    }
    case 'editable_scene_geometry_invalid':
    case 'editable_scene_payload_invalid':
    case 'editable_scene_node_type_unsupported':
    case 'editable_scene_source_id_invalid': {
      if (!element || element === doc.body) return null;
      element.remove();
      return record('element_removed', sourceId, 'An element with an invalid editable payload was removed.');
    }
    default:
      return null;
  }
}

/**
 * Run `normalizeFn(doc, dims)` (the strict scene normalizer) and repair the
 * document between attempts whenever a blocking diagnostic is locally
 * fixable. Throws the last EditableExportError when repairs are exhausted.
 */
export function normalizeWithDegradation(normalizeFn, doc, dims, onDegrade) {
  const { slideNumber } = dims;
  const attempted = new Set();
  let lastError = null;
  for (let repair = 0; repair < MAX_REPAIRS_PER_SLIDE; repair += 1) {
    try {
      return normalizeFn(doc, dims);
    } catch (error) {
      if (!(error instanceof EditableExportError)) throw error;
      lastError = error;
      const diagnostic = error.diagnostic || {};
      const key = `${diagnostic.code}:${diagnostic.sourceId}`;
      if (attempted.has(key)) break;
      attempted.add(key);
      const degradeRecord = applyExportDegradationRepair(doc, diagnostic, {
        slideNumber,
        widthPx: Math.round(dims.width * 96),
        heightPx: Math.round(dims.height * 96),
      });
      if (!degradeRecord) break;
      onDegrade?.({
        severity: 'degrade',
        slideNumber,
        sourceId: degradeRecord.sourceId,
        code: degradeRecord.code,
        message: degradeRecord.message,
      });
    }
  }
  throw lastError;
}

function simplifiedTextNodes(lines, slideNumber, height = 7.5) {
  const nodes = [];
  const [titleLine, ...bodyLines] = lines;
  let cursorY = 1.1;
  if (titleLine) {
    nodes.push({
      type: 'text',
      sourceId: `slide-${slideNumber}-degraded-title`,
      x: 0.9,
      y: cursorY,
      w: 11.5,
      h: 0.9,
      text: titleLine,
      paintOrder: 1,
      subOrder: 0,
      style: { fontSize: 28, bold: true, color: '1F2937' },
    });
    cursorY += 1.2;
  }
  const lineHeight = 0.62;
  const maxBodyLines = Math.max(0, Math.min(9, Math.floor((height - cursorY - 0.3) / lineHeight)));
  bodyLines.slice(0, maxBodyLines).forEach((line, index) => {
    nodes.push({
      type: 'text',
      sourceId: `slide-${slideNumber}-degraded-line-${index + 1}`,
      x: 0.9,
      y: cursorY,
      w: 11.5,
      h: 0.52,
      text: line,
      paintOrder: 2 + index,
      subOrder: 0,
      style: { fontSize: 15, color: '374151' },
    });
    cursorY += lineHeight;
  });
  return nodes;
}

/**
 * Build a minimal but fully valid editable scene for a slide that could not
 * be normalized even after repair: solid background plus plain editable text.
 * Guarantees the exported deck keeps the same page count instead of failing.
 */
export function buildSimplifiedEditableScene({
  doc = null,
  slide = null,
  slideNumber,
  width,
  height,
} = {}) {
  let backgroundHex = 'FFFFFF';
  let lines = [];
  try {
    const body = doc?.body;
    if (body) {
      const view = doc.defaultView;
      if (view?.getComputedStyle) {
        backgroundHex = colorToHex(view.getComputedStyle(body).backgroundColor, 'FFFFFF') || 'FFFFFF';
      }
      lines = [...body.querySelectorAll('h1, h2, h3, h4, h5, h6, p, li')]
        .map((element) => String(element.textContent || '').replace(/\s+/g, ' ').trim())
        .filter(Boolean);
    }
  } catch {
    lines = [];
  }
  if (!lines.length && slide) {
    lines = [
      String(slide.title || '').trim(),
      ...(Array.isArray(slide.elements) ? slide.elements : [])
        .flatMap((element) => [element?.text, ...(Array.isArray(element?.items) ? element.items : [])])
        .map((value) => String(value || '').trim())
        .filter(Boolean),
    ].filter(Boolean);
  }
  lines = [...new Set(lines)].map((line) => (line.length > 120 ? `${line.slice(0, 117)}…` : line)).slice(0, 10);
  const scene = {
    slideNumber,
    width,
    height,
    nodes: [
      {
        type: 'shape',
        shapeType: 'rect',
        sourceId: `slide-${slideNumber}-degraded-background`,
        x: 0,
        y: 0,
        w: width,
        h: height,
        paintOrder: 0,
        subOrder: 0,
        style: { fill: backgroundHex, line: null },
      },
      ...simplifiedTextNodes(lines, slideNumber, height),
    ],
  };
  try {
    return validateEditableSlideScene(scene);
  } catch {
    // The simplified scene itself must never be a new failure point: fall back
    // to the smallest valid scene (background + slide number caption).
    return validateEditableSlideScene({
      slideNumber,
      width,
      height,
      nodes: [
        scene.nodes[0],
        {
          type: 'text',
          sourceId: `slide-${slideNumber}-degraded-caption`,
          x: 0.9,
          y: 1.1,
          w: 11.5,
          h: 0.6,
          text: `Slide ${slideNumber}`,
          paintOrder: 1,
          subOrder: 0,
          style: { fontSize: 20, color: '1F2937' },
        },
      ],
    });
  }
}
