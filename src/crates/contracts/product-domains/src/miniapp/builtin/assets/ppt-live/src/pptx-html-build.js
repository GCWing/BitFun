import pptxgen from 'pptxgenjs';

const PX_PER_IN = 96;
const EMU_PER_IN = 914400;
const SLIDE_W_IN = 13.333;
const SLIDE_H_IN = 7.5;

function capTextBoxWidth(x, w) {
  return Math.min(w, Math.max(0.15, SLIDE_W_IN - x - 0.04));
}

function toImagePayload(src) {
  const raw = String(src || '').trim();
  if (!raw) return null;
  if (raw.startsWith('data:')) return { data: raw };
  if (raw.startsWith('file://')) return { path: raw.replace('file://', '') };
  return { path: raw };
}

function validateDimensions(bodyDimensions, pres) {
  const errors = [];
  const widthInches = bodyDimensions.width / PX_PER_IN;
  const heightInches = bodyDimensions.height / PX_PER_IN;
  if (pres.presLayout) {
    const layoutWidth = pres.presLayout.width / EMU_PER_IN;
    const layoutHeight = pres.presLayout.height / EMU_PER_IN;
    if (Math.abs(layoutWidth - widthInches) > 0.1 || Math.abs(layoutHeight - heightInches) > 0.1) {
      errors.push(
        `HTML dimensions (${widthInches.toFixed(1)}" × ${heightInches.toFixed(1)}") `
        + `don't match presentation layout (${layoutWidth.toFixed(1)}" × ${layoutHeight.toFixed(1)}")`,
      );
    }
  }
  return errors;
}

function validateTextBoxPosition(slideData, bodyDimensions) {
  const errors = [];
  const slideHeightInches = bodyDimensions.height / PX_PER_IN;
  const minBottomMargin = 0.5;
  for (const el of slideData.elements) {
    if (!['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'text', 'list', 'merged-text'].includes(el.type)) continue;
    const fontSize = el.style?.fontSize || 0;
    const bottomEdge = el.position.y + el.position.h;
    const distanceFromBottom = slideHeightInches - bottomEdge;
    const textValue = (() => {
      if (typeof el.text === 'string') return el.text;
      if (Array.isArray(el.text)) return el.text.find((t) => t.text)?.text || '';
      if (Array.isArray(el.items)) return el.items.find((item) => item.text)?.text || '';
      return '';
    })();
    const isFooter = /^\d{1,2}\s*\/\s*\d{1,2}$/.test(textValue.trim())
      || /^(Arknights|数据来源|source:)/i.test(textValue.trim());
    if (isFooter || fontSize <= 11) continue;
    if (fontSize > 12 && distanceFromBottom < minBottomMargin) {
      const textPrefix = `${textValue.substring(0, 50)}${textValue.length > 50 ? '...' : ''}`;
      errors.push(
        `Text box "${textPrefix}" ends too close to bottom edge `
        + `(${distanceFromBottom.toFixed(2)}" from bottom, minimum ${minBottomMargin}" required)`,
      );
    }
  }
  return errors;
}

async function addBackground(slideData, targetSlide) {
  if (slideData.background?.type === 'image' && slideData.background.path) {
    const payload = toImagePayload(slideData.background.path);
    if (payload) targetSlide.background = payload;
  } else if (slideData.background?.type === 'color' && slideData.background.value) {
    targetSlide.background = { color: slideData.background.value };
  }
}

function addElements(slideData, targetSlide, pres) {
  for (const el of slideData.elements) {
    if (el.type === 'image') {
      const payload = toImagePayload(el.src);
      if (!payload) continue;
      targetSlide.addImage({
        ...payload,
        x: el.position.x,
        y: el.position.y,
        w: el.position.w,
        h: el.position.h,
      });
    } else if (el.type === 'line') {
      targetSlide.addShape(pres.ShapeType.line, {
        x: el.x1,
        y: el.y1,
        w: el.x2 - el.x1,
        h: el.y2 - el.y1,
        line: { color: el.color, width: el.width },
      });
    } else if (el.type === 'shape') {
      const shapeOptions = {
        x: el.position.x,
        y: el.position.y,
        w: el.position.w,
        h: el.position.h,
        shape: el.shape.rectRadius > 0 ? pres.ShapeType.roundRect : pres.ShapeType.rect,
      };
      if (el.shape.fill) {
        shapeOptions.fill = { color: el.shape.fill };
        if (el.shape.transparency != null) shapeOptions.fill.transparency = el.shape.transparency;
      }
      if (el.shape.line) shapeOptions.line = el.shape.line;
      if (el.shape.rectRadius > 0) shapeOptions.rectRadius = el.shape.rectRadius;
      if (el.shape.shadow) shapeOptions.shadow = el.shape.shadow;
      targetSlide.addText(el.text || '', shapeOptions);
    } else if (el.type === 'list' || el.type === 'merged-text') {
      const listOptions = {
        x: el.position.x,
        y: el.position.y,
        w: capTextBoxWidth(el.position.x, el.position.w + (el.position.w * 0.04)),
        h: Math.min(el.position.h, Math.max(0.15, SLIDE_H_IN - el.position.y - 0.04)),
        fontSize: el.style.fontSize,
        fontFace: el.style.fontFace,
        color: el.style.color,
        align: el.style.align,
        valign: 'top',
        lineSpacing: el.style.lineSpacing,
        paraSpaceBefore: el.style.paraSpaceBefore,
        paraSpaceAfter: el.style.paraSpaceAfter,
        margin: el.style.margin,
        inset: 0,
        shrinkText: false,
        autoFit: false,
      };
      if (el.style.transparency != null) listOptions.transparency = el.style.transparency;
      targetSlide.addText(el.items || el.text, listOptions);
    } else {
      const lineHeight = el.style.lineSpacing || el.style.fontSize * 1.2;
      const isSingleLine = el.position.h <= lineHeight * 1.5;
      const isVerticalText = el.style.vert && el.style.vert !== 'horz';
      const widthIncrease = isVerticalText ? 0 : el.position.w * (isSingleLine ? 0.02 : 0.06);
      let adjustedX = el.position.x;
      let adjustedW = capTextBoxWidth(el.position.x, el.position.w + widthIncrease);
      const align = el.style.align;
      if (!isVerticalText && align === 'center') {
        adjustedX = el.position.x - ((adjustedW - el.position.w) / 2);
      } else if (!isVerticalText && align === 'right') {
        adjustedX = el.position.x - (adjustedW - el.position.w);
      }
      adjustedX = Math.max(0, adjustedX);
      adjustedW = capTextBoxWidth(adjustedX, adjustedW);
      const textOptions = {
        x: adjustedX,
        y: el.position.y,
        w: adjustedW,
        h: Math.min(el.position.h, Math.max(0.15, SLIDE_H_IN - el.position.y - 0.04)),
        fontSize: el.style.fontSize,
        fontFace: el.style.fontFace,
        color: el.style.color,
        bold: el.style.bold,
        italic: el.style.italic,
        underline: el.style.underline,
        valign: isVerticalText ? 'mid' : (el.style.valign || 'top'),
        lineSpacing: el.style.lineSpacing,
        paraSpaceBefore: el.style.paraSpaceBefore,
        paraSpaceAfter: el.style.paraSpaceAfter,
        inset: 0,
        shrinkText: false,
        autoFit: false,
      };
      if (el.style.align) textOptions.align = el.style.align;
      if (el.style.margin) textOptions.margin = el.style.margin;
      if (el.style.rotate !== undefined) textOptions.rotate = el.style.rotate;
      if (el.style.vert) textOptions.vert = el.style.vert;
      if (el.style.transparency != null && el.style.transparency !== undefined) {
        textOptions.transparency = el.style.transparency;
      }
      targetSlide.addText(el.text, textOptions);
    }
  }
}

export async function buildSlideFromExtracted(slideData, bodyDimensions, pres, options = {}) {
  // Validation findings (overflow, bottom-margin, dimension mismatch) are
  // logged as warnings instead of aborting the export: a clipped slide in the
  // PPTX is always better than a failed export run.
  const validationWarnings = [];
  if (bodyDimensions?.errors?.length) validationWarnings.push(...bodyDimensions.errors);
  validationWarnings.push(...validateDimensions(bodyDimensions, pres));
  validationWarnings.push(...validateTextBoxPosition(slideData, bodyDimensions));
  if (slideData?.errors?.length) validationWarnings.push(...slideData.errors);
  if (validationWarnings.length) {
    console.warn('[ppt-live-export] slide validation warnings (export continues):', validationWarnings.join('; '));
  }
  const targetSlide = options.slide || pres.addSlide();
  await addBackground(slideData, targetSlide);
  addElements(slideData, targetSlide, pres);
  return { slide: targetSlide, placeholders: slideData.placeholders || [] };
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
