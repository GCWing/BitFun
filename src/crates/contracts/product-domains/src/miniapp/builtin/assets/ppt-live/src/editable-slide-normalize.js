import { EditableExportError, validateEditableSlideScene } from './editable-slide-scene.js';
import {
  classifySvgPresetPolygon,
  createSvgViewportMapper,
  extractSlideDataFromDocument,
  measureBodyDimensions,
  PX_PER_IN,
} from './html2pptx-dom-core.js';

const GRADIENT_GRID_SIZE = 16;
const CURVE_MAX_SEGMENTS = 64;

function sourceIdOf(element, fallback = 'slide-document') {
  return element?.dataset?.pptxSourceId || element?.id || fallback;
}

function fail(slideNumber, element, code, message) {
  throw new EditableExportError({
    slideNumber,
    sourceId: sourceIdOf(element, `slide-${slideNumber}`),
    code,
    message,
  });
}

function failSource(slideNumber, sourceId, code, message) {
  throw new EditableExportError({
    slideNumber,
    sourceId: sourceId || `slide-${slideNumber}`,
    code,
    message,
  });
}

function isTransparent(value) {
  return !value
    || value === 'none'
    || value === 'transparent'
    || /^rgba\([^)]*,\s*0(?:\.0+)?\s*\)$/i.test(value);
}

function parseColor(value) {
  const raw = String(value || '').trim().toLowerCase();
  const named = {
    black: '000000', white: 'FFFFFF', red: 'FF0000', green: '008000',
    blue: '0000FF', yellow: 'FFFF00', gray: '808080', grey: '808080',
  };
  if (raw === 'transparent') return { hex: '000000', alpha: 0 };
  if (named[raw]) return { hex: named[raw], alpha: 1 };
  const hex = raw.match(/^#([\da-f]{3}|[\da-f]{4}|[\da-f]{6}|[\da-f]{8})$/i)?.[1];
  if (hex) {
    const expanded = hex.length <= 4
      ? hex.split('').map((channel) => channel.repeat(2)).join('')
      : hex;
    return {
      hex: expanded.slice(0, 6).toUpperCase(),
      alpha: expanded.length === 8 ? parseInt(expanded.slice(6, 8), 16) / 255 : 1,
    };
  }
  const rgb = raw.match(
    /^rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)(?:\s*[,/]\s*([\d.]+%?))?\s*\)$/i,
  );
  if (rgb) {
    const channels = rgb.slice(1, 4).map(Number);
    if (channels.some((channel) => !Number.isFinite(channel) || channel < 0 || channel > 255)) {
      return null;
    }
    let alpha = 1;
    if (rgb[4] != null) {
      alpha = rgb[4].endsWith('%') ? Number.parseFloat(rgb[4]) / 100 : Number(rgb[4]);
      if (!Number.isFinite(alpha) || alpha < 0 || alpha > 1) return null;
    }
    return {
      hex: channels.map((channel) => Math.round(channel).toString(16).padStart(2, '0'))
        .join('').toUpperCase(),
      alpha,
    };
  }
  return null;
}

function colorToHex(value, fallback = null) {
  return parseColor(value)?.hex || fallback;
}

function elementBox(element, bodyRect) {
  const rect = element.getBoundingClientRect();
  return {
    x: Math.max(0, rect.left - bodyRect.left) / PX_PER_IN,
    y: Math.max(0, rect.top - bodyRect.top) / PX_PER_IN,
    w: Math.max(1, rect.width) / PX_PER_IN,
    h: Math.max(1, rect.height) / PX_PER_IN,
  };
}

function generatedContentOwner(doc, selector) {
  const raw = String(selector || '').trim();
  const candidates = [raw];
  const simpleSelectors = raw.match(/(?:[.#][\w-]+|\[[^\]]+\]|[a-z][\w-]*)/gi);
  if (simpleSelectors?.length) candidates.push(simpleSelectors.at(-1));
  for (const candidate of candidates) {
    try {
      const owner = doc.querySelector(candidate);
      if (owner) return owner;
    } catch {
      // Try the next selector candidate.
    }
  }
  return null;
}

function parsePolygonPoints(element) {
  const values = String(element.getAttribute('points') || '').trim()
    .split(/[\s,]+/).map(Number).filter(Number.isFinite);
  const points = [];
  for (let index = 0; index + 1 < values.length; index += 2) {
    points.push({ x: values[index], y: values[index + 1] });
  }
  return points;
}

function validateSvgPresetGeometry(doc, slideNumber) {
  for (const polygon of doc.querySelectorAll('svg polygon')) {
    if (!classifySvgPresetPolygon(parsePolygonPoints(polygon))) {
      fail(slideNumber, polygon, 'svg_polygon_unsupported',
        'SVG polygon geometry does not exactly match an editable PowerPoint triangle or diamond preset.');
    }
  }
  for (const rect of doc.querySelectorAll('svg rect[rx], svg rect[ry]')) {
    const width = Number(rect.getAttribute('width'));
    const height = Number(rect.getAttribute('height'));
    const rawRx = rect.hasAttribute('rx') ? Number(rect.getAttribute('rx')) : null;
    const rawRy = rect.hasAttribute('ry') ? Number(rect.getAttribute('ry')) : null;
    const rx = rawRx ?? rawRy;
    const ry = rawRy ?? rawRx;
    const maxRadius = Math.min(width, height) / 2;
    const tolerance = Math.max(1, Math.min(width, height)) * 1e-6;
    const svg = rect.closest('svg');
    const viewportMapper = createSvgViewportMapper(svg, svg.getBoundingClientRect());
    const hasNonuniformRadiusScale = Math.abs(viewportMapper.xScale - viewportMapper.yScale)
      > Math.max(viewportMapper.xScale, viewportMapper.yScale, 1) * 1e-6;
    if (![width, height, rx, ry, maxRadius].every(Number.isFinite)
      || width <= 0 || height <= 0 || rx <= 0 || ry <= 0
      || Math.abs(rx - ry) > tolerance
      || rx - maxRadius > tolerance
      || hasNonuniformRadiusScale) {
      fail(slideNumber, rect, 'svg_round_rect_radius_unsupported',
        'SVG rect radius must be equal on both axes and within half the shorter side.');
    }
  }
}

function editableErrorPreflight(doc, slideNumber) {
  const view = doc.defaultView;
  for (const element of [doc.body, ...doc.querySelectorAll('*')]) {
    const computed = view.getComputedStyle(element);
    const backgroundImage = String(computed.backgroundImage || '').trim();
    if (backgroundImage && backgroundImage !== 'none' && !backgroundImage.includes('gradient(')) {
      fail(slideNumber, element, 'background_image_unsupported',
        'CSS background images cannot be represented as intentional editable picture nodes; use an img element.');
    }
    const filter = String(computed.filter || element.style?.filter || '').trim();
    if (filter && filter !== 'none') {
      fail(slideNumber, element, 'css_filter', 'CSS filters cannot be represented by editable PowerPoint objects.');
    }
    const animationName = String(computed.animationName || element.style?.animationName || '').trim();
    const animation = String(element.style?.animation || '').trim();
    if ((animationName && animationName !== 'none') || animation) {
      fail(slideNumber, element, 'animation_unsupported', 'CSS animation cannot be represented in a static editable slide.');
    }
    const mask = [
      computed.mask,
      computed.maskImage,
      element.style?.getPropertyValue?.('mask'),
      element.style?.getPropertyValue?.('mask-image'),
      element.getAttribute?.('style')?.match(/(?:^|;)\s*(?:-webkit-)?mask(?:-image)?\s*:\s*([^;]+)/i)?.[1],
    ].find((value) => value && value !== 'none');
    if (mask) {
      fail(slideNumber, element, 'css_mask', 'CSS masks cannot be represented by editable PowerPoint objects.');
    }
    for (const attribute of ['src', 'href', 'xlink:href']) {
      const value = element.getAttribute?.(attribute);
      if (value && /^(?:https?:|\/\/|file:|blob:)/i.test(value.trim())) {
        fail(slideNumber, element, 'external_resource', 'External resources are not allowed in editable slide export.');
      }
    }
    const cssText = element.getAttribute?.('style') || '';
    if (/url\(\s*["']?(?!#|data:image\/(?:png|jpeg|jpg|webp);base64,)/i.test(cssText)) {
      fail(slideNumber, element, 'external_resource', 'External CSS resources are not allowed in editable slide export.');
    }
  }

  const defsGeometry = doc.querySelector(
    'svg defs rect, svg defs circle, svg defs ellipse, svg defs line, '
      + 'svg defs polyline, svg defs polygon, svg defs path, svg defs text',
  );
  if (defsGeometry) {
    fail(slideNumber, defsGeometry, 'svg_defs_geometry_unsupported',
      'SVG geometry inside defs cannot be emitted as editable slide objects.');
  }
  for (const element of doc.querySelectorAll('svg [fill], svg [stroke]')) {
    const paint = ['fill', 'stroke'].find((attribute) => (
      /\burl\s*\(\s*["']?#[^)]+\)/i.test(element.getAttribute(attribute) || '')
    ));
    if (paint) {
      fail(slideNumber, element, 'svg_paint_server_unsupported',
        `SVG ${paint} paint servers cannot be represented as editable PowerPoint colors.`);
    }
  }

  const masked = doc.querySelector('svg [mask]') || doc.querySelector('svg mask');
  if (masked) {
    fail(slideNumber, masked, 'svg_mask', 'SVG masks cannot be represented by editable PowerPoint objects.');
  }
  const filtered = doc.querySelector('svg [filter], svg filter, svg foreignObject, svg pattern, svg use, svg image');
  if (filtered) {
    const external = filtered.matches('image,use')
      && /^(?!#)(?:https?:|\/\/|file:|blob:|data:)/i.test(
        filtered.getAttribute('href') || filtered.getAttribute('xlink:href') || '',
      );
    fail(
      slideNumber,
      filtered,
      external ? 'external_resource' : 'svg_filter_unsupported',
      'This SVG resource construct cannot be represented by editable PowerPoint objects.',
    );
  }
  const animated = doc.querySelector('animate, animateMotion, animateTransform, set');
  if (animated) {
    fail(slideNumber, animated.parentElement || animated, 'animation_unsupported',
      'SVG animation cannot be represented in a static editable slide.');
  }

  const inspectGeneratedContentRules = (rules) => {
    for (const rule of rules || []) {
      if (rule.cssRules) {
        inspectGeneratedContentRules(rule.cssRules);
        continue;
      }
      const selector = rule.selectorText || '';
      const content = rule.style?.content;
      if (!/::?(?:before|after)\b/i.test(selector)
        || !content || ['none', 'normal', '""', "''"].includes(content.trim())) continue;
      const baseSelector = selector.replace(/::?(?:before|after)\b.*$/i, '').trim();
      const owner = generatedContentOwner(doc, baseSelector);
      fail(slideNumber, owner || doc.body, 'generated_content',
        'Generated pseudo-element content cannot be represented as an editable object.');
    }
  };
  for (const sheet of doc.styleSheets || []) {
    try {
      inspectGeneratedContentRules(sheet.cssRules);
    } catch {
      // Cross-origin stylesheets are unavailable to CSSOM and cannot occur after sanitization.
    }
  }
  for (const styleElement of doc.querySelectorAll('style')) {
    const cssText = styleElement.textContent || '';
    const pseudoRules = cssText.matchAll(
      /([^{}]+)::?(?:before|after)\s*\{[^{}]*\bcontent\s*:\s*(?!none|normal)([^;}]+)/gi,
    );
    for (const match of pseudoRules) {
      const baseSelector = match[1].trim().replace(/^.*[;}]\s*/, '');
      const owner = generatedContentOwner(doc, baseSelector);
      fail(slideNumber, owner || doc.body, 'generated_content',
        'Generated pseudo-element content cannot be represented as an editable object.');
    }
  }
}

function splitGradientArguments(value) {
  const inner = value.slice(value.indexOf('(') + 1, value.lastIndexOf(')'));
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index < inner.length; index += 1) {
    if (inner[index] === '(') depth += 1;
    else if (inner[index] === ')') depth -= 1;
    else if (inner[index] === ',' && depth === 0) {
      parts.push(inner.slice(start, index).trim());
      start = index + 1;
    }
  }
  parts.push(inner.slice(start).trim());
  return parts;
}

function hexChannels(hex) {
  return [0, 2, 4].map((offset) => parseInt(hex.slice(offset, offset + 2), 16));
}

function interpolateColor(left, right, amount) {
  const alpha = left.alpha + (right.alpha - left.alpha) * amount;
  const leftChannels = hexChannels(left.hex);
  const rightChannels = hexChannels(right.hex);
  const channels = leftChannels.map((channel, index) => {
    if (alpha <= 0) return 0;
    const premultiplied = channel * left.alpha
      + (rightChannels[index] * right.alpha - channel * left.alpha) * amount;
    return Math.round(premultiplied / alpha);
  });
  return {
    hex: channels.map((channel) => channel.toString(16).padStart(2, '0')).join('').toUpperCase(),
    alpha,
  };
}

function parseLinearGradient(value) {
  const args = splitGradientArguments(value);
  let angle = 180;
  const angleMatch = args[0]?.match(/^(-?(?:\d*\.)?\d+)(deg|turn|rad|grad)$/i);
  if (angleMatch) {
    args.shift();
    const amount = Number(angleMatch[1]);
    angle = {
      deg: amount,
      turn: amount * 360,
      rad: amount * 180 / Math.PI,
      grad: amount * 0.9,
    }[angleMatch[2].toLowerCase()];
  } else if (/^to\s+/i.test(args[0])) {
    const direction = args.shift().toLowerCase();
    const resolved = {
      'to top': 0,
      'to top right': 45,
      'to right top': 45,
      'to right': 90,
      'to bottom right': 135,
      'to right bottom': 135,
      'to bottom': 180,
      'to bottom left': 225,
      'to left bottom': 225,
      'to left': 270,
      'to top left': 315,
      'to left top': 315,
    }[direction];
    if (resolved == null) return null;
    angle = resolved;
  } else if (/^-?(?:\d*\.)?\d/.test(args[0] || '')) {
    return null;
  }
  const stops = args.map((stop) => {
    const match = stop.match(/^(.*?)(?:\s+(-?(?:\d*\.)?\d+)%\s*)?$/);
    const color = parseColor(match?.[1]);
    return {
      color,
      offset: match?.[2] == null ? null : Number(match[2]) / 100,
    };
  });
  if (stops.length < 2 || stops.some((stop) => !stop.color)) return null;
  if (stops[0].offset == null) stops[0].offset = 0;
  if (stops.at(-1).offset == null) stops.at(-1).offset = 1;
  for (let index = 0; index < stops.length;) {
    if (stops[index].offset != null) {
      stops[index].offset = Math.max(index ? stops[index - 1].offset : 0, stops[index].offset);
      index += 1;
      continue;
    }
    const start = index - 1;
    let end = index;
    while (end < stops.length && stops[end].offset == null) end += 1;
    const span = end - start;
    for (let cursor = index; cursor < end; cursor += 1) {
      stops[cursor].offset = stops[start].offset
        + (stops[end].offset - stops[start].offset) * (cursor - start) / span;
    }
    index = end;
  }
  return { angle: ((angle % 360) + 360) % 360, stops };
}

function gradientValueForElement(element, view) {
  const computed = String(view.getComputedStyle(element).backgroundImage || '');
  if (computed.includes('gradient(')) return computed;
  const inline = element.getAttribute?.('style') || '';
  const start = inline.search(/(?:linear|radial)-gradient\(/i);
  if (start < 0) return '';
  let depth = 0;
  for (let index = start; index < inline.length; index += 1) {
    if (inline[index] === '(') depth += 1;
    else if (inline[index] === ')' && --depth === 0) return inline.slice(start, index + 1);
  }
  return inline.slice(start);
}

function gradientColorAt(stops, offset) {
  const rightIndex = stops.findIndex((stop) => stop.offset >= offset);
  if (rightIndex <= 0) return stops[0].color;
  if (rightIndex < 0) return stops.at(-1).color;
  const left = stops[rightIndex - 1];
  const right = stops[rightIndex];
  const range = right.offset - left.offset;
  return interpolateColor(left.color, right.color, range > 0 ? (offset - left.offset) / range : 0);
}

function gradientNodes(doc, bodyRect, slideNumber, paintOrderBySource) {
  const nodes = [];
  for (const element of [doc.body, ...doc.querySelectorAll('*')]) {
    if (element.closest?.('table')) continue;
    const backgroundImage = gradientValueForElement(element, doc.defaultView);
    if (!backgroundImage.includes('gradient(')) continue;
    if (!backgroundImage.startsWith('linear-gradient(')) {
      fail(slideNumber, element, 'css_gradient_unsupported',
        'Only linear gradients can be rewritten as editable solid strips.');
    }
    const gradient = parseLinearGradient(backgroundImage);
    if (!gradient) {
      fail(slideNumber, element, 'css_gradient_unsupported', 'Linear gradient stops could not be parsed.');
    }
    const box = elementBox(element, bodyRect);
    const radians = gradient.angle * Math.PI / 180;
    const direction = { x: Math.sin(radians), y: -Math.cos(radians) };
    const gradientLength = Math.abs(box.w * direction.x) + Math.abs(box.h * direction.y);
    const isAxisAligned = gradient.angle % 90 === 0;
    const columns = isAxisAligned && gradient.angle % 180 === 0 ? 1 : GRADIENT_GRID_SIZE;
    const rows = isAxisAligned && gradient.angle % 180 !== 0 ? 1 : GRADIENT_GRID_SIZE;
    const sourceId = sourceIdOf(element);
    const paintOrder = paintOrderBySource.get(sourceId) ?? 0;
    const parsedZIndex = Number.parseInt(doc.defaultView.getComputedStyle(element).zIndex, 10);
    const zIndex = Number.isFinite(parsedZIndex) ? parsedZIndex : 0;
    const total = columns * rows;
    for (let row = 0; row < rows; row += 1) {
      for (let column = 0; column < columns; column += 1) {
        const order = row * columns + column;
        const centerX = box.w * (column + 0.5) / columns;
        const centerY = box.h * (row + 0.5) / rows;
        const projection = (centerX - box.w / 2) * direction.x
          + (centerY - box.h / 2) * direction.y;
        const sample = gradientLength > 0 ? 0.5 + projection / gradientLength : 0.5;
        const strip = {
          x: box.x + box.w * column / columns,
          y: box.y + box.h * row / rows,
          w: box.w / columns,
          h: box.h / rows,
        };
        const color = gradientColorAt(gradient.stops, sample);
        nodes.push({
          type: 'shape',
          shapeType: 'rect',
          rewrite: 'css_gradient',
          sourceId,
          ...strip,
          order,
          paintOrder,
          subOrder: order - total,
          zIndex,
          style: {
            fill: color.hex,
            transparency: (1 - color.alpha) * 100,
            line: null,
          },
        });
      }
    }
  }
  return nodes;
}

function tableTextRuns(cell, view) {
  const runs = [];
  const blockTags = new Set(['P', 'DIV', 'H1', 'H2', 'H3', 'H4', 'H5', 'H6', 'LI']);
  const textOptions = (element) => {
    const computed = view.getComputedStyle(element || cell);
    const cellComputed = view.getComputedStyle(cell);
    const fontWeightValue = computed.fontWeight || cellComputed.fontWeight;
    const fontWeight = Number.parseInt(fontWeightValue, 10);
    return {
      fontFace: String(computed.fontFamily || cellComputed.fontFamily || '')
        .split(',')[0].replace(/['"]/g, '').trim() || 'Arial',
      fontSize: (Number.parseFloat(computed.fontSize || cellComputed.fontSize) || 16) * 0.75,
      color: colorToHex(computed.color || cellComputed.color, '000000'),
      bold: fontWeightValue === 'bold' || fontWeight >= 600,
      italic: computed.fontStyle === 'italic',
      underline: String(computed.textDecoration || '').includes('underline'),
    };
  };
  const appendLineBreak = (owner) => {
    if (!runs.length || runs.at(-1).text.endsWith('\n')) return;
    runs.push({ text: '\n', options: textOptions(owner) });
  };
  const significantSibling = (node, direction) => {
    let sibling = node[direction];
    while (sibling) {
      if (sibling.nodeType === view.Node.ELEMENT_NODE) return sibling;
      if (sibling.nodeType === view.Node.TEXT_NODE && String(sibling.textContent || '').trim()) {
        return sibling;
      }
      sibling = sibling[direction];
    }
    return null;
  };
  const isBlockNode = (node) => (
    node?.nodeType === view.Node.ELEMENT_NODE && blockTags.has(node.tagName)
  );
  const visit = (node, owner) => {
    if (node.nodeType === view.Node.TEXT_NODE) {
      const raw = String(node.textContent || '');
      let text = raw.replace(/\s+/g, ' ');
      if (!text.trim()) {
        const previous = significantSibling(node, 'previousSibling');
        const next = significantSibling(node, 'nextSibling');
        if (previous && next && !isBlockNode(previous) && !isBlockNode(next)) {
          runs.push({ text: ' ', options: textOptions(owner) });
        }
        return;
      }
      if (runs.at(-1)?.text.endsWith('\n')) text = text.replace(/^ /, '');
      if (isBlockNode(significantSibling(node, 'nextSibling'))) text = text.replace(/ $/, '');
      if (text) runs.push({ text, options: textOptions(owner) });
      return;
    }
    if (node.nodeType !== view.Node.ELEMENT_NODE) return;
    if (node.tagName === 'BR') {
      appendLineBreak(owner);
      return;
    }
    if (blockTags.has(node.tagName)) appendLineBreak(node);
    [...node.childNodes].forEach((child) => visit(child, node));
    if (blockTags.has(node.tagName)) appendLineBreak(node);
  };
  [...cell.childNodes].forEach((child) => visit(child, cell));
  if (!runs.length) return '';
  runs[0].text = runs[0].text.replace(/^\s+/, '');
  runs.at(-1).text = runs.at(-1).text.replace(/\s+$/, '');
  return runs.filter((run) => run.text);
}

function authoredStyleValue(cssText, property) {
  const escaped = property.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return String(cssText || '').match(
    new RegExp(`(?:^|;)\\s*${escaped}\\s*:\\s*([^;]+)`, 'i'),
  )?.[1]?.trim() || '';
}

function cssLengthToPoints(value) {
  const match = String(value || '').trim().match(/^(-?(?:\d*\.)?\d+)(pt|px)$/i);
  if (!match) return 0;
  const amount = Number(match[1]);
  return match[2].toLowerCase() === 'pt' ? amount : amount * 0.75;
}

function cssLengthToInches(value) {
  const match = String(value || '').trim().match(/^(-?(?:\d*\.)?\d+)(pt|px)$/i);
  if (!match) return 0;
  const amount = Number(match[1]);
  return amount / (match[2].toLowerCase() === 'pt' ? 72 : PX_PER_IN);
}

function authoredBoxValues(value) {
  const values = String(value || '').trim().split(/\s+/).filter(Boolean);
  if (!values.length || values.length > 4) return null;
  if (values.length === 1) return [values[0], values[0], values[0], values[0]];
  if (values.length === 2) return [values[0], values[1], values[0], values[1]];
  if (values.length === 3) return [values[0], values[1], values[2], values[1]];
  return values;
}

function tableBorderSide(computed, side, authoredStyle) {
  const authoredBorder = authoredStyleValue(authoredStyle, `border-${side.toLowerCase()}`)
    || authoredStyleValue(authoredStyle, 'border');
  const authoredWidth = authoredBorder.match(/-?(?:\d*\.)?\d+(?:pt|px)/i)?.[0];
  const authoredColor = authoredBorder.match(/#[\da-f]{3,8}|rgba?\([^)]+\)|[a-z]+$/i)?.[0];
  const width = authoredWidth
    ? cssLengthToPoints(authoredWidth)
    : Number.parseFloat(computed[`border${side}Width`]) * 0.75 || 0;
  const style = authoredBorder
    ? (/\bnone\b/i.test(authoredBorder) ? 'none' : 'solid')
    : computed[`border${side}Style`];
  return {
    color: colorToHex(authoredColor || computed[`border${side}Color`], '000000'),
    width: style === 'none' ? 0 : width,
  };
}

function tableCellStyle(cell, doc) {
  const view = doc.defaultView;
  const computed = view.getComputedStyle(cell);
  const textOwner = cell.querySelector(
    'p,h1,h2,h3,h4,h5,h6,span,strong,b,em,i,u,small,label,a,code,mark,sub,sup',
  ) || cell;
  const textComputed = view.getComputedStyle(textOwner);
  const fontWeightValue = textComputed.fontWeight || computed.fontWeight;
  const fontWeight = Number.parseInt(fontWeightValue, 10);
  const align = String(textComputed.textAlign || computed.textAlign || 'left');
  const verticalAlign = String(computed.verticalAlign || '').toLowerCase();
  const authoredStyle = cell.dataset.pptxAuthoredStyle || cell.getAttribute('style') || '';
  const authoredPadding = authoredBoxValues(authoredStyleValue(authoredStyle, 'padding'));
  const shorthandColor = String(cell.style.background || '').trim().match(
    /#[\da-f]{3,8}|rgba?\([^)]+\)|^[a-z]+/i,
  )?.[0];
  const authoredBackground = cell.style.backgroundColor
    || shorthandColor
    || authoredStyleValue(authoredStyle, 'background-color')
    || authoredStyleValue(authoredStyle, 'background').match(
      /#[\da-f]{3,8}|rgba?\([^)]+\)|^[a-z]+/i,
    )?.[0];
  const backgroundColor = !isTransparent(computed.backgroundColor)
    ? computed.backgroundColor
    : authoredBackground;
  return {
    fill: isTransparent(backgroundColor)
      ? null
      : colorToHex(backgroundColor, null),
    border: {
      top: tableBorderSide(computed, 'Top', authoredStyle),
      right: tableBorderSide(computed, 'Right', authoredStyle),
      bottom: tableBorderSide(computed, 'Bottom', authoredStyle),
      left: tableBorderSide(computed, 'Left', authoredStyle),
    },
    align: align === 'center' ? 'center' : align === 'right' || align === 'end' ? 'right' : 'left',
    valign: verticalAlign === 'middle'
      ? 'mid'
      : verticalAlign === 'bottom' ? 'bottom' : 'top',
    fontFamily: String(textComputed.fontFamily || '').split(',')[0].replace(/['"]/g, '').trim()
      || 'Arial',
    fontSize: (Number.parseFloat(textComputed.fontSize) || 16) * 0.75,
    fontColor: colorToHex(textComputed.color, '000000'),
    bold: fontWeightValue === 'bold' || fontWeight >= 600,
    padding: authoredPadding
      ? authoredPadding.map(cssLengthToInches)
      : [
        Number.parseFloat(computed.paddingTop) || 0,
        Number.parseFloat(computed.paddingRight) || 0,
        Number.parseFloat(computed.paddingBottom) || 0,
        Number.parseFloat(computed.paddingLeft) || 0,
      ].map((value) => value / PX_PER_IN),
  };
}

function tableZIndex(table, doc) {
  let current = table;
  while (current && current !== doc.body) {
    const parsed = Number.parseInt(doc.defaultView.getComputedStyle(current).zIndex, 10);
    if (Number.isFinite(parsed)) return parsed;
    current = current.parentElement;
  }
  return 0;
}

function effectiveTableRowspan(cell) {
  const raw = cell.getAttribute('rowspan');
  if (raw != null && String(raw).trim() === '0') {
    const row = cell.parentElement;
    const rowGroup = row?.parentElement;
    const groupRows = rowGroup && ['THEAD', 'TBODY', 'TFOOT'].includes(rowGroup.tagName)
      ? [...rowGroup.rows]
      : [];
    const groupIndex = groupRows.indexOf(row);
    return groupIndex >= 0 ? groupRows.length - groupIndex : 1;
  }
  return cell.rowSpan || 1;
}

function tableNodes(doc, bodyRect, slideNumber, paintOrderBySource) {
  const nodes = [];
  for (const table of doc.querySelectorAll('table')) {
    if (table.closest('td,th')) {
      fail(slideNumber, table, 'table_nested_unsupported',
        'Nested tables cannot be represented as one editable PowerPoint table.');
    }
    const rect = table.getBoundingClientRect();
    const rows = [...table.rows];
    if (!(rect.width > 0) || !(rect.height > 0) || !rows.length) {
      fail(slideNumber, table, 'table_geometry_unmeasurable',
        'Editable table requires measurable table and row geometry.');
    }
    const rowRects = rows.map((row) => row.getBoundingClientRect());
    if (rowRects.some((rowRect) => !(rowRect.height > 0))) {
      fail(slideNumber, table, 'table_geometry_unmeasurable',
        'Editable table rows require measurable positive heights.');
    }
    const cellsByRow = rows.map((row) => [...row.cells]);
    const cells = cellsByRow.flat();
    if (!cells.length) {
      fail(slideNumber, table, 'table_grid_non_rectangular',
        'Editable table must contain at least one cell.');
    }
    let maximumHorizontalBorder = 0;
    for (const cell of cells) {
      const computed = doc.defaultView.getComputedStyle(cell);
      maximumHorizontalBorder = Math.max(
        maximumHorizontalBorder,
        parseFloat(computed.borderLeftWidth) || 0,
        parseFloat(computed.borderRightWidth) || 0,
      );
      if (computed.backgroundImage && computed.backgroundImage !== 'none') {
        fail(slideNumber, cell, 'table_cell_background_unsupported',
          'Table cell background images and gradients cannot be represented natively.');
      }
      if (cell.querySelector('table,svg,img,canvas,video,audio')) {
        fail(slideNumber, cell, 'table_cell_content_unsupported',
          'Table cells may only contain editable text content.');
      }
    }
    const tolerance = Math.max(0.75, maximumHorizontalBorder / 2 + 0.5);

    const rawBoundaries = [
      rect.left,
      rect.right,
      ...cells.flatMap((cell) => {
        const cellRect = cell.getBoundingClientRect();
        return [cellRect.left, cellRect.right];
      }),
    ].sort((left, right) => left - right);
    const boundaries = [];
    rawBoundaries.forEach((value) => {
      const previous = boundaries.at(-1);
      if (previous == null || Math.abs(value - previous) > tolerance) boundaries.push(value);
      else boundaries[boundaries.length - 1] = (previous + value) / 2;
    });
    if (boundaries.length < 2
      || Math.abs(boundaries[0] - rect.left) > tolerance
      || Math.abs(boundaries.at(-1) - rect.right) > tolerance) {
      fail(slideNumber, table, 'table_grid_non_rectangular',
        'Rendered table cell boundaries do not cover the complete table width.');
    }
    boundaries[0] = rect.left;
    boundaries[boundaries.length - 1] = rect.right;
    const columnCount = boundaries.length - 1;
    const grid = rows.map(() => Array(columnCount).fill(null));
    const nearestBoundary = (value) => (
      boundaries.findIndex((boundary) => Math.abs(boundary - value) <= tolerance)
    );
    const normalizedRows = rows.map((row, rowIndex) => {
      const normalizedCells = cellsByRow[rowIndex].map((cell) => {
        const cellRect = cell.getBoundingClientRect();
        const startColumn = nearestBoundary(cellRect.left);
        const endColumn = nearestBoundary(cellRect.right);
        const colspan = cell.colSpan || 1;
        const rowspan = effectiveTableRowspan(cell);
        if (startColumn < 0 || endColumn <= startColumn
          || endColumn - startColumn !== colspan
          || rowIndex + rowspan > rows.length) {
          fail(slideNumber, table, 'table_grid_non_rectangular',
            'Rendered cell boundaries do not match the declared colspan or rowspan.');
        }
        const expectedTop = rowRects[rowIndex].top;
        const expectedBottom = rowRects[rowIndex + rowspan - 1].bottom;
        if (Math.abs(cellRect.top - expectedTop) > tolerance
          || Math.abs(cellRect.bottom - expectedBottom) > tolerance) {
          fail(slideNumber, table, 'table_grid_non_rectangular',
            'Rendered row boundaries do not match the declared rowspan.');
        }
        for (let gridRow = rowIndex; gridRow < rowIndex + rowspan; gridRow += 1) {
          for (let column = startColumn; column < endColumn; column += 1) {
            if (grid[gridRow][column]) {
              fail(slideNumber, table, 'table_grid_non_rectangular',
                'Rendered table spans overlap and cannot form a rectangular grid.');
            }
            grid[gridRow][column] = cell;
          }
        }
        return {
          text: tableTextRuns(cell, doc.defaultView),
          style: tableCellStyle(cell, doc),
          ...(colspan > 1 ? { colspan } : {}),
          ...(rowspan > 1 ? { rowspan } : {}),
        };
      });
      return {
        height: rowRects[rowIndex].height / PX_PER_IN,
        cells: normalizedCells,
      };
    });
    if (grid.some((row) => row.some((cell) => !cell))) {
      fail(slideNumber, table, 'table_grid_non_rectangular',
        'Rendered table spans leave uncovered cells and cannot form a rectangular grid.');
    }
    const sourceId = sourceIdOf(table);
    const normalizedTableHeight = rowRects.reduce((sum, rowRect) => (
      sum + rowRect.height
    ), 0) / PX_PER_IN;
    nodes.push({
      type: 'table',
      sourceId,
      x: Math.max(0, rect.left - bodyRect.left) / PX_PER_IN,
      y: Math.max(0, rowRects[0].top - bodyRect.top) / PX_PER_IN,
      w: rect.width / PX_PER_IN,
      h: normalizedTableHeight,
      columnWidths: boundaries.slice(1).map((boundary, index) => (
        (boundary - boundaries[index]) / PX_PER_IN
      )),
      rows: normalizedRows,
      zIndex: tableZIndex(table, doc),
      paintOrder: paintOrderBySource.get(sourceId) ?? 0,
      subOrder: 0,
    });
  }
  return nodes;
}

function pathTokens(data) {
  return String(data || '').match(/[a-zA-Z]|[-+]?(?:\d*\.\d+|\d+\.?)(?:e[-+]?\d+)?/gi) || [];
}

function pointLineDistance(point, start, end) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSquared = dx * dx + dy * dy;
  if (!lengthSquared) return Math.hypot(point.x - start.x, point.y - start.y);
  const area = Math.abs(dy * point.x - dx * point.y + end.x * start.y - end.y * start.x);
  return area / Math.sqrt(lengthSquared);
}

function midpoint(left, right) {
  return { x: (left.x + right.x) / 2, y: (left.y + right.y) / 2 };
}

function sampleQuadratic(start, control, end, tolerance = 0.5) {
  const output = [];
  const visit = (a, b, c, depth) => {
    if (depth >= Math.log2(CURVE_MAX_SEGMENTS) || pointLineDistance(b, a, c) <= tolerance) {
      output.push(c);
      return;
    }
    const ab = midpoint(a, b);
    const bc = midpoint(b, c);
    const center = midpoint(ab, bc);
    visit(a, ab, center, depth + 1);
    visit(center, bc, c, depth + 1);
  };
  visit(start, control, end, 0);
  return output;
}

function sampleCubic(start, first, second, end, tolerance = 0.5) {
  const output = [];
  const visit = (a, b, c, d, depth) => {
    const flatness = Math.max(pointLineDistance(b, a, d), pointLineDistance(c, a, d));
    if (depth >= Math.log2(CURVE_MAX_SEGMENTS) || flatness <= tolerance) {
      output.push(d);
      return;
    }
    const ab = midpoint(a, b);
    const bc = midpoint(b, c);
    const cd = midpoint(c, d);
    const abc = midpoint(ab, bc);
    const bcd = midpoint(bc, cd);
    const center = midpoint(abc, bcd);
    visit(a, ab, abc, center, depth + 1);
    visit(center, bcd, cd, d, depth + 1);
  };
  visit(start, first, second, end, 0);
  return output;
}

function parseEditablePath(data, slideNumber, element) {
  const tokens = pathTokens(data);
  const points = [];
  let index = 0;
  let command = '';
  let current = { x: 0, y: 0 };
  let subpathStart = null;
  let lastCubicControl = null;
  let lastQuadraticControl = null;
  const number = () => Number(tokens[index++]);
  const point = (relative) => {
    const result = { x: number(), y: number() };
    return relative ? { x: current.x + result.x, y: current.y + result.y } : result;
  };
  const append = (next) => {
    points.push([current, next]);
    current = next;
  };

  while (index < tokens.length) {
    if (/^[a-z]$/i.test(tokens[index])) command = tokens[index++];
    if (!command) fail(slideNumber, element, 'svg_path_invalid', 'SVG path begins without a command.');
    const relative = command === command.toLowerCase();
    switch (command.toUpperCase()) {
      case 'M': {
        current = point(relative);
        subpathStart = current;
        command = relative ? 'l' : 'L';
        break;
      }
      case 'L':
        append(point(relative));
        break;
      case 'H': {
        const x = number();
        append({ x: relative ? current.x + x : x, y: current.y });
        break;
      }
      case 'V': {
        const y = number();
        append({ x: current.x, y: relative ? current.y + y : y });
        break;
      }
      case 'C': {
        const start = current;
        const first = point(relative);
        const second = point(relative);
        const end = point(relative);
        sampleCubic(start, first, second, end).forEach(append);
        lastCubicControl = second;
        lastQuadraticControl = null;
        break;
      }
      case 'S': {
        const start = current;
        const first = lastCubicControl
          ? { x: 2 * current.x - lastCubicControl.x, y: 2 * current.y - lastCubicControl.y }
          : current;
        const second = point(relative);
        const end = point(relative);
        sampleCubic(start, first, second, end).forEach(append);
        lastCubicControl = second;
        lastQuadraticControl = null;
        break;
      }
      case 'Q': {
        const start = current;
        const control = point(relative);
        const end = point(relative);
        sampleQuadratic(start, control, end).forEach(append);
        lastQuadraticControl = control;
        lastCubicControl = null;
        break;
      }
      case 'T': {
        const start = current;
        const control = lastQuadraticControl
          ? { x: 2 * current.x - lastQuadraticControl.x, y: 2 * current.y - lastQuadraticControl.y }
          : current;
        const end = point(relative);
        sampleQuadratic(start, control, end).forEach(append);
        lastQuadraticControl = control;
        lastCubicControl = null;
        break;
      }
      case 'Z':
        if (subpathStart && (current.x !== subpathStart.x || current.y !== subpathStart.y)) append(subpathStart);
        command = '';
        break;
      default:
        fail(slideNumber, element, 'svg_path_command_unsupported',
          `SVG path command "${command}" cannot be represented as editable lines.`);
    }
    if (!['C', 'S'].includes(command.toUpperCase())) lastCubicControl = null;
    if (!['Q', 'T'].includes(command.toUpperCase())) lastQuadraticControl = null;
  }
  return points;
}

function dashStyle(value) {
  const values = String(value || '').match(/[-+]?(?:\d*\.)?\d+/g)?.map(Number)
    .filter((item) => Number.isFinite(item) && item >= 0) || [];
  if (!values.length) return null;
  if (values.length >= 4 && values[2] <= values[0] * 0.5) return 'dashDot';
  if (values.length >= 2 && values[0] <= values[1] * 0.5) return 'dot';
  return 'dash';
}

function hrNodes(doc, bodyRect, paintOrderBySource) {
  return [...doc.querySelectorAll('hr')].map((element) => {
    const rect = element.getBoundingClientRect();
    const computed = doc.defaultView.getComputedStyle(element);
    const borderCandidates = [
      ['Top', computed.borderTopStyle, computed.borderTopWidth, computed.borderTopColor],
      ['Bottom', computed.borderBottomStyle, computed.borderBottomWidth, computed.borderBottomColor],
      ['Left', computed.borderLeftStyle, computed.borderLeftWidth, computed.borderLeftColor],
      ['Right', computed.borderRightStyle, computed.borderRightWidth, computed.borderRightColor],
    ];
    const border = borderCandidates.find(([, style, width]) => (
      style && style !== 'none' && (Number.parseFloat(width) || 0) > 0
    ));
    const [, borderStyle = 'solid', borderWidth = '1px', borderColor = computed.color] = border || [];
    const width = Number.parseFloat(borderWidth) || 1;
    const color = colorToHex(borderColor || computed.color);
    const sourceId = sourceIdOf(element);
    const dash = borderStyle === 'dotted' ? 'dot' : borderStyle === 'dashed' ? 'dash' : null;
    return {
      type: 'line',
      sourceId,
      x1: Math.max(0, rect.left - bodyRect.left) / PX_PER_IN,
      y1: Math.max(0, rect.top - bodyRect.top) / PX_PER_IN,
      x2: Math.max(0, rect.right - bodyRect.left) / PX_PER_IN,
      y2: Math.max(0, rect.top - bodyRect.top) / PX_PER_IN,
      paintOrder: paintOrderBySource.get(sourceId) ?? 0,
      subOrder: 0,
      zIndex: Number.parseInt(computed.zIndex, 10) || 0,
      style: {
        color,
        width: width * 0.75,
        ...(dash ? { dash, dashType: dash } : {}),
      },
    };
  });
}

function pathNodes(doc, bodyRect, slideNumber, paintOrderBySource) {
  const nodes = [];
  for (const path of doc.querySelectorAll('svg path')) {
    let transformedAncestor = path;
    while (transformedAncestor && transformedAncestor.localName !== 'svg') {
      const computedTransform = doc.defaultView.getComputedStyle(transformedAncestor).transform;
      if (transformedAncestor.hasAttribute('transform')
        || (computedTransform && computedTransform !== 'none')) {
        fail(slideNumber, path, 'svg_path_transform_unsupported',
          'Transformed SVG paths cannot yet be safely decomposed into editable slide lines.');
      }
      transformedAncestor = transformedAncestor.parentElement;
    }
    const segments = parseEditablePath(path.getAttribute('d'), slideNumber, path);
    const computed = doc.defaultView.getComputedStyle(path);
    const fill = path.hasAttribute('fill')
      ? path.getAttribute('fill')
      : computed.fill || 'black';
    if (segments.length && !isTransparent(fill)) {
      fail(slideNumber, path, 'svg_path_fill_unsupported',
        'A filled SVG path cannot be safely decomposed into editable line objects.');
    }
    const svg = path.closest('svg');
    const svgRect = svg.getBoundingClientRect();
    const viewportMapper = createSvgViewportMapper(svg, svgRect);
    const map = (point) => {
      const mapped = viewportMapper.map(point);
      return {
        x: Math.max(0, mapped.x - bodyRect.left) / PX_PER_IN,
        y: Math.max(0, mapped.y - bodyRect.top) / PX_PER_IN,
      };
    };
    const stroke = path.getAttribute('stroke') || computed.stroke || fill;
    const dashArray = path.getAttribute('stroke-dasharray') || computed.strokeDasharray;
    const dash = dashStyle(dashArray);
    const style = {
      color: colorToHex(stroke),
      width: (Number.parseFloat(path.getAttribute('stroke-width') || computed.strokeWidth) || 1) * 0.75,
      ...(dash ? { dash, dashType: dash } : {}),
    };
    const sourceId = sourceIdOf(path);
    segments.forEach(([start, end], subOrder) => {
      const first = map(start);
      const second = map(end);
      if (first.x === second.x && first.y === second.y) return;
      nodes.push({
        type: 'line',
        rewrite: 'svg_path_rewrite',
        sourceId,
        x1: first.x, y1: first.y, x2: second.x, y2: second.y,
        paintOrder: paintOrderBySource.get(sourceId) ?? 0,
        subOrder,
        style: { ...style },
      });
    });
  }
  return nodes;
}

function mapExtractedElement(element, doc) {
  const position = element.position || element.bbox;
  const shapeRadius = element.type === 'shape'
    ? element.shape?.rectRadius
    : element.shape?.radius;
  const source = element.sourceId
    ? [...doc.querySelectorAll('[data-pptx-source-id]')]
      .find((candidate) => candidate.dataset.pptxSourceId === element.sourceId)
    : null;
  if (element.type === 'line') {
    const dashArray = source?.getAttribute('stroke-dasharray')
      || (source ? doc.defaultView.getComputedStyle(source).strokeDasharray : '');
    const dash = element.style?.dash || dashStyle(dashArray);
    return {
      type: 'line',
      sourceId: element.sourceId,
      x1: element.x1, y1: element.y1, x2: element.x2, y2: element.y2,
      paintOrder: element.paintOrder,
      subOrder: element.subOrder,
      zIndex: element.zIndex,
      style: {
        color: String(element.color || '000000').toUpperCase(),
        width: element.width || 0.75,
        ...(dash ? { dash, dashType: dash } : {}),
      },
    };
  }
  if (element.type === 'image') {
    return {
      type: 'image',
      intent: 'user-image',
      sourceId: element.sourceId,
      x: position.x, y: position.y, w: position.w, h: position.h,
      src: element.src,
      paintOrder: element.paintOrder,
      subOrder: element.subOrder,
      zIndex: element.zIndex,
    };
  }
  if (element.type === 'shape' || element.type === 'svg-shape') {
    const cssEllipse = element.type === 'shape' && element.shape?.rectRadius === 1;
    const rounded = element.svgType === 'roundRect'
      || (source?.localName === 'rect'
        && (Number(source.getAttribute('rx')) > 0 || Number(source.getAttribute('ry')) > 0));
    return {
      type: 'shape',
      shapeType: cssEllipse ? 'ellipse' : (rounded ? 'roundRect' : ({
        rect: 'rect', circle: 'ellipse', ellipse: 'ellipse',
        triangle: 'triangle', diamond: 'diamond',
      }[element.svgType] || (element.shape?.rectRadius ? 'roundRect' : 'rect'))),
      sourceId: element.sourceId,
      x: position.x, y: position.y, w: position.w, h: position.h,
      paintOrder: element.paintOrder,
      subOrder: element.subOrder,
      zIndex: element.zIndex,
      ...(element.rewrite ? { rewrite: element.rewrite } : {}),
      style: {
        fill: String(element.shape?.fill || 'FFFFFF').toUpperCase(),
        line: element.shape?.line,
        transparency: element.shape?.transparency,
        rotate: element.shape?.rotate ?? 0,
        ...(element.shape?.shadow ? { shadow: element.shape.shadow } : {}),
        ...(Number.isFinite(shapeRadius) && shapeRadius > 0 && !cssEllipse
          ? { radius: shapeRadius }
          : {}),
      },
    };
  }
  const text = element.text ?? element.items;
  if (text != null) {
    const anchor = source?.getAttribute('text-anchor');
    return {
      type: 'text',
      sourceId: element.sourceId,
      x: position.x, y: position.y, w: position.w, h: position.h,
      text,
      paintOrder: element.paintOrder,
      subOrder: element.subOrder,
      zIndex: element.zIndex,
      style: {
        ...(element.style || {}),
        ...(anchor ? { align: { start: 'left', middle: 'center', end: 'right' }[anchor] || 'left' } : {}),
      },
    };
  }
  return null;
}

function isSupportedEditablePolygon(element) {
  return Boolean(classifySvgPresetPolygon(parsePolygonPoints(element)));
}

function auditExtractedDiagnostics(extracted, rewrittenSourceIds, slideNumber) {
  for (const diagnostic of extracted.diagnostics || []) {
    if (diagnostic.severity !== 'blocking' || rewrittenSourceIds.has(diagnostic.sourceId)) continue;
    failSource(
      slideNumber,
      diagnostic.sourceId,
      diagnostic.code || 'editable_input_unsupported',
      diagnostic.message || 'The source cannot be represented as editable PowerPoint objects.',
    );
  }
}

export function normalizeDocumentToEditableScene(doc, { slideNumber, width, height } = {}) {
  if (!doc?.body || !doc.defaultView) {
    fail(slideNumber, null, 'unreadable_document', 'The slide document is not readable.');
  }
  editableErrorPreflight(doc, slideNumber);
  validateSvgPresetGeometry(doc, slideNumber);
  const bodyRect = doc.body.getBoundingClientRect();
  if (!(bodyRect.width > 0) || !(bodyRect.height > 0)) {
    fail(slideNumber, doc.body, 'unmeasurable_canvas',
      'The editable slide canvas must have measurable positive dimensions.');
  }
  if (Math.abs(bodyRect.width - width * PX_PER_IN) > 2
    || Math.abs(bodyRect.height - height * PX_PER_IN) > 2) {
    fail(slideNumber, doc.body, 'canvas_size',
      'The source canvas dimensions do not match the editable slide dimensions.');
  }
  const bodyDimensions = measureBodyDimensions(doc);
  if (bodyDimensions.errors?.length) {
    fail(slideNumber, doc.body, 'canvas_overflow', bodyDimensions.errors.join(' '));
  }
  const extracted = extractSlideDataFromDocument(doc);
  for (const element of doc.querySelectorAll('[data-pptx-source-id]')) {
    const computed = doc.defaultView.getComputedStyle(element);
    const textShadow = [computed.textShadow, element.style?.textShadow,
      authoredStyleValue(element.dataset?.pptxAuthoredStyle, 'text-shadow')]
      .find((value) => value && value !== 'none');
    if (textShadow) {
      fail(slideNumber, element, 'text_shadow_unsupported',
        'CSS text-shadow cannot be represented as editable PowerPoint text.');
    }
    const boxShadow = [computed.boxShadow, element.style?.boxShadow,
      authoredStyleValue(element.dataset?.pptxAuthoredStyle, 'box-shadow')]
      .find((value) => value && value !== 'none');
    if (!boxShadow) continue;
    const sourceId = sourceIdOf(element);
    const nativeShadow = extracted.elements.some((candidate) => (
      candidate.sourceId === sourceId && candidate.shape?.shadow
    ));
    const ringRewrite = extracted.elements.some((candidate) => (
      candidate.sourceId === sourceId && candidate.rewrite === 'css_box_shadow_ring'
    ));
    if (!nativeShadow && !ringRewrite) {
      fail(slideNumber, element, 'box_shadow_unsupported',
        'CSS box-shadow cannot be represented as one native editable PowerPoint outer shadow.');
    }
  }
  const paintOrderBySource = new Map();
  [doc.body, ...doc.body.querySelectorAll('[data-pptx-source-id]')].forEach((element, index) => {
    paintOrderBySource.set(sourceIdOf(element), index);
  });
  extracted.elements.forEach((element) => {
    if (element.sourceId && Number.isFinite(element.paintOrder)) {
      paintOrderBySource.set(element.sourceId, element.paintOrder);
    }
  });
  const tableOwnedSourceIds = new Set(
    [...doc.querySelectorAll('table, table *')]
      .map((element) => element.dataset?.pptxSourceId || element.id || null)
      .filter(Boolean),
  );
  const hrSourceIds = new Set([...doc.querySelectorAll('hr')].map(sourceIdOf));
  const pathSourceIds = new Set([...doc.querySelectorAll('svg path')].map(sourceIdOf));
  const gradientSourceIds = new Set(
    [doc.body, ...doc.querySelectorAll('*')]
      .filter((element) => (
        !element.closest?.('table')
        && gradientValueForElement(element, doc.defaultView).includes('gradient(')
      ))
      .map(sourceIdOf),
  );
  const rewrittenSourceIds = new Set([
    ...pathSourceIds,
    ...gradientSourceIds,
    ...tableOwnedSourceIds,
    ...hrSourceIds,
  ]);
  doc.querySelectorAll('svg').forEach((svg) => {
    const paths = [...svg.querySelectorAll('path')];
    if (paths.length && paths.every((path) => pathSourceIds.has(sourceIdOf(path)))) {
      rewrittenSourceIds.add(sourceIdOf(svg));
    }
  });
  doc.querySelectorAll('svg polygon').forEach((polygon) => {
    const sourceId = sourceIdOf(polygon);
    const nativePolygon = extracted.elements.some((element) => (
      element.sourceId === sourceId
      && element.type === 'svg-shape'
      && ['triangle', 'diamond'].includes(element.svgType)
    ));
    if (isSupportedEditablePolygon(polygon) && nativePolygon) rewrittenSourceIds.add(sourceId);
  });
  const normalizedPaths = pathNodes(doc, bodyRect, slideNumber, paintOrderBySource);
  const normalizedTables = tableNodes(doc, bodyRect, slideNumber, paintOrderBySource);
  auditExtractedDiagnostics(extracted, rewrittenSourceIds, slideNumber);
  if (extracted.background?.type === 'image') {
    fail(slideNumber, doc.body, 'background_image_unsupported',
      'Slide background images must be authored as intentional img elements.');
  }
  const backgroundNodes = extracted.background?.type === 'color'
    ? [{
      type: 'shape',
      shapeType: 'rect',
      sourceId: `${sourceIdOf(doc.body)}-background`,
      x: 0,
      y: 0,
      w: width,
      h: height,
      paintOrder: -1,
      subOrder: 0,
      zIndex: -1,
      style: {
        fill: String(extracted.background.value || 'FFFFFF').toUpperCase(),
        line: null,
      },
    }]
    : [];
  const nativeNodes = extracted.elements
    .filter((element) => (
      !tableOwnedSourceIds.has(element.sourceId)
      && !pathSourceIds.has(element.sourceId)
      && !(gradientSourceIds.has(element.sourceId)
        && ['shape', 'svg-shape'].includes(element.type))
    ))
    .map((element) => mapExtractedElement(element, doc))
    .filter(Boolean);
  const scene = {
    slideNumber,
    width,
    height,
    nodes: [
      ...backgroundNodes,
      ...nativeNodes,
      ...gradientNodes(doc, bodyRect, slideNumber, paintOrderBySource),
      ...normalizedPaths,
      ...normalizedTables,
      ...hrNodes(doc, bodyRect, paintOrderBySource),
    ].sort((left, right) => (
      (left.zIndex ?? 0) - (right.zIndex ?? 0)
      || (left.paintOrder ?? 0) - (right.paintOrder ?? 0)
      || (left.subOrder ?? 0) - (right.subOrder ?? 0)
    )),
  };
  return validateEditableSlideScene(scene);
}
