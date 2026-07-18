// ─────────────────────────────────────────────────────────────────────────────
// EditableSlideScene → PPTX Build
//
// buildSlideFromScene() validates and serializes only scene.nodes.
//
// Key design decisions:
//   WIDTH_SAFETY_IN  — text boxes are widened by 0.15" to absorb cross-renderer
//     font metric drift (PowerPoint renders CJK glyphs slightly wider than
//     browsers). safeTextBoxGeometry() shifts the x coordinate for right/center
//     aligned text so the extra width doesn't shift the visual anchor.
//   margin — set from the element's CSS padding so PPTX internal inset matches
//     the HTML box model (prevents text from shifting toward top-left).
//   valign — resolved from CSS flex/grid align-items or line-height ratio.
// ─────────────────────────────────────────────────────────────────────────────
import pptxgen from 'pptxgenjs';
import JSZip from 'jszip';
import { EditableExportError, validateEditableSlideScene } from './editable-slide-scene.js';

const SLIDE_W_IN = 13.333;
const SLIDE_H_IN = 7.5;
const OOXML_ROUND_RECT_ADJUSTMENTS = Symbol('ppt-live-round-rect-adjustments');

// PowerPoint and browsers render the same font at the same point size with
// measurably different glyph widths (different font metric tables / hinting).
// For CJK text the drift is amplified because every glyph is full-width:
// if a line of CJK text *barely* fits on one line in the browser, PowerPoint's
// slightly wider rendering pushes the last character to the next line.
//
// 0.15 inch (~14.4px @ 96dpi) ≈ one CJK glyph at ~14pt body text. This is
// large enough to absorb the worst-case cross-renderer metric drift while
// capTextBoxWidth() prevents any overflow past the slide edge.
//
// IMPORTANT: the safety width widens the text box to prevent wrapping, but
// for right/center-aligned text this alone would shift the rendered glyphs
// (right edge moves right, center moves right).  The callers compensate by
// adjusting the x coordinate so that the *original* text region is preserved.
const WIDTH_SAFETY_IN = 0.15;

function capTextBoxWidth(x, w) {
  return Math.min(w, Math.max(0.15, SLIDE_W_IN - x - 0.02));
}

// Given an element's original x/w and its text-align, return {x, w} for the
// PPTX text box.  The box is widened by WIDTH_SAFETY_IN to prevent wrapping,
// but the x coordinate is shifted so the original left/right/center anchor
// stays in the same visual position:
//   left   → extra width extends to the right (x unchanged)
//   right  → extra width extends to the left  (x shifts left by safety)
//   center → split equally                    (x shifts left by safety/2)
function safeTextBoxGeometry(origX, origW, align, isVerticalText) {
  const safety = isVerticalText ? 0 : WIDTH_SAFETY_IN;
  const rawW = origW + safety;
  if (!safety || align === 'left' || !align) {
    return { x: origX, w: capTextBoxWidth(origX, rawW) };
  }
  if (align === 'right') {
    const x = Math.max(0, origX - safety);
    return { x, w: capTextBoxWidth(x, rawW) };
  }
  if (align === 'center') {
    const x = Math.max(0, origX - safety / 2);
    return { x, w: capTextBoxWidth(x, rawW) };
  }
  // justify behaves like left for anchoring
  return { x: origX, w: capTextBoxWidth(origX, rawW) };
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

function resolvePptxFontFace(fontFace, textHint = '') {
  const face = String(fontFace || '').replace(/['"]/g, '').trim();
  if (!face) return 'PingFang SC';
  const lower = face.toLowerCase();
  if (CSS_SYSTEM_FONT_FACES.has(lower) || lower.startsWith('.')) {
    return 'PingFang SC';
  }
  // PptxGenJS writes the same typeface into latin/ea/cs. Latin-only faces such
  // as Arial therefore lock East-Asian glyphs to tofu after PowerPoint opens
  // (or repairs) the table. Prefer the deck CJK body font for CJK runs.
  if (
    textHintContainsCjk(textHint)
    && (lower === 'arial' || lower === 'helvetica' || lower === 'times new roman')
  ) {
    return 'PingFang SC';
  }
  return face;
}

function withResolvedFontFace(options = {}, textHint = '') {
  if (!options || typeof options !== 'object') return options;
  if (options.fontFace == null && options.fontFamily == null) {
    return textHintContainsCjk(textHint)
      ? { ...options, fontFace: 'PingFang SC' }
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
      const { x: boxX, w: boxW } = safeTextBoxGeometry(el.x, el.w, el.style.align, isVerticalText);
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

async function postProcessPptxOutput(output, outputType, adjustments) {
  if (!['base64', 'nodebuffer'].includes(outputType)) return output;
  const needsRoundRect = adjustments.length > 0;
  const zip = await JSZip.loadAsync(output, { base64: outputType === 'base64' });
  const slidePaths = Object.keys(zip.files)
    .filter((path) => /^ppt\/slides\/slide\d+\.xml$/.test(path))
    .sort((left, right) => (
      Number(left.match(/\d+/)?.[0]) - Number(right.match(/\d+/)?.[0])
    ));
  let adjustmentIndex = 0;
  for (const path of slidePaths) {
    let xml = await zip.file(path).async('string');
    if (needsRoundRect) {
      xml = xml.replace(
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
    }
    xml = uniquifySlideObjectIds(xml);
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
    headFontFace: 'PingFang SC',
    bodyFontFace: 'PingFang SC',
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
