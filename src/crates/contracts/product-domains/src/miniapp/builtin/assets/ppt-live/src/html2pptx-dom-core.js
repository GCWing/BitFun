// ─────────────────────────────────────────────────────────────────────────────
// HTML → PPTX Slide Data Extraction (Stage 1 of 2)
//
// extractSlideDataFromDocument() walks a live DOM document and produces a
// structured slideData object: { background, elements[], placeholders[], errors[] }.
//
// Each element has: { type, position: {x,y,w,h in inches}, style: {...}, text }
// Positions come from getBoundingClientRect() (border-box), converted to inches.
//
// This file handles EXTRACTION only. The slideData is then passed to
// pptx-html-build.js which maps it to pptxgenjs API calls (Stage 2).
//
// Unit conversions:  96 px = 1 inch,  1 px = 0.75 pt,  PPTX uses inches/EMU.
// Slide canvas:      1280×720 px = 13.333"×7.5" (LAYOUT_WIDE).
// ─────────────────────────────────────────────────────────────────────────────
import { buildDomPaintOrderMap } from './paint-order.js';

export const PT_PER_PX = 0.75;
export const PX_PER_IN = 96;

export function createSvgViewportMapper(svg, svgRect) {
  const values = String(svg.getAttribute('viewBox') || `0 0 ${svgRect.width} ${svgRect.height}`)
    .trim().split(/[\s,]+/).map(Number);
  const [viewBoxX = 0, viewBoxY = 0, viewBoxWidth = svgRect.width, viewBoxHeight = svgRect.height] = values;
  const raw = String(svg.getAttribute('preserveAspectRatio') || 'xMidYMid meet').trim();
  const parts = raw.split(/\s+/).filter(Boolean);
  const align = parts.find((part) => /^(?:none|x(?:Min|Mid|Max)Y(?:Min|Mid|Max))$/.test(part))
    || 'xMidYMid';
  const mode = parts.includes('slice') ? 'slice' : 'meet';
  const naturalXScale = svgRect.width / (viewBoxWidth || svgRect.width);
  const naturalYScale = svgRect.height / (viewBoxHeight || svgRect.height);
  let xScale = naturalXScale;
  let yScale = naturalYScale;
  let offsetX = 0;
  let offsetY = 0;
  if (align !== 'none') {
    const scale = mode === 'slice'
      ? Math.max(naturalXScale, naturalYScale)
      : Math.min(naturalXScale, naturalYScale);
    xScale = scale;
    yScale = scale;
    const remainingX = svgRect.width - viewBoxWidth * scale;
    const remainingY = svgRect.height - viewBoxHeight * scale;
    offsetX = align.startsWith('xMid') ? remainingX / 2 : align.startsWith('xMax') ? remainingX : 0;
    offsetY = align.includes('YMid') ? remainingY / 2 : align.includes('YMax') ? remainingY : 0;
  }
  return {
    xScale,
    yScale,
    map(point) {
      return {
        x: svgRect.left + offsetX + (point.x - viewBoxX) * xScale,
        y: svgRect.top + offsetY + (point.y - viewBoxY) * yScale,
      };
    },
  };
}

export function classifySvgPresetPolygon(points) {
  if (!Array.isArray(points)) return null;
  const tolerance = 1e-6;
  if (points.length === 3) {
    for (let left = 0; left < points.length; left += 1) {
      for (let right = left + 1; right < points.length; right += 1) {
        const apex = points.find((_point, index) => index !== left && index !== right);
        const baseMidpoint = {
          x: (points[left].x + points[right].x) / 2,
          y: (points[left].y + points[right].y) / 2,
        };
        if (Math.abs(points[left].x - points[right].x) <= tolerance
          && Math.abs(apex.y - baseMidpoint.y) <= tolerance
          && Math.abs(apex.x - baseMidpoint.x) > tolerance) {
          return { shapeType: 'triangle', rotate: apex.x > baseMidpoint.x ? 90 : 270 };
        }
        if (Math.abs(points[left].y - points[right].y) <= tolerance
          && Math.abs(apex.x - baseMidpoint.x) <= tolerance
          && Math.abs(apex.y - baseMidpoint.y) > tolerance) {
          return { shapeType: 'triangle', rotate: apex.y > baseMidpoint.y ? 180 : 0 };
        }
      }
    }
    return null;
  }
  if (points.length !== 4) return null;
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const minX = Math.min(...xs);
  const maxX = Math.max(...xs);
  const minY = Math.min(...ys);
  const maxY = Math.max(...ys);
  const centerX = (minX + maxX) / 2;
  const centerY = (minY + maxY) / 2;
  const expected = [
    { x: centerX, y: minY },
    { x: maxX, y: centerY },
    { x: centerX, y: maxY },
    { x: minX, y: centerY },
  ];
  const matches = expected.every((target) => points.some((point) => (
    Math.abs(point.x - target.x) <= tolerance && Math.abs(point.y - target.y) <= tolerance
  )));
  return matches ? { shapeType: 'diamond', rotate: 0 } : null;
}

function clipsOverflowAxis(value) {
  return value === 'hidden' || value === 'clip';
}

export function measureBodyDimensions(doc = document) {
  const view = doc.defaultView || window;
  const body = doc.body;
  const style = view.getComputedStyle(body);
  const bodyDimensions = {
    width: parseFloat(style.width),
    height: parseFloat(style.height),
    scrollWidth: body.scrollWidth,
    scrollHeight: body.scrollHeight,
  };
  const errors = [];
  const widthOverflowPx = Math.max(0, bodyDimensions.scrollWidth - bodyDimensions.width - 1);
  const heightOverflowPx = Math.max(0, bodyDimensions.scrollHeight - bodyDimensions.height - 1);
  const widthOverflowPt = widthOverflowPx * PT_PER_PX;
  const heightOverflowPt = heightOverflowPx * PT_PER_PX;
  const overflowX = style.overflowX || style.overflow || 'visible';
  const overflowY = style.overflowY || style.overflow || 'visible';
  const clipsX = clipsOverflowAxis(overflowX);
  const clipsY = clipsOverflowAxis(overflowY);
  // Slide bodies use overflow:hidden as the canvas clip frame (also enforced by
  // ensureExportCanvas). Decorative bleed/rotation expands scrollWidth/Height
  // while remaining visually clipped — same as PowerPoint off-slide shapes.
  // Only unclipped overflow is a blocking canvas error; text bounds stay separate.
  const reportWidth = widthOverflowPx > 0 && !clipsX;
  const reportHeight = heightOverflowPx > 0 && !clipsY;
  if (reportWidth || reportHeight) {
    const directions = [];
    if (reportWidth) directions.push(`${widthOverflowPt.toFixed(1)}pt horizontally`);
    if (reportHeight) directions.push(`${heightOverflowPt.toFixed(1)}pt vertically`);
    const reminder = reportHeight ? ' (Remember: leave 0.5" margin at bottom of slide)' : '';
    errors.push(`HTML content overflows body by ${directions.join(' and ')}${reminder}`);
  }
  return { ...bodyDimensions, errors };
}

export function extractSlideDataFromDocument(doc = document) {
  const document = doc;
  const view = document?.defaultView || globalThis.window;
  const diagnostics = [];
  const diagnosticKeys = new Set();

  const addDiagnostic = (severity, code, message, element = null) => {
    const sourceId = element?.dataset?.pptxSourceId || element?.id || null;
    const key = `${severity}:${code}:${sourceId || ''}`;
    if (diagnosticKeys.has(key)) return;
    diagnosticKeys.add(key);
    const diagnostic = {
      severity,
      kind: severity === 'blocking' ? 'blocking' : undefined,
      code,
      message,
      sourceId,
      tag: element?.tagName?.toLowerCase?.() || null,
    };
    try {
      const rect = element?.getBoundingClientRect?.();
      if (rect && rect.width > 0 && rect.height > 0) {
        diagnostic.bbox = {
          x: rect.left,
          y: rect.top,
          width: rect.width,
          height: rect.height,
        };
      }
    } catch {
      // bbox is optional.
    }
    diagnostics.push(diagnostic);
  };

  if (!document?.body || !view?.getComputedStyle) {
    addDiagnostic(
      'blocking',
      'unreadable_document',
      'Slide document has no readable body or computed-style view.',
      document?.documentElement || null,
    );
    const unreadableDiagnostic = diagnostics[diagnostics.length - 1];
    unreadableDiagnostic.sourceId = unreadableDiagnostic.sourceId || 'slide-document';
    unreadableDiagnostic.tag = 'document';
    return {
      background: { type: 'color', value: 'FFFFFF' },
      elements: [],
      placeholders: [],
      diagnostics,
      errors: diagnostics.map((item) => item.message),
    };
  }

    const PT_PER_PX = 0.75;
    const PX_PER_IN = 96;

    // Fonts that are single-weight and should not have bold applied
    // (applying bold causes PowerPoint to use faux bold which makes text wider)
    const SINGLE_WEIGHT_FONTS = ['impact'];

    // Helper: Check if a font should skip bold formatting
    const shouldSkipBold = (fontFamily) => {
      if (!fontFamily) return false;
      const normalizedFont = fontFamily.toLowerCase().replace(/['"]/g, '').split(',')[0].trim();
      return SINGLE_WEIGHT_FONTS.includes(normalizedFont);
    };

    // Unit conversion helpers
    const pxToInch = (px) => px / PX_PER_IN;
    const pxToPoints = (pxStr) => parseFloat(pxStr) * PT_PER_PX;
    const rgbToHex = (rgbStr) => {
      // Handle transparent backgrounds by defaulting to white
      if (rgbStr === 'rgba(0, 0, 0, 0)' || rgbStr === 'transparent') return 'FFFFFF';

      const hex = String(rgbStr || '').trim().match(/^#([\da-f]{3,8})$/i)?.[1];
      if (hex) {
        if (hex.length === 3 || hex.length === 4) {
          return hex.slice(0, 3).split('').map((channel) => channel.repeat(2)).join('');
        }
        return hex.slice(0, 6);
      }
      const match = rgbStr.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
      if (!match) return 'FFFFFF';
      return match.slice(1).map(n => parseInt(n).toString(16).padStart(2, '0')).join('');
    };

    const extractAlpha = (rgbStr) => {
      const match = rgbStr.match(/rgba\((\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\)/);
      if (!match || !match[4]) return null;
      const alpha = parseFloat(match[4]);
      return Math.round((1 - alpha) * 100);
    };

    const parseRgbChannels = (rgbStr) => {
      const match = String(rgbStr || '').match(/rgba?\((\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\)/);
      if (!match) return null;
      return {
        r: parseInt(match[1], 10),
        g: parseInt(match[2], 10),
        b: parseInt(match[3], 10),
        a: match[4] != null ? parseFloat(match[4]) : 1,
      };
    };

    const hexToRgb = (hex) => {
      const clean = String(hex || '0E0E12').replace('#', '');
      return {
        r: parseInt(clean.slice(0, 2), 16),
        g: parseInt(clean.slice(2, 4), 16),
        b: parseInt(clean.slice(4, 6), 16),
      };
    };

    const resolveSolidFill = (rgbStr, backdropHex = '0E0E12') => {
      const channels = parseRgbChannels(rgbStr);
      if (!channels) {
        return { fill: rgbToHex(rgbStr), transparency: extractAlpha(rgbStr) };
      }
      if (channels.a >= 0.98) {
        return { fill: rgbToHex(rgbStr), transparency: null };
      }
      const bg = hexToRgb(backdropHex);
      const r = Math.round(bg.r * (1 - channels.a) + channels.r * channels.a);
      const g = Math.round(bg.g * (1 - channels.a) + channels.g * channels.a);
      const b = Math.round(bg.b * (1 - channels.a) + channels.b * channels.a);
      return {
        fill: [r, g, b].map((n) => n.toString(16).padStart(2, '0')).join('').toUpperCase(),
        transparency: null,
      };
    };

    const applyTextTransform = (text, textTransform) => {
      if (textTransform === 'uppercase') return text.toUpperCase();
      if (textTransform === 'lowercase') return text.toLowerCase();
      if (textTransform === 'capitalize') {
        return text.replace(/\b\w/g, c => c.toUpperCase());
      }
      return text;
    };

    const getTextDirection = (writingMode) => {
      if (writingMode === 'vertical-rl' || writingMode === 'vertical-lr') {
        return 'eaVert';
      }
      return null;
    };

    // Extract rotation angle from CSS transform only. CSS writing-mode is exported
    // as native PowerPoint vertical text so its box geometry stays anchored.
    const getRotation = (transform) => {
      let angle = 0;

      if (transform && transform !== 'none') {
        // Try to match rotate() function
        const rotateMatch = transform.match(/rotate\((-?\d+(?:\.\d+)?)deg\)/);
        if (rotateMatch) {
          angle += parseFloat(rotateMatch[1]);
        } else {
          // Browser may compute as matrix - extract rotation from matrix
          const matrixMatch = transform.match(/matrix\(([^)]+)\)/);
          if (matrixMatch) {
            const values = matrixMatch[1].split(',').map(parseFloat);
            // matrix(a, b, c, d, e, f) where rotation = atan2(b, a)
            const matrixAngle = Math.atan2(values[1], values[0]) * (180 / Math.PI);
            angle += Math.round(matrixAngle);
          }
        }
      }

      // Normalize to 0-359 range
      angle = angle % 360;
      if (angle < 0) angle += 360;

      return angle === 0 ? null : angle;
    };

    // Get position/dimensions accounting for rotation
    const getPositionAndSize = (el, rect, rotation) => {
      if (rotation === null) {
        return { x: rect.left, y: rect.top, w: rect.width, h: rect.height };
      }

      const isVertical = rotation === 90 || rotation === 270;

      if (isVertical) {
        const centerX = rect.left + rect.width / 2;
        const centerY = rect.top + rect.height / 2;

        return {
          x: centerX - rect.height / 2,
          y: centerY - rect.width / 2,
          w: rect.height,
          h: rect.width,
        };
      }

      const centerX = rect.left + rect.width / 2;
      const centerY = rect.top + rect.height / 2;
      return {
        x: centerX - el.offsetWidth / 2,
        y: centerY - el.offsetHeight / 2,
        w: el.offsetWidth,
        h: el.offsetHeight,
      };
    };

    // Parse CSS box-shadow into either:
    // - { mode: 'outer', shadow } native PowerPoint outer shadow (zero spread)
    // - { mode: 'ring', ring } zero-offset/zero-blur spread ring → concentric shape rewrite
    // Returns null for none / fully unsupported values (all layers inset, etc.).
    //
    // Multi-layer shadows and spread forms no longer block: the first usable
    // non-inset layer is mapped to the native outer shadow; a negative or
    // soft spread is approximated to zero. That approximation is far closer
    // to the authored visual than dropping the whole export.
    const splitBoxShadowLayers = (boxShadow) => {
      const layers = [];
      let nesting = 0;
      let start = 0;
      for (let index = 0; index < boxShadow.length; index += 1) {
        const character = boxShadow[index];
        if (character === '(') nesting += 1;
        else if (character === ')') nesting = Math.max(0, nesting - 1);
        else if (character === ',' && nesting === 0) {
          layers.push(boxShadow.slice(start, index));
          start = index + 1;
        }
      }
      layers.push(boxShadow.slice(start));
      return layers.map((layer) => layer.trim()).filter(Boolean);
    };

    const parseBoxShadowLayer = (layer) => {
      // Browser computed style format: "rgba(0, 0, 0, 0.3) 2px 2px 8px 0px [inset]"
      // CSS format: "[inset] 2px 2px 8px 0px rgba(0, 0, 0, 0.3)"

      // IMPORTANT: PptxGenJS/PowerPoint doesn't properly support inset shadows
      // Only process outer shadows to avoid file corruption
      if (/\binset\b/i.test(layer)) return null;

      // Extract color first (rgba/rgb/hex); length tokens may be unitless 0.
      const colorMatch = layer.match(/rgba?\([^)]+\)|#[0-9a-fA-F]{3,8}/);
      const withoutColor = layer
        .replace(/rgba?\([^)]+\)/g, ' ')
        .replace(/#[0-9a-fA-F]{3,8}/g, ' ');
      // CSS allows unitless 0; browsers often keep "0 0 0 1px" in authored/computed forms.
      const parts = withoutColor.match(/-?\d*\.?\d+(?:px|pt)?/g);

      if (!parts || parts.length < 2) return null;

      const offsetX = parseFloat(parts[0]);
      const offsetY = parseFloat(parts[1]);
      const blur = parts.length > 2 ? parseFloat(parts[2]) : 0;
      const spread = parts.length > 3 ? parseFloat(parts[3]) : 0;
      if (![offsetX, offsetY, blur, spread].every(Number.isFinite) || blur < 0) return null;

      // Extract opacity from rgba
      let opacity = 0.5;
      if (colorMatch) {
        const opacityMatch = colorMatch[0].match(/[\d.]+\)$/);
        if (opacityMatch) {
          opacity = parseFloat(opacityMatch[0].replace(')', ''));
        }
      }
      if (!Number.isFinite(opacity) || opacity < 0 || opacity > 1) return null;
      const color = colorMatch ? rgbToHex(colorMatch[0]) : '000000';
      if (!color) return null;

      // Hard CSS ring: 0 offset, 0 blur, positive spread > 0 → concentric editable shape.
      if (spread > 0 && offsetX === 0 && offsetY === 0 && blur === 0) {
        return {
          mode: 'ring',
          ring: { spreadPx: spread, color, opacity },
        };
      }

      // Native PowerPoint outer shadows have no spread concept. Approximate
      // negative/positive spread as zero instead of rejecting the shadow.
      // Calculate angle from offsets (in degrees, 0 = right, 90 = down)
      let angle = 0;
      if (offsetX !== 0 || offsetY !== 0) {
        angle = Math.atan2(offsetY, offsetX) * (180 / Math.PI);
        if (angle < 0) angle += 360;
      }

      // Calculate offset distance (hypotenuse)
      const offset = Math.sqrt(offsetX * offsetX + offsetY * offsetY) * PT_PER_PX;

      return {
        mode: 'outer',
        shadow: {
          type: 'outer',
          angle: Math.round(angle),
          blur: blur * 0.75, // Convert to points
          color,
          offset,
          opacity,
        },
      };
    };

    const parseBoxShadow = (boxShadow) => {
      if (!boxShadow || boxShadow === 'none') return null;
      for (const layer of splitBoxShadowLayers(boxShadow)) {
        const parsed = parseBoxShadowLayer(layer);
        if (parsed) return parsed;
      }
      return null;
    };

    const outerShadowOf = (effect) => (effect?.mode === 'outer' ? effect.shadow : null);

    // Inline tags keep per-run style. Block wrappers (especially <li><p>…</p></li>
    // authored by the deck skill / element-model path) must also be flattened —
    // skipping them yields empty list items, scene validation fails, and degrade
    // removes the entire UL/OL (dropping all dense body copy).
    const INLINE_FORMAT_TAGS = new Set([
      'SPAN', 'B', 'STRONG', 'I', 'EM', 'U', 'SMALL', 'LABEL', 'A', 'CODE', 'MARK', 'SUB', 'SUP',
    ]);
    const BLOCK_TEXT_WRAPPER_TAGS = new Set(['P', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6']);

    const applyTextRunComputedStyle = (node, options, textTransform) => {
      const computed = view.getComputedStyle(node);
      const nextOptions = { ...options };
      let nextTransform = textTransform;
      const isBold = computed.fontWeight === 'bold' || parseInt(computed.fontWeight, 10) >= 600;
      if (isBold && !shouldSkipBold(computed.fontFamily)) nextOptions.bold = true;
      if (computed.fontStyle === 'italic') nextOptions.italic = true;
      if (computed.textDecoration && computed.textDecoration.includes('underline')) {
        nextOptions.underline = true;
      }
      if (computed.color && computed.color !== 'rgb(0, 0, 0)') {
        nextOptions.color = rgbToHex(computed.color);
        const transparency = extractAlpha(computed.color);
        if (transparency !== null) nextOptions.transparency = transparency;
      }
      if (computed.fontSize) nextOptions.fontSize = pxToPoints(computed.fontSize);
      if (computed.textTransform && computed.textTransform !== 'none') {
        const transformStr = computed.textTransform;
        nextTransform = (text) => applyTextTransform(text, transformStr);
      }
      return { options: nextOptions, textTransform: nextTransform, computed };
    };

    // Parse inline formatting tags (<b>, <i>, <u>, <strong>, <em>, <span>) into text runs
    const parseInlineFormatting = (element, baseOptions = {}, runs = [], baseTextTransform = (x) => x) => {
      let prevNodeIsText = false;

      element.childNodes.forEach((node) => {
        let textTransform = baseTextTransform;

        const isText = node.nodeType === view.Node.TEXT_NODE || node.tagName === 'BR';
        if (isText) {
          const text = node.tagName === 'BR' ? '\n' : textTransform(node.textContent.replace(/\s+/g, ' '));
          const prevRun = runs[runs.length - 1];
          if (prevNodeIsText && prevRun) {
            prevRun.text += text;
          } else {
            runs.push({ text, options: { ...baseOptions } });
          }

        } else if (node.nodeType === view.Node.ELEMENT_NODE && node.textContent.trim()) {
          if (INLINE_FORMAT_TAGS.has(node.tagName) || BLOCK_TEXT_WRAPPER_TAGS.has(node.tagName)) {
            const styled = applyTextRunComputedStyle(node, baseOptions, textTransform);
            const options = styled.options;
            textTransform = styled.textTransform;
            const computed = styled.computed;

            // Validate: Check for margins on inline elements
            if (INLINE_FORMAT_TAGS.has(node.tagName)) {
              if (computed.marginLeft && parseFloat(computed.marginLeft) > 0) {
                addDiagnostic('blocking', 'inline_margin', `Inline element <${node.tagName.toLowerCase()}> has unsupported margin-left.`, node);
              }
              if (computed.marginRight && parseFloat(computed.marginRight) > 0) {
                addDiagnostic('blocking', 'inline_margin', `Inline element <${node.tagName.toLowerCase()}> has unsupported margin-right.`, node);
              }
              if (computed.marginTop && parseFloat(computed.marginTop) > 0) {
                addDiagnostic('blocking', 'inline_margin', `Inline element <${node.tagName.toLowerCase()}> has unsupported margin-top.`, node);
              }
              if (computed.marginBottom && parseFloat(computed.marginBottom) > 0) {
                addDiagnostic('blocking', 'inline_margin', `Inline element <${node.tagName.toLowerCase()}> has unsupported margin-bottom.`, node);
              }
            }

            // Recursively process the child node. This will flatten nested spans
            // and block wrappers (li > p) into multiple runs.
            parseInlineFormatting(node, options, runs, textTransform);
          }
        }

        prevNodeIsText = isText;
      });

      // Trim leading space from first run and trailing space from last run
      if (runs.length > 0) {
        runs[0].text = runs[0].text.replace(/^\s+/, '');
        runs[runs.length - 1].text = runs[runs.length - 1].text.replace(/\s+$/, '');
      }

      return runs.filter(r => r.text.length > 0);
    };

    const isTransparentBg = (color) => !color || color === 'transparent' || color === 'rgba(0, 0, 0, 0)';

    const authoredStyleValue = (element, property) => {
      const authored = element?.dataset?.pptxAuthoredStyle || '';
      const match = authored.match(new RegExp(`(?:^|;)\\s*${property}\\s*:\\s*([^;]+)`, 'i'));
      return match?.[1]?.trim() || '';
    };

    const elementBoxShadow = (element, computed) => (
      [computed?.boxShadow, element?.style?.boxShadow, authoredStyleValue(element, 'box-shadow')]
        .find((value) => value && value !== 'none') || 'none'
    );

    const elementBackgroundColor = (element, computed) => {
      if (!isTransparentBg(computed.backgroundColor)) return computed.backgroundColor;
      return element.style.backgroundColor
        || authoredStyleValue(element, 'background-color')
        || element.style.background
        || authoredStyleValue(element, 'background')
        || computed.backgroundColor;
    };

    const resolveSlideBackground = (rootBody) => {
      const candidates = [
        rootBody,
        document.documentElement,
        rootBody?.querySelector?.(':scope > section, :scope > div, :scope > main'),
      ].filter(Boolean);
      for (const el of candidates) {
        const style = view.getComputedStyle(el);
        const bgImage = style.backgroundImage || '';
        if (bgImage.includes('linear-gradient') || bgImage.includes('radial-gradient')) {
          return { gradient: true, element: el };
        }
        if (bgImage && bgImage !== 'none') {
          const urlMatch = bgImage.match(/url\(["']?([^"')]+)["']?\)/);
          if (urlMatch) return { type: 'image', path: urlMatch[1] };
        }
        const bgColor = style.backgroundColor;
        if (!isTransparentBg(bgColor)) {
          return { type: 'color', value: rgbToHex(bgColor) };
        }
      }
      return { type: 'color', value: 'FFFFFF' };
    };

    // Extract background from body / slide root wrapper
    const body = document.body;
    const bodyRect = body.getBoundingClientRect();
    if (!(bodyRect.width > 0) || !(bodyRect.height > 0)) {
      addDiagnostic(
        'blocking',
        'unmeasurable_canvas',
        'Slide canvas has no measurable width or height.',
        body,
      );
    }
    const boxFor = (rect) => ({
      left: rect.left - bodyRect.left,
      top: rect.top - bodyRect.top,
      width: rect.width,
      height: rect.height,
    });
    const rectFor = (el) => boxFor(el.getBoundingClientRect());
    const textRectFor = (el) => {
      try {
        const range = document.createRange();
        range.selectNodeContents(el);
        const rect = boxFor(range.getBoundingClientRect());
        range.detach?.();
        if (rect.width > 0 && rect.height > 0) return rect;
      } catch {
        // Fall back to the element box when Range geometry is unavailable.
      }
      return rectFor(el);
    };

    const resolveTextColor = (computed, el) => {
      const channels = parseRgbChannels(computed.color);
      if (!channels || channels.a >= 0.2) return rgbToHex(computed.color);
      const plain = el.textContent.trim();
      if (!plain) return rgbToHex(computed.color);
      return 'E8E8E8';
    };

    const expandTextFrame = (el, rect, rotation) => {
      let { x, y, w, h } = getPositionAndSize(el, rect, rotation);
      const slideWidthPx = bodyRect.width;
      const slideHeightPx = bodyRect.height;
      const maxWPx = Math.max(8, slideWidthPx - x - 4);
      const scrollH = el.scrollHeight || 0;
      // Width: trust the browser's layout.  Adding extra width shifts the
      // wrapping point and causes the PPT to break lines in different places
      // than the HTML preview.  Only expand the height for scroll overflow.
      w = Math.min(w, maxWPx);
      const maxHPx = Math.max(8, slideHeightPx - y - 4);
      if (scrollH > h + 2) h = Math.min(scrollH, maxHPx);
      else h = Math.min(h, maxHPx);
      return { x, y, w, h };
    };

    const readZIndex = (el) => {
      let current = el;
      while (current && current !== body) {
        const raw = view.getComputedStyle(current).zIndex;
        if (raw && raw !== 'auto') {
          const parsed = parseInt(raw, 10);
          if (Number.isFinite(parsed)) return parsed;
        }
        current = current.parentElement;
      }
      return 0;
    };

    const domPaintOrder = buildDomPaintOrderMap(document);
    const sourceSubOrders = new Map();
    let unmappedPaintOrder = Math.max(-1, ...domPaintOrder.values()) + 1;
    const nextPaintMetadata = (el) => {
      const sourceId = el?.dataset?.pptxSourceId || el?.id || null;
      const subOrder = sourceSubOrders.get(sourceId) || 0;
      sourceSubOrders.set(sourceId, subOrder + 1);
      return {
        sourceId,
        paintOrder: domPaintOrder.get(sourceId) ?? unmappedPaintOrder++,
        subOrder,
      };
    };
    const pushElement = (entry, el) => {
      const paintMetadata = nextPaintMetadata(el);
      entry.kind = entry.kind || 'native';
      entry.paintOrder = entry.paintOrder ?? paintMetadata.paintOrder;
      entry.subOrder = entry.subOrder ?? paintMetadata.subOrder;
      if (!entry.bbox) {
        if (entry.position) {
          entry.bbox = { ...entry.position };
        } else if (entry.type === 'line') {
          entry.bbox = {
            x: Math.min(entry.x1, entry.x2),
            y: Math.min(entry.y1, entry.y2),
            w: Math.abs(entry.x2 - entry.x1),
            h: Math.abs(entry.y2 - entry.y1),
          };
        }
      }
      if (el) {
        entry.zIndex = readZIndex(el);
        entry.sourceId = paintMetadata.sourceId;
      }
      elements.push(entry);
    };

    const pushCssRingBackdrop = (el, rectPx, rectRadius, ring) => {
      if (!ring || !rectPx || !(rectPx.width > 0) || !(rectPx.height > 0)) return;
      const spreadIn = pxToInch(ring.spreadPx);
      if (!(spreadIn > 0)) return;
      const ringRadius = rectRadius === 1 || !Number.isFinite(rectRadius) || rectRadius <= 0
        ? rectRadius
        : rectRadius + spreadIn;
      const transparency = Math.round((1 - ring.opacity) * 100);
      pushElement({
        type: 'shape',
        text: '',
        rewrite: 'css_box_shadow_ring',
        position: {
          x: pxToInch(rectPx.left) - spreadIn,
          y: pxToInch(rectPx.top) - spreadIn,
          w: pxToInch(rectPx.width) + spreadIn * 2,
          h: pxToInch(rectPx.height) + spreadIn * 2,
        },
        shape: {
          fill: ring.color,
          transparency: Number.isFinite(transparency) ? transparency : null,
          line: null,
          rectRadius: ringRadius || 0,
          shadow: null,
        },
      }, el);
      addDiagnostic(
        'rewrite',
        'css_box_shadow_ring',
        'CSS ring box-shadow was rewritten as a concentric editable shape.',
        el,
      );
    };

    const resolveListBulletColor = (ul, liElements, textHex) => {
      const ulComputed = view.getComputedStyle(ul);
      const listColor = ulComputed.listStyleColor;
      if (listColor && listColor !== 'rgba(0, 0, 0, 0)') {
        const hex = rgbToHex(listColor);
        if (hex && hex !== textHex) return hex;
      }
      for (const li of liElements) {
        try {
          const marker = view.getComputedStyle(li, '::marker');
          if (marker?.color) {
            const hex = rgbToHex(marker.color);
            if (hex && hex !== textHex) return hex;
          }
        } catch {
          // ::marker not supported in this WebView
        }
      }
      return null;
    };

    const bgResolved = resolveSlideBackground(body);
    if (bgResolved.gradient) {
      addDiagnostic(
        'rewrite',
        'css_gradient',
        'CSS gradient will be rewritten as editable solid strips.',
        bgResolved.element || body,
      );
    }

    const background = bgResolved.gradient
      ? { type: 'color', value: 'FFFFFF' }
      : { type: bgResolved.type, ...(bgResolved.path ? { path: bgResolved.path } : { value: bgResolved.value }) };
    const slideBackdropHex = background.value || '0E0E12';

    // Process all elements
    const elements = [];
    const placeholders = [];
    const textTags = ['P', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'UL', 'OL', 'LI'];
    const genericInlineTextTags = new Set([
      'SPAN', 'SMALL', 'LABEL', 'A', 'CODE', 'BUTTON', 'B', 'STRONG', 'I', 'EM', 'U', 'MARK', 'SUB', 'SUP',
    ]);
    const tableCellTags = new Set(['TD', 'TH']);
    const containerTags = new Set(['DIV', 'SECTION', 'ARTICLE', 'ASIDE', ...tableCellTags]);
    const processed = new Set();

    const hasTextOwnerAncestor = (el) => {
      let parent = el.parentElement;
      while (parent && parent !== body) {
        if (textTags.includes(parent.tagName) || parent.dataset?.pptxMerge === 'true') return true;
        parent = parent.parentElement;
      }
      return false;
    };

    const hasDirectText = (el) => Array.from(el.childNodes).some(
      (node) => node.nodeType === view.Node.TEXT_NODE && node.textContent.replace(/\s+/g, ' ').trim(),
    );

    const isGenericTextElement = (el) => {
      if (!el.textContent.replace(/\s+/g, ' ').trim() || hasTextOwnerAncestor(el)) return false;
      if (genericInlineTextTags.has(el.tagName)) return true;
      if (!containerTags.has(el.tagName) || (!hasDirectText(el) && !tableCellTags.has(el.tagName))) return false;
      if (el.querySelector('p,h1,h2,h3,h4,h5,h6,ul,ol,li')) return false;
      return !Array.from(el.children).some((child) => containerTags.has(child.tagName));
    };

    const resolveTextAlign = (computed) => {
      if (computed.textAlign === 'end') return 'right';
      if (['left', 'center', 'right', 'justify'].includes(computed.textAlign)) {
        return computed.textAlign;
      }
      if (computed.display.includes('flex')) {
        if (computed.justifyContent === 'center') return 'center';
        if (computed.justifyContent === 'flex-end' || computed.justifyContent === 'end') return 'right';
      }
      if (computed.display.includes('grid')) {
        if ((computed.justifyItems || computed.placeItems || '').includes('center')) return 'center';
        if ((computed.justifyItems || computed.placeItems || '').includes('end')) return 'right';
      }
      return 'left';
    };

    const resolveVerticalAlign = (computed, rect) => {
      if (computed.display === 'table-cell') {
        if (computed.verticalAlign === 'middle') return 'mid';
        if (computed.verticalAlign === 'bottom') return 'bottom';
      }
      if (!computed.display.includes('flex') && !computed.display.includes('grid')) {
        const lineHeight = parseFloat(computed.lineHeight || '0');
        return lineHeight >= rect.height * 0.75 ? 'mid' : 'top';
      }
      const alignment = computed.alignItems || computed.placeItems || '';
      if (alignment.includes('center')) return 'mid';
      if (alignment.includes('end')) return 'bottom';
      return 'top';
    };

    // Resolve CSS line-height to a concrete pt value that matches browser
    // rendering.  CSS "normal" is font-dependent (~1.15–1.2× font-size); the
    // PPT default differs, so we emit an explicit value to preserve the
    // visual line spacing of the HTML preview.
    const resolveLineSpacing = (computed) => {
      const lh = computed.lineHeight;
      const fontSize = pxToPoints(computed.fontSize) || 12;
      if (!lh || lh === 'normal') {
        return fontSize * 1.2;
      }
      const parsed = parseFloat(lh);
      if (Number.isNaN(parsed)) return fontSize * 1.2;
      return pxToPoints(lh) || fontSize * 1.2;
    };

    // CSS letter-spacing → PPTX charSpacing (pt). Negative tracking is common
    // on titles; dropping it makes PowerPoint glyphs looser and can wrap a
    // line that fit in the browser.
    const resolveCharSpacing = (computed) => {
      const raw = computed.letterSpacing;
      if (!raw || raw === 'normal') return undefined;
      const px = parseFloat(raw);
      if (!Number.isFinite(px) || Math.abs(px) < 0.01) return undefined;
      return px * PT_PER_PX;
    };

    const emitTextElement = (el, type = el.tagName.toLowerCase(), exactFrame = false, rectOverride = null) => {
      const rect = rectOverride || rectFor(el);
      const text = el.textContent.replace(/\s+/g, ' ').trim();
      if (rect.width === 0 || rect.height === 0 || !text) return false;

      if (type !== 'text' && el.tagName !== 'LI' && /^[•●○▪‣·▸◆◇■□]\s/u.test(text.trimStart())) {
        addDiagnostic(
          'blocking',
          'manual_bullet_unrepaired',
          `Text element <${el.tagName.toLowerCase()}> still starts with a manual bullet; exporting as editable text.`,
          el,
        );
      }

      const computed = view.getComputedStyle(el);
      if (computed.display === 'none' || computed.visibility === 'hidden' || parseFloat(computed.opacity || '1') <= 0) {
        return false;
      }
      const rotation = getRotation(computed.transform);
      const textDirection = getTextDirection(computed.writingMode);
      const frame = exactFrame
        ? getPositionAndSize(el, rect, rotation)
        : expandTextFrame(el, rect, rotation);
      const isBold = computed.fontWeight === 'bold' || parseInt(computed.fontWeight, 10) >= 600;

      // The element's CSS padding defines how far the text is inset from the
      // element's border edge. In HTML, getBoundingClientRect() returns the
      // border-box, and the browser renders text inside the content-box
      // (border-box minus padding). To reproduce this in PPTX, we keep the
      // frame at the border-box and set the PPTX internal margin (= inset)
      // to the element's padding. This prevents text from shifting up-left
      // when the element has non-zero padding.
      const padL = parseFloat(computed.paddingLeft) || 0;
      const padR = parseFloat(computed.paddingRight) || 0;
      const padT = parseFloat(computed.paddingTop) || 0;
      const padB = parseFloat(computed.paddingBottom) || 0;
      const textInset = [padT, padR, padB, padL].some((v) => v > 0)
        ? [pxToInch(padT), pxToInch(padR), pxToInch(padB), pxToInch(padL)]
        : 0;

      const baseStyle = {
        fontSize: pxToPoints(computed.fontSize) || 12,
        fontFace: computed.fontFamily.split(',')[0].replace(/['"]/g, '').trim() || 'Arial',
        color: resolveTextColor(computed, el),
        align: resolveTextAlign(computed),
        valign: resolveVerticalAlign(computed, rect),
        lineSpacing: resolveLineSpacing(computed),
        paraSpaceBefore: 0,
        paraSpaceAfter: 0,
        margin: textInset,
      };
      const charSpacing = resolveCharSpacing(computed);
      if (charSpacing !== undefined) baseStyle.charSpacing = charSpacing;

      const transparency = extractAlpha(computed.color);
      if (transparency !== null) baseStyle.transparency = transparency;
      if (rotation !== null) baseStyle.rotate = rotation;
      if (textDirection !== null) baseStyle.vert = textDirection;

      const hasFormatting = el.querySelector('b, i, u, strong, em, span, small, label, a, code, mark, sub, sup, br');
      if (hasFormatting) {
        const transformStr = computed.textTransform;
        const runBase = {};
        if (isBold && !shouldSkipBold(computed.fontFamily)) runBase.bold = true;
        let runs = parseInlineFormatting(el, runBase, [], (str) => applyTextTransform(str, transformStr));
        const runText = runs.map((run) => run.text).join('').trim();
        if (!runText && text) {
          runs = [{ text: applyTextTransform(text, transformStr), options: { ...runBase } }];
        }

        const adjustedStyle = { ...baseStyle };
        if (adjustedStyle.lineSpacing) {
          const maxFontSize = Math.max(
            adjustedStyle.fontSize,
            ...runs.map((run) => run.options?.fontSize || 0),
          );
          if (maxFontSize > adjustedStyle.fontSize) {
            adjustedStyle.lineSpacing = maxFontSize * (adjustedStyle.lineSpacing / adjustedStyle.fontSize);
          }
        }

        pushElement({
          type,
          text: runs,
          position: {
            x: pxToInch(frame.x),
            y: pxToInch(frame.y),
            w: pxToInch(frame.w),
            h: pxToInch(frame.h),
          },
          style: adjustedStyle,
        }, el);
      } else {
        pushElement({
          type,
          text: applyTextTransform(text, computed.textTransform),
          position: {
            x: pxToInch(frame.x),
            y: pxToInch(frame.y),
            w: pxToInch(frame.w),
            h: pxToInch(frame.h),
          },
          style: {
            ...baseStyle,
            bold: isBold && !shouldSkipBold(computed.fontFamily),
            italic: computed.fontStyle === 'italic',
            underline: computed.textDecoration.includes('underline'),
          },
        }, el);
      }

      processed.add(el);
      if (type === 'text') {
        el.querySelectorAll('span,small,label,a,code,b,strong,i,em,u,mark,sub,sup').forEach((child) => processed.add(child));
      }
      return true;
    };

    const svgColor = (value, fallback = null) => {
      if (!value || value === 'none' || value === 'transparent') return fallback;
      return rgbToHex(value);
    };
    const svgNumber = (value, fallback = 0) => {
      const parsed = parseFloat(String(value || ''));
      return Number.isFinite(parsed) ? parsed : fallback;
    };
    const svgDashStyle = (value) => {
      const values = String(value || '').match(/[-+]?(?:\d*\.)?\d+/g)?.map(Number)
        .filter((item) => Number.isFinite(item) && item >= 0) || [];
      if (!values.length) return null;
      if (values.length >= 4 && values[2] <= values[0] * 0.5) return 'dashDot';
      if (values.length >= 2 && values[0] <= values[1] * 0.5) return 'dot';
      return 'dash';
    };
    const identityMatrix = () => [1, 0, 0, 1, 0, 0];
    const multiplyMatrix = (left, right) => [
      left[0] * right[0] + left[2] * right[1],
      left[1] * right[0] + left[3] * right[1],
      left[0] * right[2] + left[2] * right[3],
      left[1] * right[2] + left[3] * right[3],
      left[0] * right[4] + left[2] * right[5] + left[4],
      left[1] * right[4] + left[3] * right[5] + left[5],
    ];
    const parseSvgTransform = (value = '') => {
      const raw = String(value || '').trim();
      let matrix = identityMatrix();
      let layoutMatrix = identityMatrix();
      let rotation = 0;
      if (!raw || raw === 'none') {
        return { matrix, layoutMatrix, rotation, reliable: true };
      }
      if (/(?:skew|perspective|matrix3d)\s*\(/i.test(raw)) {
        return { matrix, layoutMatrix, rotation, reliable: false };
      }
      const functions = raw.matchAll(/(matrix|translate|scale|rotate)\s*\(([^)]*)\)/gi);
      let matched = false;
      for (const match of functions) {
        matched = true;
        const name = match[1].toLowerCase();
        const values = match[2].trim().split(/[\s,]+/).filter(Boolean).map((item) => parseFloat(item));
        let next = identityMatrix();
        let nextLayout = identityMatrix();
        if (name === 'matrix' && values.length >= 6) {
          if (Math.abs(values[1]) > 1e-6 || Math.abs(values[2]) > 1e-6) {
            return { matrix, layoutMatrix, rotation, reliable: false };
          }
          next = values.slice(0, 6);
          nextLayout = next;
        } else if (name === 'translate') {
          next = [1, 0, 0, 1, values[0] || 0, values[1] || 0];
          nextLayout = next;
        } else if (name === 'scale') {
          const x = Number.isFinite(values[0]) ? values[0] : 1;
          const y = Number.isFinite(values[1]) ? values[1] : x;
          next = [x, 0, 0, y, 0, 0];
          nextLayout = next;
        } else if (name === 'rotate') {
          const angle = values[0] || 0;
          rotation += angle;
          const radians = angle * Math.PI / 180;
          const cos = Math.cos(radians);
          const sin = Math.sin(radians);
          const rotationMatrix = [cos, sin, -sin, cos, 0, 0];
          if (Number.isFinite(values[1]) && Number.isFinite(values[2])) {
            next = multiplyMatrix(
              multiplyMatrix([1, 0, 0, 1, values[1], values[2]], rotationMatrix),
              [1, 0, 0, 1, -values[1], -values[2]],
            );
          } else {
            next = rotationMatrix;
          }
        }
        matrix = multiplyMatrix(matrix, next);
        layoutMatrix = multiplyMatrix(layoutMatrix, nextLayout);
      }
      return { matrix, layoutMatrix, rotation, reliable: matched };
    };
    const transformForSvgNode = (node, svg) => {
      if (typeof node.getCTM === 'function') {
        try {
          const ctm = node.getCTM();
          const matrix = ctm
            ? [ctm.a, ctm.b, ctm.c, ctm.d, ctm.e, ctm.f].map(Number)
            : null;
          if (matrix?.every(Number.isFinite)) {
            const scaleX = Math.hypot(matrix[0], matrix[1]);
            const scaleY = Math.hypot(matrix[2], matrix[3]);
            const orthogonality = scaleX > 0 && scaleY > 0
              ? Math.abs((matrix[0] * matrix[2] + matrix[1] * matrix[3]) / (scaleX * scaleY))
              : Infinity;
            if (orthogonality > 1e-5 || matrix[0] * matrix[3] - matrix[1] * matrix[2] <= 0) {
              return {
                matrix,
                layoutMatrix: identityMatrix(),
                rotation: 0,
                reliable: false,
                coordinateSpace: 'viewport',
              };
            }
            return {
              matrix,
              layoutMatrix: [scaleX, 0, 0, scaleY, matrix[4], matrix[5]],
              rotation: Math.atan2(matrix[1], matrix[0]) * 180 / Math.PI,
              reliable: true,
              coordinateSpace: 'viewport',
            };
          }
        } catch {
          // Fall through to deterministic attribute/computed-style parsing.
        }
      }
      const chain = [];
      let current = node;
      while (current && current !== svg) {
        chain.unshift(current);
        current = current.parentElement;
      }
      let matrix = identityMatrix();
      let layoutMatrix = identityMatrix();
      let rotation = 0;
      for (const item of chain) {
        const attributeTransform = parseSvgTransform(item.getAttribute('transform'));
        if (!attributeTransform.reliable) {
          return { matrix, layoutMatrix, rotation, reliable: false };
        }
        matrix = multiplyMatrix(matrix, attributeTransform.matrix);
        layoutMatrix = multiplyMatrix(layoutMatrix, attributeTransform.layoutMatrix);
        rotation += attributeTransform.rotation;

        const computedTransformValue = view.getComputedStyle(item).transform;
        if (computedTransformValue && computedTransformValue !== 'none'
          && computedTransformValue !== item.getAttribute('transform')) {
          const computedTransform = parseSvgTransform(computedTransformValue);
          if (!computedTransform.reliable) {
            return { matrix, layoutMatrix, rotation, reliable: false };
          }
          let computedMatrix = computedTransform.matrix;
          const originValues = String(view.getComputedStyle(item).transformOrigin || '')
            .split(/\s+/).map((value) => parseFloat(value));
          if (Number.isFinite(originValues[0]) && Number.isFinite(originValues[1])
            && computedTransform.rotation) {
            computedMatrix = multiplyMatrix(
              multiplyMatrix(
                [1, 0, 0, 1, originValues[0], originValues[1]],
                computedMatrix,
              ),
              [1, 0, 0, 1, -originValues[0], -originValues[1]],
            );
          }
          matrix = multiplyMatrix(matrix, computedMatrix);
          layoutMatrix = multiplyMatrix(layoutMatrix, computedTransform.layoutMatrix);
          rotation += computedTransform.rotation;
        }
      }
      return {
        matrix,
        layoutMatrix,
        rotation,
        reliable: true,
        coordinateSpace: 'viewBox',
      };
    };
    const transformPoint = (point, matrix) => ({
      x: matrix[0] * point.x + matrix[2] * point.y + matrix[4],
      y: matrix[1] * point.x + matrix[3] * point.y + matrix[5],
    });
    const parseSvgPoints = (value) => {
      const numbers = String(value || '').trim().split(/[\s,]+/).filter(Boolean).map(Number);
      const points = [];
      for (let index = 0; index + 1 < numbers.length; index += 2) {
        if (Number.isFinite(numbers[index]) && Number.isFinite(numbers[index + 1])) {
          points.push({ x: numbers[index], y: numbers[index + 1] });
        }
      }
      return points;
    };
    const boundsOfPoints = (points) => {
      const xs = points.map((point) => point.x);
      const ys = points.map((point) => point.y);
      return {
        left: Math.min(...xs),
        top: Math.min(...ys),
        width: Math.max(...xs) - Math.min(...xs),
        height: Math.max(...ys) - Math.min(...ys),
      };
    };
    const emitNativeSvg = (svg) => {
      const svgRect = rectFor(svg);
      if (svgRect.width <= 0 || svgRect.height <= 0) return;
      const viewportMapper = createSvgViewportMapper(svg, svgRect);
      const { xScale, yScale } = viewportMapper;
      const toSlidePoint = (point) => viewportMapper.map(point);
      const pushLine = (start, end, node, stroke, width, coordinateSpace = 'viewBox') => {
        const toLinePoint = (point) => (
          coordinateSpace === 'viewport'
            ? { x: svgRect.left + point.x, y: svgRect.top + point.y }
            : toSlidePoint(point)
        );
        const first = toLinePoint(start);
        const second = toLinePoint(end);
        const dashArray = node.getAttribute?.('stroke-dasharray')
          || view.getComputedStyle(node).strokeDasharray;
        const dash = svgDashStyle(dashArray);
        const lineStyle = {
          color: stroke || '000000',
          width,
          ...(dash ? { dash } : {}),
        };
        pushElement({
          type: 'line',
          kind: 'native',
          x1: pxToInch(first.x),
          y1: pxToInch(first.y),
          x2: pxToInch(second.x),
          y2: pxToInch(second.y),
          color: lineStyle.color,
          width: lineStyle.width,
          style: lineStyle,
        }, node);
      };
      svg.querySelectorAll('rect,circle,ellipse,line,polyline,polygon,text,path').forEach((node) => {
        const tag = node.tagName.toLowerCase();
        if (tag === 'path') {
          return;
        }
        const transform = transformForSvgNode(node, svg);
        if (!transform.reliable) {
          addDiagnostic(
            'blocking',
            'svg_transform_unsupported',
            'SVG transform cannot be represented reliably as an editable native shape.',
            node,
          );
          return;
        }
        const transformedToSlidePoint = (transformed) => (
          transform.coordinateSpace === 'viewport'
            ? { x: svgRect.left + transformed.x, y: svgRect.top + transformed.y }
            : toSlidePoint(transformed)
        );
        const transformToSlidePoint = (point, matrix) => (
          transformedToSlidePoint(transformPoint(point, matrix))
        );
        const mapPoint = (x, y) => transformToSlidePoint(
          { x: svgNumber(x), y: svgNumber(y) },
          transform.matrix,
        );
        const mapLayoutPoint = (x, y) => transformToSlidePoint(
          { x: svgNumber(x), y: svgNumber(y) },
          transform.layoutMatrix,
        );
        const fill = svgColor(node.getAttribute('fill') || view.getComputedStyle(node).fill);
        const stroke = svgColor(node.getAttribute('stroke') || view.getComputedStyle(node).stroke);
        const opacity = svgNumber(node.getAttribute('opacity') || view.getComputedStyle(node).opacity, 1);
        const lineWidth = svgNumber(node.getAttribute('stroke-width'), 1) * 0.75;
        const common = {
          type: tag === 'text' ? 'svg-text' : 'svg-shape',
          kind: 'native',
          svgType: tag,
          position: null,
          shape: { fill, line: stroke ? { color: stroke, width: lineWidth } : null, transparency: Math.round((1 - opacity) * 100), rectRadius: 0 },
        };
        if (tag === 'rect') {
          const x = svgNumber(node.getAttribute('x'));
          const y = svgNumber(node.getAttribute('y'));
          const width = svgNumber(node.getAttribute('width'));
          const height = svgNumber(node.getAttribute('height'));
          const points = [
            mapPoint(x, y), mapPoint(x + width, y), mapPoint(x + width, y + height), mapPoint(x, y + height),
          ];
          const bounds = boundsOfPoints(points);
          const layoutBounds = boundsOfPoints([
            mapLayoutPoint(x, y),
            mapLayoutPoint(x + width, y),
            mapLayoutPoint(x + width, y + height),
            mapLayoutPoint(x, y + height),
          ]);
          common.position = { x: pxToInch(layoutBounds.left), y: pxToInch(layoutBounds.top), w: pxToInch(layoutBounds.width), h: pxToInch(layoutBounds.height) };
          common.bbox = { x: pxToInch(bounds.left), y: pxToInch(bounds.top), w: pxToInch(bounds.width), h: pxToInch(bounds.height) };
          const rx = svgNumber(node.getAttribute('rx'));
          const ry = svgNumber(node.getAttribute('ry'));
          if (rx > 0 || ry > 0) {
            common.svgType = 'roundRect';
            common.shape.rectRadius = pxToInch(Math.max(rx, ry));
            common.shape.radius = pxToInch(rx * xScale);
          }
          if (transform.rotation) common.shape.rotate = transform.rotation;
        } else if (tag === 'circle' || tag === 'ellipse') {
          const rx = tag === 'circle' ? svgNumber(node.getAttribute('r')) : svgNumber(node.getAttribute('rx'));
          const ry = tag === 'circle' ? rx : svgNumber(node.getAttribute('ry'));
          const cx = svgNumber(node.getAttribute('cx'));
          const cy = svgNumber(node.getAttribute('cy'));
          const points = [
            mapPoint(cx - rx, cy), mapPoint(cx + rx, cy), mapPoint(cx, cy - ry), mapPoint(cx, cy + ry),
          ];
          const bounds = boundsOfPoints(points);
          const layoutBounds = boundsOfPoints([
            mapLayoutPoint(cx - rx, cy), mapLayoutPoint(cx + rx, cy),
            mapLayoutPoint(cx, cy - ry), mapLayoutPoint(cx, cy + ry),
          ]);
          common.position = { x: pxToInch(layoutBounds.left), y: pxToInch(layoutBounds.top), w: pxToInch(layoutBounds.width), h: pxToInch(layoutBounds.height) };
          common.bbox = { x: pxToInch(bounds.left), y: pxToInch(bounds.top), w: pxToInch(bounds.width), h: pxToInch(bounds.height) };
          if (transform.rotation) common.shape.rotate = transform.rotation;
        } else if (tag === 'line') {
          pushLine(
            transformPoint({ x: svgNumber(node.getAttribute('x1')), y: svgNumber(node.getAttribute('y1')) }, transform.matrix),
            transformPoint({ x: svgNumber(node.getAttribute('x2')), y: svgNumber(node.getAttribute('y2')) }, transform.matrix),
            node,
            stroke,
            lineWidth,
            transform.coordinateSpace,
          );
          return;
        } else if (tag === 'text') {
          common.text = node.textContent || '';
          const fontSize = svgNumber(node.getAttribute('font-size'), 16);
          const origin = mapPoint(node.getAttribute('x'), svgNumber(node.getAttribute('y')) - fontSize);
          const textAnchor = node.getAttribute('text-anchor')
            || view.getComputedStyle(node).textAnchor
            || 'start';
          let measuredBox = null;
          try {
            const bbox = node.getBBox?.();
            if (bbox && [bbox.x, bbox.y, bbox.width, bbox.height].every(Number.isFinite)
              && bbox.width > 0 && bbox.height > 0) {
              const bounds = boundsOfPoints([
                mapPoint(bbox.x, bbox.y),
                mapPoint(bbox.x + bbox.width, bbox.y + bbox.height),
              ]);
              measuredBox = {
                x: bounds.left,
                y: bounds.top,
                width: bounds.width,
                height: bounds.height,
              };
            }
          } catch {
            // Fall through to text length or deterministic character metrics.
          }
          let userWidth = 0;
          if (!measuredBox) {
            try {
              userWidth = Number(node.getComputedTextLength?.()) || 0;
            } catch {
              userWidth = 0;
            }
            if (!(userWidth > 0)) {
              userWidth = [...common.text].reduce((sum, character) => (
                sum + (/[\u1100-\u11ff\u2e80-\u9fff\uf900-\ufaff\uff01-\uff60\uffe0-\uffe6]/u.test(character)
                  ? fontSize
                  : fontSize * 0.6)
              ), 0);
            }
            const textWidth = Math.max(1, userWidth * xScale);
            const anchorOffset = textAnchor === 'middle' ? textWidth / 2 : textAnchor === 'end' ? textWidth : 0;
            measuredBox = {
              x: Math.max(0, origin.x - anchorOffset),
              y: origin.y,
              width: textWidth,
              height: Math.max(1, fontSize * yScale * 1.3),
            };
          }
          common.position = {
            x: pxToInch(measuredBox.x),
            y: pxToInch(measuredBox.y),
            w: pxToInch(measuredBox.width),
            h: pxToInch(measuredBox.height),
          };
          common.style = {
            fontSize: svgNumber(node.getAttribute('font-size'), 16) * 0.75,
            fontFace: 'Arial',
            color: fill || '000000',
            align: { start: 'left', middle: 'center', end: 'right' }[textAnchor] || 'left',
          };
          if (transform.rotation) common.style.rotate = transform.rotation;
        } else {
          const rawPoints = parseSvgPoints(node.getAttribute('points'));
          const points = rawPoints
            .map((point) => transformPoint(point, transform.matrix));
          const preset = tag === 'polygon' ? classifySvgPresetPolygon(points) : null;
          if (preset) {
            const slidePoints = points.map(transformedToSlidePoint);
            const bounds = boundsOfPoints(slidePoints);
            common.svgType = preset.shapeType;
            common.position = {
              x: pxToInch(bounds.left),
              y: pxToInch(bounds.top),
              w: pxToInch(bounds.width),
              h: pxToInch(bounds.height),
            };
            common.bbox = { ...common.position };
            common.shape.rotate = preset.rotate;
          } else {
            const closed = tag === 'polygon' && points.length > 2 ? [...points, points[0]] : points;
            for (let index = 0; index + 1 < closed.length; index += 1) {
              pushLine(
                closed[index],
                closed[index + 1],
                node,
                stroke || fill,
                lineWidth,
                transform.coordinateSpace,
              );
            }
            return;
          }
        }
        if (common.position || common.type === 'line') pushElement(common, node);
      });
      processed.add(svg);
      svg.querySelectorAll('*').forEach((node) => processed.add(node));
    };

    document.querySelectorAll('*').forEach((el) => {
      if (processed.has(el)) return;

      if (el.tagName === 'svg') {
        emitNativeSvg(el);
        return;
      }

      // [data-pptx-merge="true"] — opt-in: merge all <p>/<h1>-<h6> descendants
      // into ONE PowerPoint text frame (single editable text box).
      // Each child paragraph becomes a run with breakLine:true at the end;
      // per-paragraph fontSize/color/bold/italic/underline are preserved as run options.
      // The container's bg/border (if any) still becomes its own shape, same as a normal div.
      if (el.tagName === 'DIV' && el.dataset && el.dataset.pptxMerge === 'true') {
        const containerRect = rectFor(el);
        if (containerRect.width === 0 || containerRect.height === 0) {
          processed.add(el);
          return;
        }

        // Reject nested merge containers — undefined behavior.
        if (el.querySelector('[data-pptx-merge="true"]')) {
          addDiagnostic(
            'blocking',
            'nested_merge_container',
            'Nested data-pptx-merge containers cannot be represented as editable text.',
            el,
          );
          processed.add(el);
          return;
        }

        const mergeComputed = view.getComputedStyle(el);

        // Container background image — same restriction as regular divs.
        if (mergeComputed.backgroundImage && mergeComputed.backgroundImage !== 'none') {
          addDiagnostic(
            'blocking',
            'merge_background_image',
            'Background image on data-pptx-merge cannot be represented as editable objects.',
            el,
          );
        }

        // Emit a shape for the container's bg/uniform-border (mirrors the regular div branch).
        const mHasBg = mergeComputed.backgroundColor && mergeComputed.backgroundColor !== 'rgba(0, 0, 0, 0)';
        const mBorders = [
          mergeComputed.borderTopWidth,
          mergeComputed.borderRightWidth,
          mergeComputed.borderBottomWidth,
          mergeComputed.borderLeftWidth
        ].map(b => parseFloat(b) || 0);
        const mHasBorder = mBorders.some(b => b > 0);
        const mHasUniformBorder = mHasBorder && mBorders.every(b => b === mBorders[0]);

        if (mHasBg || mHasUniformBorder) {
          const mergeRectRadius = (() => {
            const radius = mergeComputed.borderRadius;
            const radiusValue = parseFloat(radius);
            if (radiusValue === 0) return 0;
            if (radius.includes('%')) {
              if (radiusValue >= 50) return 1;
              const minDim = Math.min(containerRect.width, containerRect.height);
              return (radiusValue / 100) * pxToInch(minDim);
            }
            if (radius.includes('pt')) return radiusValue / 72;
            return radiusValue / PX_PER_IN;
          })();
          const mergeShadowEffect = parseBoxShadow(elementBoxShadow(el, mergeComputed));
          if (mergeShadowEffect?.mode === 'ring') {
            pushCssRingBackdrop(el, containerRect, mergeRectRadius, mergeShadowEffect.ring);
          }
          pushElement({
            type: 'shape',
            text: '',
            position: {
              x: pxToInch(containerRect.left),
              y: pxToInch(containerRect.top),
              w: pxToInch(containerRect.width),
              h: pxToInch(containerRect.height)
            },
            shape: {
              fill: mHasBg ? rgbToHex(mergeComputed.backgroundColor) : null,
              transparency: mHasBg ? extractAlpha(mergeComputed.backgroundColor) : null,
              line: mHasUniformBorder ? {
                color: rgbToHex(mergeComputed.borderColor),
                width: pxToPoints(mergeComputed.borderWidth)
              } : null,
              rectRadius: mergeRectRadius,
              shadow: outerShadowOf(mergeShadowEffect),
            }
          }, el);
        }

        // Collect <p>/<h*> descendants in document order.
        const textDescendants = Array.from(el.querySelectorAll('p, h1, h2, h3, h4, h5, h6'));
        if (textDescendants.length === 0) {
          addDiagnostic(
            'blocking',
            'empty_merge_container',
            'data-pptx-merge container has no semantic text children.',
            el,
          );
          processed.add(el);
          return;
        }

        // Use the first text element's computed style as the textbox-level base
        // (align / lineSpacing / paraSpace are paragraph/textbox-level in pptxgenjs, not per-run).
        const firstComputed = view.getComputedStyle(textDescendants[0]);
        // Preserve the merge container's padding as PPTX internal margin so
        // text stays inset from the frame edges like it does in HTML.
        // (mergeComputed was declared earlier — line ~642)
        const mPadL = pxToInch(parseFloat(mergeComputed.paddingLeft) || 0);
        const mPadR = pxToInch(parseFloat(mergeComputed.paddingRight) || 0);
        const mPadT = pxToInch(parseFloat(mergeComputed.paddingTop) || 0);
        const mPadB = pxToInch(parseFloat(mergeComputed.paddingBottom) || 0);
        const baseStyle = {
          fontSize: pxToPoints(firstComputed.fontSize) || 12,
          fontFace: firstComputed.fontFamily.split(',')[0].replace(/['"]/g, '').trim() || 'Arial',
          color: rgbToHex(firstComputed.color),
          align: !firstComputed.textAlign || firstComputed.textAlign === 'start'
            ? 'left'
            : firstComputed.textAlign,
          lineSpacing: resolveLineSpacing(firstComputed),
          paraSpaceBefore: 0,
          paraSpaceAfter: 0,
          margin: [mPadT, mPadR, mPadB, mPadL].some((v) => v > 0) ? [mPadT, mPadR, mPadB, mPadL] : 0,
        };
        const baseTransparency = extractAlpha(firstComputed.color);
        if (baseTransparency !== null) baseStyle.transparency = baseTransparency;

        // Build the merged runs.
        const mergedRuns = [];
        textDescendants.forEach((textEl, idx) => {
          const isLast = idx === textDescendants.length - 1;
          const tComputed = view.getComputedStyle(textEl);
          const transformStr = tComputed.textTransform;

          // Per-paragraph style overrides — only include if they differ from base.
          const elemFontSize = pxToPoints(tComputed.fontSize) || baseStyle.fontSize;
          const elemFontFace = tComputed.fontFamily.split(',')[0].replace(/['"]/g, '').trim()
            || baseStyle.fontFace;
          const elemColor = rgbToHex(tComputed.color);
          const elemBold = tComputed.fontWeight === 'bold' || parseInt(tComputed.fontWeight) >= 600;
          const elemItalic = tComputed.fontStyle === 'italic';
          const elemUnderline = tComputed.textDecoration.includes('underline');

          const runBaseOptions = {};
          if (elemFontSize !== baseStyle.fontSize) runBaseOptions.fontSize = elemFontSize;
          if (elemFontFace !== baseStyle.fontFace) runBaseOptions.fontFace = elemFontFace;
          if (elemColor !== baseStyle.color) runBaseOptions.color = elemColor;
          if (elemBold && !shouldSkipBold(tComputed.fontFamily)) runBaseOptions.bold = true;
          if (elemItalic) runBaseOptions.italic = true;
          if (elemUnderline) runBaseOptions.underline = true;

          const hasInline = textEl.querySelector('b, i, u, strong, em, span, small, label, a, code, mark, sub, sup, br');
          let runs;
          if (hasInline) {
            runs = parseInlineFormatting(
              textEl,
              runBaseOptions,
              [],
              (str) => applyTextTransform(str, transformStr)
            );
          } else {
            const txt = applyTextTransform(textEl.textContent.trim(), transformStr);
            if (!txt) return;
            runs = [{ text: txt, options: { ...runBaseOptions } }];
          }

          if (runs.length > 0 && !isLast) {
            runs[runs.length - 1].options.breakLine = true;
          }
          mergedRuns.push(...runs);
          processed.add(textEl);
        });

        if (mergedRuns.length === 0) {
          processed.add(el);
          return;
        }

        pushElement({
          type: 'merged-text',
          items: mergedRuns,
          position: {
            x: pxToInch(containerRect.left),
            y: pxToInch(containerRect.top),
            w: pxToInch(containerRect.width),
            h: pxToInch(containerRect.height)
          },
          style: baseStyle
        }, el);

        processed.add(el);
        return;
      }

      const genericTextExtracted = isGenericTextElement(el)
        ? emitTextElement(
            el,
            'text',
            true,
            genericInlineTextTags.has(el.tagName) ? textRectFor(el) : null,
          )
        : false;

      // Text tags with decorative boxes (pills, chips) become a shape + text.
      if (textTags.includes(el.tagName) || genericInlineTextTags.has(el.tagName)) {
        const computed = view.getComputedStyle(el);
        const hasBg = computed.backgroundColor && computed.backgroundColor !== 'rgba(0, 0, 0, 0)';
        const hasBorder = (computed.borderWidth && parseFloat(computed.borderWidth) > 0) ||
                          (computed.borderTopWidth && parseFloat(computed.borderTopWidth) > 0) ||
                          (computed.borderRightWidth && parseFloat(computed.borderRightWidth) > 0) ||
                          (computed.borderBottomWidth && parseFloat(computed.borderBottomWidth) > 0) ||
                          (computed.borderLeftWidth && parseFloat(computed.borderLeftWidth) > 0);
        const hasShadow = computed.boxShadow && computed.boxShadow !== 'none';

        if (hasBg || hasBorder || hasShadow) {
          const decoRect = rectFor(el);
          if (decoRect.width > 0 && decoRect.height > 0) {
            const borders = [computed.borderTopWidth, computed.borderRightWidth, computed.borderBottomWidth, computed.borderLeftWidth]
              .map((b) => parseFloat(b) || 0);
            const hasUniformBorder = borders.some((b) => b > 0) && borders.every((b) => b === borders[0]);
            const solid = hasBg ? resolveSolidFill(computed.backgroundColor, slideBackdropHex) : { fill: null, transparency: null };
            if (solid.fill || hasUniformBorder) {
              const radius = computed.borderRadius;
              const radiusValue = parseFloat(radius);
              const decoRectRadius = (() => {
                if (!radiusValue) return 0;
                if (radius.includes('%')) {
                  if (radiusValue >= 50) return 1;
                  const minDim = Math.min(decoRect.width, decoRect.height);
                  return (radiusValue / 100) * pxToInch(minDim);
                }
                if (radius.includes('pt')) return radiusValue / 72;
                return radiusValue / PX_PER_IN;
              })();
              const decoShadowEffect = parseBoxShadow(elementBoxShadow(el, computed));
              if (decoShadowEffect?.mode === 'ring') {
                pushCssRingBackdrop(el, decoRect, decoRectRadius, decoShadowEffect.ring);
              }
              pushElement({
                type: 'shape',
                text: '',
                position: {
                  x: pxToInch(decoRect.left),
                  y: pxToInch(decoRect.top),
                  w: pxToInch(decoRect.width),
                  h: pxToInch(decoRect.height),
                },
                shape: {
                  fill: solid.fill,
                  transparency: solid.transparency,
                  line: hasUniformBorder ? {
                    color: rgbToHex(computed.borderColor),
                    width: pxToPoints(computed.borderWidth),
                  } : null,
                  rectRadius: decoRectRadius,
                  shadow: outerShadowOf(decoShadowEffect),
                },
              }, el);
            }
          }
        }
      }

      // Extract placeholder elements (for charts, etc.)
      // Use classList.contains — el.className is a string for HTML elements but an
      // SVGAnimatedString object for SVG elements, so .includes() would throw
      // "className.includes is not a function" when slides contain inline SVG.
      if (el.classList && el.classList.contains('placeholder')) {
        const rect = rectFor(el);
        if (rect.width === 0 || rect.height === 0) {
          addDiagnostic(
            'blocking',
            'unmeasurable_placeholder',
            `Placeholder "${el.id || 'unnamed'}" has ${rect.width === 0 ? 'width: 0' : 'height: 0'}.`,
            el,
          );
        } else {
          placeholders.push({
            id: el.id || `placeholder-${placeholders.length}`,
            x: pxToInch(rect.left),
            y: pxToInch(rect.top),
            w: pxToInch(rect.width),
            h: pxToInch(rect.height)
          });
        }
        processed.add(el);
        return;
      }

      // Extract images
      if (el.tagName === 'IMG') {
        const rect = rectFor(el);
        if (rect.width > 0 && rect.height > 0) {
          pushElement({
            type: 'image',
            src: el.src,
            position: {
              x: pxToInch(rect.left),
              y: pxToInch(rect.top),
              w: pxToInch(rect.width),
              h: pxToInch(rect.height)
            }
          }, el);
          processed.add(el);
          return;
        }
      }

      // Common CSS arrow: an empty transparent box with one opaque border and
      // the two perpendicular transparent side borders maps cleanly to an
      // editable PPT triangle. WebKit may report the total border-box size as
      // computed width/height for authored zero-content border boxes, so the
      // classification must use border semantics rather than computed size.
      if (el.tagName === 'DIV') {
        const computed = view.getComputedStyle(el);
        const isZeroBox = svgNumber(computed.width) === 0 && svgNumber(computed.height) === 0;
        const sides = ['Top', 'Right', 'Bottom', 'Left'].map((side) => ({
          side,
          width: svgNumber(computed[`border${side}Width`]),
          color: computed[`border${side}Color`],
        }));
        const opaqueSides = sides.filter((side) => (
          side.width > 0 && !isTransparentBg(side.color)
        ));
        const transparentSides = sides.filter((side) => (
          side.width > 0 && isTransparentBg(side.color)
        ));
        const perpendicularSides = {
          Bottom: ['Left', 'Right'],
          Left: ['Top', 'Bottom'],
          Top: ['Left', 'Right'],
          Right: ['Top', 'Bottom'],
        };
        const active = opaqueSides[0];
        const requiredTransparentSides = active ? perpendicularSides[active.side] : [];
        const hasNoVisibleContent = !el.textContent.replace(/\s+/g, '').length
          && el.children.length === 0;
        const hasTransparentBackground = isTransparentBg(elementBackgroundColor(el, computed));
        const formsDirectionalTriangle = requiredTransparentSides.length === 2
          && requiredTransparentSides.every((side) => (
            transparentSides.some((candidate) => candidate.side === side)
          ));
        const isSemanticCssTriangle = hasNoVisibleContent
          && hasTransparentBackground
          && opaqueSides.length === 1
          && transparentSides.length >= 2
          && formsDirectionalTriangle;
        if (isSemanticCssTriangle) {
          const rect = rectFor(el);
          const rotations = { Bottom: 0, Left: 90, Top: 180, Right: 270 };
          pushElement({
            type: 'svg-shape',
            svgType: 'triangle',
            kind: 'native',
            text: '',
            position: {
              x: pxToInch(rect.left),
              y: pxToInch(rect.top),
              w: pxToInch(Math.max(1, rect.width)),
              h: pxToInch(Math.max(1, rect.height)),
            },
            shape: {
              fill: rgbToHex(active.color),
              line: null,
              transparency: extractAlpha(active.color),
              rectRadius: 0,
              rotate: rotations[active.side],
            },
          }, el);
          processed.add(el);
          return;
        }
      }

      // Extract container blocks with backgrounds/borders as shapes
      const isContainer = containerTags.has(el.tagName);
      if (isContainer) {
        const computed = view.getComputedStyle(el);
        const backgroundColor = elementBackgroundColor(el, computed);
        const hasBg = !isTransparentBg(backgroundColor);

        // Check for background images on shapes
        const bgImage = computed.backgroundImage;
        if (bgImage && bgImage !== 'none') {
          addDiagnostic(
            'blocking',
            'container_background_image',
            'Container background image cannot be represented as editable objects.',
            el,
          );
        }

        // Check for borders - both uniform and partial
        const borderTop = computed.borderTopWidth;
        const borderRight = computed.borderRightWidth;
        const borderBottom = computed.borderBottomWidth;
        const borderLeft = computed.borderLeftWidth;
        const borders = [borderTop, borderRight, borderBottom, borderLeft].map(b => parseFloat(b) || 0);
        const hasBorder = borders.some(b => b > 0);
        const hasUniformBorder = hasBorder && borders.every(b => b === borders[0]);
        const borderLines = [];

        if (hasBorder && !hasUniformBorder) {
          const rect = rectFor(el);
          const x = pxToInch(rect.left);
          const y = pxToInch(rect.top);
          const w = pxToInch(rect.width);
          const h = pxToInch(rect.height);

          // Collect lines to add after shape (inset by half the line width to center on edge)
          if (parseFloat(borderTop) > 0) {
            const widthPt = pxToPoints(borderTop);
            const inset = (widthPt / 72) / 2; // Convert points to inches, then half
            borderLines.push({
              type: 'line',
              x1: x, y1: y + inset, x2: x + w, y2: y + inset,
              width: widthPt,
              color: rgbToHex(computed.borderTopColor)
            });
          }
          if (parseFloat(borderRight) > 0) {
            const widthPt = pxToPoints(borderRight);
            const inset = (widthPt / 72) / 2;
            borderLines.push({
              type: 'line',
              x1: x + w - inset, y1: y, x2: x + w - inset, y2: y + h,
              width: widthPt,
              color: rgbToHex(computed.borderRightColor)
            });
          }
          if (parseFloat(borderBottom) > 0) {
            const widthPt = pxToPoints(borderBottom);
            const inset = (widthPt / 72) / 2;
            borderLines.push({
              type: 'line',
              x1: x, y1: y + h - inset, x2: x + w, y2: y + h - inset,
              width: widthPt,
              color: rgbToHex(computed.borderBottomColor)
            });
          }
          if (parseFloat(borderLeft) > 0) {
            const widthPt = pxToPoints(borderLeft);
            const inset = (widthPt / 72) / 2;
            borderLines.push({
              type: 'line',
              x1: x + inset, y1: y, x2: x + inset, y2: y + h,
              width: widthPt,
              color: rgbToHex(computed.borderLeftColor)
            });
          }
        }

        if (hasBg || hasBorder) {
          const rect = rectFor(el);
          const coversSlide = rect.width >= bodyRect.width * 0.97
            && rect.height >= bodyRect.height * 0.97;
          if (coversSlide && hasBg) {
            processed.add(el);
            return;
          }
          if (rect.width > 0 && rect.height > 0) {
            const shadowEffect = parseBoxShadow(elementBoxShadow(el, computed));
            const rotation = getRotation(computed.transform)
              || getRotation(el.style?.transform)
              || getRotation(authoredStyleValue(el, 'transform'));
            const placed = getPositionAndSize(el, rect, rotation);

            // Only add shape if there's background or uniform border
            if (hasBg || hasUniformBorder) {
              const solid = hasBg
                ? resolveSolidFill(backgroundColor, slideBackdropHex)
                : { fill: null, transparency: null };
              let fillHex = solid.fill;
              let fillTransparency = solid.transparency;
              if (!fillHex && hasUniformBorder) {
                fillHex = rgbToHex(computed.borderColor) || '2A2A30';
                fillTransparency = fillTransparency ?? 88;
              }
              // Convert border-radius to rectRadius (in inches)
              // % values: 50%+ = circle (1), <50% = percentage of min dimension
              // pt values: divide by 72 (72pt = 1 inch)
              // px values: divide by 96 (96px = 1 inch)
              const shapeRectRadius = (() => {
                const radius = computed.borderRadius
                  || el.style.borderRadius
                  || authoredStyleValue(el, 'border-radius');
                const radiusValue = parseFloat(radius);
                if (radiusValue === 0) return 0;

                if (radius.includes('%')) {
                  if (radiusValue >= 50) return 1;
                  // Calculate percentage of smaller dimension
                  const minDim = Math.min(placed.w, placed.h);
                  return (radiusValue / 100) * pxToInch(minDim);
                }

                if (radius.includes('pt')) return radiusValue / 72;
                return radiusValue / PX_PER_IN;
              })();
              const placedRect = {
                left: placed.x,
                top: placed.y,
                width: placed.w,
                height: placed.h,
              };
              if (shadowEffect?.mode === 'ring') {
                pushCssRingBackdrop(el, placedRect, shapeRectRadius, shadowEffect.ring);
              }
              pushElement({
                type: 'shape',
                text: '',  // Shape only - child text elements render on top
                position: {
                  x: pxToInch(placed.x),
                  y: pxToInch(placed.y),
                  w: pxToInch(placed.w),
                  h: pxToInch(placed.h)
                },
                shape: {
                  fill: fillHex,
                  transparency: fillTransparency,
                  line: hasUniformBorder ? {
                    color: rgbToHex(computed.borderColor),
                    width: pxToPoints(computed.borderWidth)
                  } : null,
                  rectRadius: shapeRectRadius,
                  shadow: outerShadowOf(shadowEffect),
                  ...(rotation != null ? { rotate: rotation } : {}),
                }
              }, el);
            }

            // Add partial border lines
            borderLines.forEach((line) => pushElement(line, el));

            processed.add(el);
            return;
          }
        }
      }

      if (genericTextExtracted) return;

      // Extract bullet lists as single text block
      if (el.tagName === 'UL' || el.tagName === 'OL') {
        const rect = rectFor(el);
        if (rect.width === 0 || rect.height === 0) return;

        // Prefer real <li> children. After repairNestedParagraphs unwraps
        // <li><p>…</p></li> into sibling <p> under the UL, fall back to those
        // direct semantic blocks so dense list body text is not lost.
        const liElements = Array.from(el.querySelectorAll(':scope > li'));
        const listItemElements = liElements.length
          ? liElements
          : Array.from(el.querySelectorAll(':scope > p, :scope > h1, :scope > h2, :scope > h3, :scope > h4, :scope > h5, :scope > h6'));
        const items = [];
        const ulComputed = view.getComputedStyle(el);
        // jsdom / stripped lists may report paddingLeft as "" → NaN. A non-finite
        // indent makes scene validation reject the whole list payload.
        const ulPaddingLeftPt = (() => {
          const value = pxToPoints(ulComputed.paddingLeft);
          return Number.isFinite(value) && value >= 0 ? value : 18;
        })();

        // Split: margin-left for bullet position, indent for text position
        // margin-left + indent = ul padding-left
        const marginLeft = ulPaddingLeftPt * 0.5;
        const textIndent = ulPaddingLeftPt * 0.5;

        const computed = view.getComputedStyle(listItemElements[0] || el);
        const textHex = rgbToHex(computed.color);
        const bulletColor = resolveListBulletColor(el, liElements, textHex);

        listItemElements.forEach((itemEl, idx) => {
          const isLast = idx === listItemElements.length - 1;
          const runs = parseInlineFormatting(itemEl, { breakLine: false });
          // Clean manual bullets from first run
          if (runs.length > 0) {
            runs[0].text = runs[0].text.replace(/^[•\-\*▪▸]\s*/, '');
            runs[0].options.bullet = { indent: textIndent };
          }
          if (runs.length > 0 && bulletColor && bulletColor !== textHex) {
            runs.unshift({
              text: '\u200B',
              options: {
                bullet: { indent: textIndent },
                color: bulletColor,
                fontSize: runs[0]?.options?.fontSize || pxToPoints(computed.fontSize),
                breakLine: false,
              },
            });
          }
          // Set breakLine on last run
          if (runs.length > 0 && !isLast) {
            runs[runs.length - 1].options.breakLine = true;
          }
          items.push(...runs);
        });

        const listFrame = expandTextFrame(el, rect, null);

        // UL/OL padding: paddingLeft is already split into bullet margin +
        // text indent below. The remaining padding (top/right/bottom) must
        // be preserved as PPTX internal margin so text doesn't shift.
        const inchPad = (value) => {
          const px = parseFloat(value);
          return Number.isFinite(px) && px > 0 ? pxToInch(px) : 0;
        };
        const ulPadR = inchPad(ulComputed.paddingRight);
        const ulPadT = inchPad(ulComputed.paddingTop);
        const ulPadB = inchPad(ulComputed.paddingBottom);
        const listCharSpacing = resolveCharSpacing(computed);

        // Empty list payloads fail scene validation; degrade then deletes the
        // whole UL/OL and every nested body line with it. Leave the list
        // unmarked so nested semantic paragraphs can still export.
        if (!items.length) {
          listItemElements.forEach((itemEl) => processed.add(itemEl));
          return;
        }

        pushElement({
          type: 'list',
          items: items,
          position: {
            x: pxToInch(listFrame.x),
            y: pxToInch(listFrame.y),
            w: pxToInch(listFrame.w),
            h: pxToInch(listFrame.h)
          },
          style: {
            fontSize: pxToPoints(computed.fontSize) || 12,
            fontFace: computed.fontFamily.split(',')[0].replace(/['"]/g, '').trim() || 'Arial',
            color: textHex,
            ...(bulletColor ? { bulletColor } : {}),
            ...(extractAlpha(computed.color) != null
              ? { transparency: extractAlpha(computed.color) }
              : {}),
            align: resolveTextAlign(computed),
            lineSpacing: resolveLineSpacing(computed),
            paraSpaceBefore: 0,
            paraSpaceAfter: pxToPoints(computed.marginBottom) || 0,
            // PptxGenJS margin array is [top, right, bottom, left].
            margin: [ulPadT, ulPadR, ulPadB, marginLeft / 72],
            ...(listCharSpacing !== undefined ? { charSpacing: listCharSpacing } : {}),
          }
        }, el);

        listItemElements.forEach((itemEl) => {
          processed.add(itemEl);
          // Nested semantic blocks were flattened into the list runs above.
          itemEl.querySelectorAll('p, h1, h2, h3, h4, h5, h6').forEach((child) => processed.add(child));
        });
        // Any leftover LI wrappers (when items came from unwrapped paragraphs)
        // must not emit a second empty list pass.
        liElements.forEach((li) => processed.add(li));
        processed.add(el);
        return;
      }

      // Extract text elements (P, H1, H2, etc.)
      if (!textTags.includes(el.tagName)) return;

      emitTextElement(el);
    });

    elements.sort((a, b) => {
      const z = (a.zIndex ?? 0) - (b.zIndex ?? 0);
      if (z !== 0) return z;
      const paint = (a.paintOrder ?? 0) - (b.paintOrder ?? 0);
      if (paint !== 0) return paint;
      const sub = (a.subOrder ?? 0) - (b.subOrder ?? 0);
      if (sub !== 0) return sub;
      return (a.stableOrder ?? 0) - (b.stableOrder ?? 0);
    });

    document.querySelectorAll('*').forEach((element) => {
      const computed = view.getComputedStyle(element);
      const filter = String(computed.filter || element.style?.filter || '');
      if (filter && filter !== 'none') {
        addDiagnostic('blocking', 'css_filter', 'CSS filter cannot be represented as editable objects.', element);
      }
      if (String(element.tagName).toUpperCase() === 'SVG') {
        if (element.querySelector('filter,mask,foreignObject,use,pattern,textPath,clipPath,image')) {
          addDiagnostic(
            'blocking',
            'complex_svg_unsupported',
            'SVG filter, mask, or foreignObject cannot be represented as editable objects.',
            element,
          );
        } else if (element.querySelector('path')) {
          addDiagnostic(
            'rewrite',
            'svg_path_rewrite',
            'SVG path geometry will be rewritten as editable line segments.',
            element,
          );
        }
      }
    });

    const blockingErrors = diagnostics
      .filter((diagnostic) => diagnostic.severity === 'blocking')
      .map((diagnostic) => diagnostic.message);
    return {
      background,
      elements,
      placeholders,
      diagnostics,
      errors: blockingErrors,
    };
  
}
