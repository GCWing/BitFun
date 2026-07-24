// ─────────────────────────────────────────────────────────────────────────────
// EditableSlideScene → PPTX Build
//
// buildSlideFromScene() validates and serializes only scene.nodes.
//
// Key design decisions:
//   width safety — text boxes are widened to absorb cross-renderer font metric
//     drift (PowerPoint renders CJK glyphs slightly wider than browsers). The
//     safety scales with font size (calibrated as ~0.36" at 14pt ≈ ~2.4 CJK em);
//     a fixed margin under-compensates large titles and causes false wraps.
//     safeTextBoxGeometry() shifts x for right/center align so the extra width
//     does not move the visual anchor.
//   margin — set from the element's CSS padding so PPTX internal inset matches
//     the HTML box model (prevents text from shifting toward top-left).
//   charSpacing — CSS letter-spacing is preserved so negative tracking in the
//     preview cannot become looser (and wrap) in PowerPoint.
//   valign — resolved from CSS flex/grid align-items or line-height ratio.
// ─────────────────────────────────────────────────────────────────────────────
import pptxgen from 'pptxgenjs';
import JSZip from 'jszip';
import { EditableExportError, validateEditableSlideScene } from './editable-slide-scene.js';

const SLIDE_W_IN = 13.333;
const SLIDE_H_IN = 7.5;
const OOXML_ROUND_RECT_ADJUSTMENTS = Symbol('ppt-live-round-rect-adjustments');

// Cross-platform PPTX fonts. PingFang SC is macOS-only — Windows PowerPoint
// substitutes poorly after a repair pass and CJK runs often become mojibake.
// Microsoft YaHei is present on Windows Office and maps cleanly on Mac/Linux
// Office / LibreOffice; Arial covers Latin glyphs on all three platforms.
export const PPTX_LATIN_FONT_FACE = 'Arial';
export const PPTX_CJK_FONT_FACE = 'Microsoft YaHei';

const EMPTY_SHAPE_TX_BODY = '<p:txBody><a:bodyPr/><a:lstStyle/><a:p>'
  + `<a:endParaRPr lang="zh-CN"/></a:p></p:txBody>`;

const PLATFORM_CJK_FONT_ALIASES = new Set([
  'pingfang sc',
  'pingfang tc',
  'hiragino sans gb',
  'hiragino sans',
  'stheiti',
  'heiti sc',
  'heiti tc',
  'songti sc',
  'source han sans sc',
  'source han sans cn',
  'noto sans cjk sc',
  'noto sans sc',
  'wenquanyi micro hei',
  'wenquanyi zen hei',
  'droid sans fallback',
  '微软雅黑',
  'microsoft yahei',
  'microsoft yahei ui',
  'simhei',
  'simsun',
  'nsimsun',
  'kaiti',
  'fangsong',
]);

// PowerPoint and browsers render the same font at the same point size with
// measurably different glyph widths (different font metric tables / hinting).
// For CJK text the drift is amplified because every glyph is full-width:
// if a line of CJK text *barely* fits on one line in the browser, PowerPoint's
// slightly wider rendering pushes the last character to the next line.
//
// Calibration: 0.36" ≈ ~2.4 CJK em at 14pt body text. Prefer a generous
// right-side slack over false wraps — empty padding rarely hurts layout.
// Safety must scale with font size — a 42pt title needs ~3× that margin.
// Boxes may extend past the slide edge; PowerPoint allows off-slide shapes,
// and clipping the safety margin would reintroduce wraps near the right edge.
//
// IMPORTANT: the safety width widens the text box to prevent wrapping, but
// for right/center-aligned text this alone would shift the rendered glyphs
// (right edge moves right, center moves right).  The callers compensate by
// adjusting the x coordinate so that the *original* text region is preserved.
const WIDTH_SAFETY_BASE_IN = 0.36;
const WIDTH_SAFETY_REF_PT = 14;
const WIDTH_SAFETY_MIN_IN = 0.28;
const WIDTH_SAFETY_MAX_IN = 1.6;

export function textBoxWidthSafetyInches(fontSizePt) {
  const size = Math.max(8, Number(fontSizePt) || WIDTH_SAFETY_REF_PT);
  const scaled = WIDTH_SAFETY_BASE_IN * (size / WIDTH_SAFETY_REF_PT);
  return Math.min(WIDTH_SAFETY_MAX_IN, Math.max(WIDTH_SAFETY_MIN_IN, scaled));
}

function resolveTextFontSizePt(el) {
  const base = Number(el?.style?.fontSize) || WIDTH_SAFETY_REF_PT;
  if (!Array.isArray(el?.text)) return base;
  const runSizes = el.text
    .map((run) => Number(run?.options?.fontSize) || 0)
    .filter((size) => size > 0);
  return runSizes.length ? Math.max(base, ...runSizes) : base;
}

function textUsesBold(el) {
  if (el?.style?.bold) return true;
  if (!Array.isArray(el?.text)) return false;
  return el.text.some((run) => run?.options?.bold);
}

// Given an element's original x/w and its text-align, return {x, w} for the
// PPTX text box.  The box is widened by a font-size-scaled safety margin to
// prevent wrapping, but the x coordinate is shifted so the original
// left/right/center anchor stays in the same visual position:
//   left   → extra width extends to the right (x unchanged)
//   right  → extra width extends to the left  (x shifts left by safety)
//   center → split equally                    (x shifts left by safety/2)
export function safeTextBoxGeometry(origX, origW, align, isVerticalText, fontSizePt, options = {}) {
  if (isVerticalText) return { x: origX, w: Math.max(0.15, origW) };
  let safety = textBoxWidthSafetyInches(fontSizePt);
  // Faux / true bold can widen CJK runs by a few percent beyond the base drift.
  if (options.bold) safety *= 1.12;
  const w = Math.max(0.15, origW + safety);
  if (align === 'left' || !align) {
    return { x: origX, w };
  }
  if (align === 'right') {
    return { x: Math.max(0, origX - safety), w };
  }
  if (align === 'center') {
    return { x: Math.max(0, origX - safety / 2), w };
  }
  // justify behaves like left for anchoring
  return { x: origX, w };
}

function toImagePayload(src) {
  const raw = String(src || '').trim();
  if (!/^data:image\/(?:png|jpeg|webp);base64,[A-Za-z0-9+/]+={0,2}$/i.test(raw)) {
    throw new Error('Intentional images must use an inline base64 PNG, JPEG, or WebP data URL');
  }
  return { data: raw };
}

function tableCellBorders(border = {}) {
  const uniform = border.color
    ? { color: border.color, width: border.width || 0 }
    : null;
  return ['top', 'right', 'bottom', 'left'].map((side) => {
    const value = border[side] || uniform || { color: '000000', width: 0 };
    return {
      type: value.width > 0 ? 'solid' : 'none',
      color: value.color,
      pt: value.width,
    };
  });
}

// CSS system / private font names are not installable PowerPoint typefaces.
// Mapping them to the deck CJK body font keeps <a:ea> usable after export.
const CSS_SYSTEM_FONT_FACES = new Set([
  'system-ui',
  '-apple-system',
  'blinkmacsystemfont',
  'ui-sans-serif',
  'ui-serif',
  'ui-monospace',
  'sans-serif',
  'serif',
  'monospace',
  'emoji',
  'math',
  'fangsong',
  '.applesystemuifont',
  '.sf ns text',
  '.sf ns display',
]);

function textHintContainsCjk(textHint) {
  return /[\u4e00-\u9fff]/.test(String(textHint || ''));
}

function isPlatformCjkFontFace(fontFace) {
  const lower = String(fontFace || '').replace(/['"]/g, '').trim().toLowerCase();
  return PLATFORM_CJK_FONT_ALIASES.has(lower);
}

/**
 * Resolve a CSS/computed face to a single pptxgenjs fontFace.
 * PptxGenJS writes the same typeface into latin/ea/cs; postProcess then splits
 * CJK faces into Arial (latin/cs) + Microsoft YaHei (ea) for Win/Mac/Linux.
 */
export function resolvePptxFontFace(fontFace, textHint = '') {
  const face = String(fontFace || '').replace(/['"]/g, '').trim();
  if (!face) {
    return textHintContainsCjk(textHint) ? PPTX_CJK_FONT_FACE : PPTX_LATIN_FONT_FACE;
  }
  const lower = face.toLowerCase();
  if (CSS_SYSTEM_FONT_FACES.has(lower) || lower.startsWith('.')) {
    return textHintContainsCjk(textHint) ? PPTX_CJK_FONT_FACE : PPTX_LATIN_FONT_FACE;
  }
  if (isPlatformCjkFontFace(face) || lower === 'aptos') {
    // Aptos is Windows 11 Office-only; map to the cross-platform CJK stack when
    // the run may contain CJK, otherwise keep Latin-safe Arial.
    if (lower === 'aptos') {
      return textHintContainsCjk(textHint) ? PPTX_CJK_FONT_FACE : PPTX_LATIN_FONT_FACE;
    }
    return PPTX_CJK_FONT_FACE;
  }
  // PptxGenJS writes the same typeface into latin/ea/cs. Latin-only faces such
  // as Arial therefore lock East-Asian glyphs to tofu after PowerPoint opens
  // (or repairs) the table. Prefer the deck CJK body font for CJK runs.
  if (
    textHintContainsCjk(textHint)
    && (lower === 'arial' || lower === 'helvetica' || lower === 'times new roman'
      || lower === 'calibri' || lower === 'georgia' || lower === 'verdana'
      || lower === 'tahoma' || lower === 'trebuchet ms')
  ) {
    return PPTX_CJK_FONT_FACE;
  }
  return face;
}

/** Split a resolved face into OOXML latin / ea / cs typefaces. */
export function resolveCrossPlatformFontPair(fontFace) {
  const face = String(fontFace || '').replace(/['"]/g, '').trim();
  if (!face) {
    return {
      latin: PPTX_LATIN_FONT_FACE,
      ea: PPTX_CJK_FONT_FACE,
      cs: PPTX_LATIN_FONT_FACE,
    };
  }
  if (isPlatformCjkFontFace(face) || face === PPTX_CJK_FONT_FACE) {
    return {
      latin: PPTX_LATIN_FONT_FACE,
      ea: PPTX_CJK_FONT_FACE,
      cs: PPTX_LATIN_FONT_FACE,
    };
  }
  // Keep designer / Latin faces for Western glyphs; always give CJK a fallback.
  return {
    latin: face,
    ea: PPTX_CJK_FONT_FACE,
    cs: face,
  };
}

function withResolvedFontFace(options = {}, textHint = '') {
  if (!options || typeof options !== 'object') return options;
  if (options.fontFace == null && options.fontFamily == null) {
    return textHintContainsCjk(textHint)
      ? { ...options, fontFace: PPTX_CJK_FONT_FACE }
      : options;
  }
  return {
    ...options,
    fontFace: resolvePptxFontFace(options.fontFace || options.fontFamily, textHint),
  };
}

function resolveTableText(text) {
  if (!Array.isArray(text)) return text;
  return text.map((run) => {
    if (!run || typeof run !== 'object') return run;
    return {
      ...run,
      options: withResolvedFontFace(run.options || {}, run.text),
    };
  });
}

function addEditableTable(table, targetSlide) {
  const rows = table.rows.map((row) => row.cells.map((cell) => {
    const style = cell.style || {};
    const cellTextHint = Array.isArray(cell.text)
      ? cell.text.map((run) => run?.text || '').join('')
      : cell.text;
    const options = {
      fill: style.fill == null
        ? { color: 'FFFFFF', transparency: 100 }
        : style.fill,
      border: tableCellBorders(style.border),
      align: style.align,
      valign: style.valign,
      fontFace: resolvePptxFontFace(style.fontFamily, cellTextHint),
      fontSize: style.fontSize,
      color: style.fontColor,
      bold: style.bold,
      margin: Array.isArray(style.padding)
        ? style.padding.map((inches) => inches * 72)
        : 0,
      ...(cell.colspan > 1 ? { colspan: cell.colspan } : {}),
      ...(cell.rowspan > 1 ? { rowspan: cell.rowspan } : {}),
    };
    return { text: resolveTableText(cell.text), options };
  }));
  targetSlide.addTable(rows, {
    x: table.x,
    y: table.y,
    w: table.w,
    h: table.h,
    colW: table.columnWidths,
    rowH: table.rows.map((row) => row.height),
    autoFit: false,
    autoPage: false,
  });
}

function pptxLineStyle(style = {}) {
  const { dash, ...line } = style;
  return {
    ...line,
    ...(dash ? { dashType: dash } : {}),
  };
}

function pptxTextMargin(margin) {
  if (!Array.isArray(margin)) return Number.isFinite(margin) ? margin * 72 : 0;
  const [top, right, bottom, left] = margin;
  // EditableSlideScene uses CSS order. PptxGenJS 4.0.1's text serializer reads
  // its four-value array as left/right/bottom/top and expects point values.
  return [left, right, bottom, top].map((inches) => inches * 72);
}

function pptxTextValue(value) {
  if (!Array.isArray(value)) return value;
  return value.map((run) => {
    const options = withResolvedFontFace({ ...(run.options || {}) }, run.text);
    if (options.bullet?.type === 'bullet') {
      const bullet = { ...options.bullet };
      delete bullet.type;
      options.bullet = bullet;
    }
    return {
      ...run,
      options,
    };
  });
}

function addSceneNodes(slideData, targetSlide, pres) {
  const paintItems = slideData.nodes.map((item, stableOrder) => ({
    type: 'element',
    item,
    zIndex: item.zIndex ?? 0,
    order: item.paintOrder ?? stableOrder,
    subOrder: item.subOrder ?? 0,
    stableOrder,
  })).sort((left, right) => (
    left.zIndex - right.zIndex
    || left.order - right.order
    || left.subOrder - right.subOrder
    || left.stableOrder - right.stableOrder
  ));
  for (const paintItem of paintItems) {
    const el = paintItem.item;
    if (el.type === 'table') {
      addEditableTable(el, targetSlide);
    } else if (el.type === 'image') {
      try {
        const payload = toImagePayload(el.src || el.path || el.data);
        if (!payload) throw new Error(`Intentional image "${el.sourceId}" has no payload`);
        targetSlide.addImage({
          ...payload,
          x: el.x,
          y: el.y,
          w: el.w,
          h: el.h,
        });
      } catch (cause) {
        throw new EditableExportError({
          slideNumber: slideData.slideNumber,
          sourceId: el.sourceId,
          code: 'pptx_image_serialization',
          message: `Intentional image "${el.sourceId}" could not be serialized.`,
          cause,
        });
      }
    } else if (el.type === 'line') {
      targetSlide.addShape(pres.ShapeType.line, {
        x: el.x1,
        y: el.y1,
        w: el.x2 - el.x1,
        h: el.y2 - el.y1,
        line: pptxLineStyle(el.style),
      });
    } else if (el.type === 'shape') {
      const shapeOptions = {
        x: el.x,
        y: el.y,
        w: el.w,
        h: el.h,
      };
      shapeOptions.shape = pres.ShapeType[el.shapeType];
      if (!shapeOptions.shape) {
        throw new Error(`Unsupported native shape type "${el.shapeType}"`);
      }
      if (el.style.fill) {
        shapeOptions.fill = { color: el.style.fill };
        if (el.style.transparency != null) {
          shapeOptions.fill.transparency = el.style.transparency;
        }
      }
      if (el.style.line) shapeOptions.line = pptxLineStyle(el.style.line);
      if (el.style.shadow) shapeOptions.shadow = el.style.shadow;
      if (el.style.rotate != null) shapeOptions.rotate = el.style.rotate;
      targetSlide.addShape(shapeOptions.shape, shapeOptions);
      if (el.shapeType === 'roundRect') {
        const adjustment = Number.isFinite(el.style.radius)
          ? Math.round((el.style.radius / Math.min(el.w, el.h)) * 100000)
          : 16667;
        if (!Array.isArray(pres[OOXML_ROUND_RECT_ADJUSTMENTS])) {
          pres[OOXML_ROUND_RECT_ADJUSTMENTS] = [];
        }
        pres[OOXML_ROUND_RECT_ADJUSTMENTS].push(Math.max(0, Math.min(50000, adjustment)));
      }
    } else if (el.type === 'text') {
      const isVerticalText = el.style.vert && el.style.vert !== 'horz';
      const fontSizePt = resolveTextFontSizePt(el);
      const { x: boxX, w: boxW } = safeTextBoxGeometry(
        el.x,
        el.w,
        el.style.align,
        isVerticalText,
        fontSizePt,
        { bold: textUsesBold(el) },
      );
      const textOptions = {
        x: boxX,
        y: el.y,
        w: boxW,
        h: Math.min(el.h, Math.max(0.15, SLIDE_H_IN - el.y - 0.04)),
        fontSize: el.style.fontSize,
        fontFace: resolvePptxFontFace(
          el.style.fontFace,
          Array.isArray(el.text) ? el.text.map((run) => run?.text || '').join('') : el.text,
        ),
        color: el.style.color,
        bold: el.style.bold,
        italic: el.style.italic,
        underline: el.style.underline,
        valign: isVerticalText ? 'mid' : (el.style.valign || 'top'),
        lineSpacing: el.style.lineSpacing,
        paraSpaceBefore: el.style.paraSpaceBefore,
        paraSpaceAfter: el.style.paraSpaceAfter,
        // margin reproduces the element's CSS padding as PPTX internal inset,
        // preventing text from shifting toward the frame's top-left corner.
        margin: pptxTextMargin(el.style.margin),
        shrinkText: false,
        autoFit: false,
      };
      if (el.style.align) textOptions.align = el.style.align;
      if (el.style.rotate !== undefined) textOptions.rotate = el.style.rotate;
      if (el.style.vert) textOptions.vert = el.style.vert;
      if (el.style.transparency != null && el.style.transparency !== undefined) {
        textOptions.transparency = el.style.transparency;
      }
      if (Number.isFinite(el.style.charSpacing)) {
        textOptions.charSpacing = el.style.charSpacing;
      }
      targetSlide.addText(pptxTextValue(el.text), textOptions);
    }
  }
}

export async function buildSlideFromScene(slideData, pres, options = {}) {
  validateEditableSlideScene(slideData);
  const targetSlide = options.slide || pres.addSlide();
  try {
    addSceneNodes(slideData, targetSlide, pres);
  } catch (error) {
    if (error instanceof EditableExportError) throw error;
    const diagnostic = {
      severity: 'blocking',
      kind: 'blocking',
      code: 'pptx_serialization',
      message: String(error?.message || error || 'PPTX serialization failed.'),
      slideNumber: slideData.slideNumber,
      sourceId: null,
      tag: null,
      cause: error,
    };
    error.diagnostic = diagnostic;
    error.diagnostics = [diagnostic];
    throw error;
  }
  return {
    slide: targetSlide,
    diagnostics: [],
  };
}

// PptxGenJS 4.0.1 assigns table cNvPr ids with `tableIndex * slideNum + 1`,
// while shapes use `objectIndex + 2`. On a slide that already has a background
// shape at id=2, the first table also gets id=2. Duplicate cNvPr ids make
// Microsoft PowerPoint open the file as "needs repair"; the repair rewrite
// commonly destroys CJK runs inside a:tbl while leaving non-table text intact.
function uniquifySlideObjectIds(xml) {
  let nextId = 1;
  return xml.replace(/<p:cNvPr id="\d+"/g, () => `<p:cNvPr id="${nextId++}"`);
}

// PptxGenJS emits one slideMaster Override per slide, but only writes
// slideMaster1.xml (gitbrent/PptxGenJS#1444). Phantom Overrides force the
// PowerPoint "repair this presentation" dialog on every multi-slide deck.
function fixContentTypesXml(xml) {
  return String(xml || '')
    .replace(
      /<Override PartName="\/ppt\/slideMasters\/slideMaster(?:[2-9]|\d{2,})\.xml"[^>]*\/>/g,
      '',
    )
    .replace(
      'ContentType="image/jpg"',
      'ContentType="image/jpeg"',
    );
}

// Placeholder shapes on notesMaster are stripped by PowerPoint repair
// (gitbrent/PptxGenJS#1443). Per-slide notesSlide parts keep their own ph shapes.
function stripNotesMasterPlaceholderShapes(xml) {
  return String(xml || '').replace(/<p:sp>[\s\S]*?<\/p:sp>/g, '');
}

// OOXML requires every <p:sp> to contain <p:txBody>. Decorative shapes from
// addShape() omit it (gitbrent/PptxGenJS#1441), which also triggers repair.
// notesSlide "Slide Image Placeholder" ships the same defect on every deck.
export function ensureShapeTextBodies(xml) {
  return String(xml || '').replace(/<p:sp>([\s\S]*?)<\/p:sp>/g, (match, body) => {
    if (body.includes('<p:txBody')) return match;
    return `<p:sp>${body}${EMPTY_SHAPE_TX_BODY}</p:sp>`;
  });
}

// PptxGenJS emits empty <a:ln></a:ln> for shapes without a border. PowerPoint
// repair rewrites these; normalize to an explicit noFill line.
export function ensureLineNoFill(xml) {
  return String(xml || '')
    .replace(/<a:ln([^>]*)\/>/g, '<a:ln$1><a:noFill/></a:ln>')
    .replace(/<a:ln([^>]*)>\s*<\/a:ln>/g, '<a:ln$1><a:noFill/></a:ln>');
}

// Solid slide backgrounds from pptxgen omit <a:effectLst/> inside <p:bgPr>
// (gitbrent/PptxGenJS#1442). Image backgrounds already include it; solid ones
// must match or PowerPoint may repair the package.
export function ensureSolidBackgroundEffectList(xml) {
  return String(xml || '').replace(/<p:bgPr>([\s\S]*?)<\/p:bgPr>/g, (match, body) => {
    if (!body.includes('<a:solidFill') || body.includes('<a:effectLst')) return match;
    return `<p:bgPr>${body}<a:effectLst/></p:bgPr>`;
  });
}

// OOXML ST_PositiveCoordinate forbids negative a:ext cx/cy. Upward/leftward
// lines from html2pptx often serialize as negative extents; PowerPoint repair
// clamps them to 0. Rewrite as positive extents + flipH/flipV.
export function fixNegativeExtents(xml) {
  return String(xml || '').replace(/<a:xfrm([^>]*)>([\s\S]*?)<\/a:xfrm>/g, (match, attrs, body) => {
    const off = body.match(/<a:off x="(-?\d+)" y="(-?\d+)"\/>/);
    const ext = body.match(/<a:ext cx="(-?\d+)" cy="(-?\d+)"\/>/);
    if (!off || !ext) return match;
    let x = Number(off[1]);
    let y = Number(off[2]);
    let cx = Number(ext[1]);
    let cy = Number(ext[2]);
    if (![x, y, cx, cy].every(Number.isFinite) || (cx >= 0 && cy >= 0)) return match;
    let flipH = /\bflipH="1"/.test(attrs);
    let flipV = /\bflipV="1"/.test(attrs);
    if (cx < 0) {
      x += cx;
      cx = Math.abs(cx);
      flipH = !flipH;
    }
    if (cy < 0) {
      y += cy;
      cy = Math.abs(cy);
      flipV = !flipV;
    }
    let nextAttrs = String(attrs || '')
      .replace(/\s*flipH="1"/g, '')
      .replace(/\s*flipV="1"/g, '');
    if (flipH) nextAttrs += ' flipH="1"';
    if (flipV) nextAttrs += ' flipV="1"';
    const nextBody = body
      .replace(/<a:off x="-?\d+" y="-?\d+"\/>/, `<a:off x="${x}" y="${y}"/>`)
      .replace(/<a:ext cx="-?\d+" cy="-?\d+"\/>/, `<a:ext cx="${cx}" cy="${cy}"/>`);
    return `<a:xfrm${nextAttrs}>${nextBody}</a:xfrm>`;
  });
}

// PptxGenJS maps valign "mid" to OOXML anchor="mid", but ST_TextAnchoringType
// only allows t/ctr/b. Invalid mid on a:tcPr triggers PowerPoint repair.
export function fixInvalidTableCellAnchors(xml) {
  return String(xml || '').replace(
    /<a:tcPr\b([^>]*)>/g,
    (match, attrs) => `<a:tcPr${String(attrs || '').replace(/\banchor="mid"/g, 'anchor="ctr"')}>`,
  );
}

function formatFontSlot(slot, typeface, attrs) {
  const cleaned = String(attrs || '').replace(/\/\s*$/, '');
  return `<a:${slot} typeface="${typeface}"${cleaned}/>`;
}

/**
 * Normalize DrawingML font slots for Win/Mac/Linux:
 * - fill empty theme ea/cs
 * - split identical latin/ea/cs triplets so CJK uses Microsoft YaHei on ea
 *   while Latin stays on Arial (or the designer latin face)
 */
export function normalizeOoxmlFonts(xml) {
  let next = String(xml || '');
  next = next.replace(
    /<a:latin typeface="([^"]*)"([^>]*)\/?>\s*<a:ea typeface="([^"]*)"([^>]*)\/?>\s*<a:cs typeface="([^"]*)"([^>]*)\/?>/g,
    (_match, latinFace, latinAttrs, eaFace, eaAttrs, _csFace, csAttrs) => {
      const sourceFace = latinFace || eaFace || '';
      const pair = resolveCrossPlatformFontPair(sourceFace);
      return (
        formatFontSlot('latin', pair.latin, latinAttrs)
        + formatFontSlot('ea', pair.ea, eaAttrs)
        + formatFontSlot('cs', pair.cs, csAttrs)
      );
    },
  );
  // Theme often ships `<a:ea typeface=""/><a:cs typeface=""/>` beside latin.
  next = next.replace(/<a:ea typeface=""/g, `<a:ea typeface="${PPTX_CJK_FONT_FACE}"`);
  next = next.replace(/<a:cs typeface=""/g, `<a:cs typeface="${PPTX_LATIN_FONT_FACE}"`);
  // Lone major/minor latin entries that still name a CJK-only face.
  next = next.replace(
    /<a:latin typeface="(?:PingFang SC|Microsoft YaHei|微软雅黑)"/g,
    `<a:latin typeface="${PPTX_LATIN_FONT_FACE}"`,
  );
  return next;
}

function applyRoundRectAdjustments(xml, adjustments, startIndex) {
  let adjustmentIndex = startIndex;
  const next = xml.replace(
    /<a:prstGeom prst="roundRect"><a:avLst(?:\/>|>[\s\S]*?<\/a:avLst>)<\/a:prstGeom>/g,
    (match) => {
      const adjustment = adjustments[adjustmentIndex];
      adjustmentIndex += 1;
      if (adjustment == null) return match;
      return '<a:prstGeom prst="roundRect"><a:avLst>'
        + `<a:gd name="adj" fmla="val ${adjustment}"/>`
        + '</a:avLst></a:prstGeom>';
    },
  );
  return { xml: next, adjustmentIndex };
}

async function postProcessPptxOutput(output, outputType, adjustments) {
  if (!['base64', 'nodebuffer'].includes(outputType)) return output;
  const needsRoundRect = adjustments.length > 0;
  const zip = await JSZip.loadAsync(output, { base64: outputType === 'base64' });

  const contentTypes = zip.file('[Content_Types].xml');
  if (contentTypes) {
    zip.file('[Content_Types].xml', fixContentTypesXml(await contentTypes.async('string')));
  }

  const xmlPaths = Object.keys(zip.files)
    .filter((path) => path.endsWith('.xml') && !path.endsWith('/'))
    .sort();
  let adjustmentIndex = 0;
  for (const path of xmlPaths) {
    if (path === '[Content_Types].xml') continue;
    let xml = await zip.file(path).async('string');
    const isSlide = /^ppt\/slides\/slide\d+\.xml$/.test(path);
    const isNotesSlide = /^ppt\/notesSlides\/notesSlide\d+\.xml$/.test(path);
    if (path === 'ppt/notesMasters/notesMaster1.xml') {
      xml = stripNotesMasterPlaceholderShapes(xml);
      xml = ensureSolidBackgroundEffectList(xml);
      xml = ensureLineNoFill(xml);
      xml = normalizeOoxmlFonts(xml);
      zip.file(path, xml);
      continue;
    }
    if (isSlide && needsRoundRect) {
      ({ xml, adjustmentIndex } = applyRoundRectAdjustments(xml, adjustments, adjustmentIndex));
    }
    if (isSlide || isNotesSlide) {
      xml = ensureShapeTextBodies(xml);
    }
    if (isSlide) {
      xml = uniquifySlideObjectIds(xml);
      xml = fixNegativeExtents(xml);
      xml = fixInvalidTableCellAnchors(xml);
    }
    if (
      isSlide
      || isNotesSlide
      || path === 'ppt/slideMasters/slideMaster1.xml'
      || /^ppt\/slideLayouts\/slideLayout\d+\.xml$/.test(path)
    ) {
      xml = ensureSolidBackgroundEffectList(xml);
    }
    if (isSlide || isNotesSlide) {
      xml = ensureLineNoFill(xml);
    }
    xml = normalizeOoxmlFonts(xml);
    zip.file(path, xml);
  }
  if (needsRoundRect && adjustmentIndex !== adjustments.length) {
    throw new Error('Round rectangle OOXML adjustment count did not match serialized shapes');
  }
  return zip.generateAsync({
    type: outputType,
    compression: 'DEFLATE',
  });
}

export function createPptxDeck(deck = {}) {
  const pptx = new pptxgen();
  pptx.layout = 'LAYOUT_WIDE';
  pptx.author = 'PPT Live';
  pptx.subject = deck.brief?.topic || deck.title || 'PPT Live deck';
  pptx.title = deck.title || 'PPT Live';
  pptx.company = 'BitFun';
  pptx.lang = 'zh-CN';
  pptx.theme = {
    headFontFace: PPTX_LATIN_FONT_FACE,
    bodyFontFace: PPTX_LATIN_FONT_FACE,
    lang: 'zh-CN',
  };
  pptx[OOXML_ROUND_RECT_ADJUSTMENTS] = [];
  const write = pptx.write.bind(pptx);
  pptx.write = async (options = {}) => {
    const output = await write(options);
    return postProcessPptxOutput(
      output,
      options.outputType,
      pptx[OOXML_ROUND_RECT_ADJUSTMENTS],
    );
  };
  return pptx;
}

export function buildSpeakerNotes(sourceSlide = {}) {
  return [
    sourceSlide.notes,
    sourceSlide.claim ? `Claim: ${sourceSlide.claim}` : '',
    sourceSlide.proofObject ? `Proof object: ${sourceSlide.proofObject}` : '',
    sourceSlide.supportNote ? `Support note: ${sourceSlide.supportNote}` : '',
    sourceSlide.sourceNote ? `Source note: ${sourceSlide.sourceNote}` : '',
  ].filter(Boolean).join('\n\n');
}
