import {
  EditableExportError,
  validateEditableSlideScene,
} from './editable-slide-scene.js';

const SLIDE_W = 13.333;
const SLIDE_H = 7.5;

function pct(value) {
  return Math.max(0, Math.min(100, Number(value) || 0)) / 100;
}

function pxToPt(value) {
  return Math.max(6, Math.min(66, Math.round((Number(value) || 22) * 0.58)));
}

function hex(value, fallback = '111827') {
  const raw = String(value || '').trim();
  if (/^#[0-9a-f]{6}$/i.test(raw)) return raw.slice(1).toUpperCase();
  if (/^[0-9a-f]{6}$/i.test(raw)) return raw.toUpperCase();
  if (/^#[0-9a-f]{3}$/i.test(raw)) {
    return raw.slice(1).split('').map((part) => part + part).join('').toUpperCase();
  }
  return fallback;
}

function themeFor(sourceSlide = {}) {
  const theme = sourceSlide.theme || {};
  return {
    background: hex(theme.background, 'FBFCFF'),
    ink: hex(theme.ink, '111827'),
    muted: hex(theme.muted, '5B6575'),
    primary: hex(theme.primary, '0F766E'),
    accent: hex(theme.accent, 'F97316'),
    panel: hex(theme.panel, 'FFFFFF'),
  };
}

function resolveColor(value, theme) {
  const key = String(value || '').replace(/^#/, '').toLowerCase();
  if (theme[key]) return theme[key];
  return hex(value, theme.ink);
}

function elementBox(element) {
  return {
    x: pct(element.x) * SLIDE_W,
    y: pct(element.y) * SLIDE_H,
    w: Math.max(0.01, pct(element.w) * SLIDE_W),
    h: Math.max(0.01, pct(element.h) * SLIDE_H),
  };
}

function nodeId(element, index, suffix = '') {
  const base = String(element.id || element.sourceId || `element-${index + 1}`);
  return suffix ? `${base}-${suffix}` : base;
}

function shapeNode(sourceId, box, shapeType, fill, paintOrder, extraStyle = {}) {
  return {
    type: 'shape',
    shapeType,
    sourceId,
    ...box,
    zIndex: paintOrder,
    paintOrder,
    subOrder: 0,
    style: {
      fill,
      line: null,
      ...extraStyle,
    },
  };
}

function textNode(sourceId, box, text, style, theme, paintOrder, subOrder = 1) {
  return {
    type: 'text',
    sourceId,
    ...box,
    zIndex: paintOrder,
    text,
    paintOrder,
    subOrder,
    style: {
      margin: 0.08,
      fontFace: 'Microsoft YaHei',
      fontSize: pxToPt(style.fontSize || 22),
      bold: Number(style.fontWeight || 500) >= 700,
      color: resolveColor(style.color || 'ink', theme),
      align: style.align || 'left',
      valign: 'mid',
    },
  };
}

function elementModelError(slideNumber, sourceId, code, message) {
  throw new EditableExportError({
    slideNumber,
    sourceId,
    code,
    message,
  });
}

function hasText(value) {
  return typeof value === 'string' && Boolean(value.trim());
}

function normalizeElement(element, index, theme, paintOrder, slideNumber) {
  const box = elementBox(element);
  const style = element.style || {};
  const sourceId = nodeId(element, index);
  const zIndex = Number.isFinite(element.zIndex) ? element.zIndex : paintOrder;
  if ((element.type === 'image' || element.type === 'media') && (element.src || element.path || element.data)) {
    const nodes = [{
      type: 'image',
      intent: 'user-image',
      sourceId,
      ...box,
      zIndex,
      paintOrder,
      subOrder: 0,
      ...(element.src ? { src: element.src } : {}),
      ...(element.path ? { path: element.path } : {}),
      ...(element.data ? { data: element.data } : {}),
    }];
    validateEditableSlideScene({
      slideNumber,
      width: SLIDE_W,
      height: SLIDE_H,
      nodes,
    });
    return nodes;
  }
  if (element.type === 'image' || element.type === 'media' || element.type === 'video') {
    elementModelError(
      slideNumber,
      sourceId,
      'element_model_media_unsupported',
      `Element "${sourceId}" requires an inline base64 user image; media placeholders and video are not editable.`,
    );
  }
  if (element.type === 'shape') {
    const fill = resolveColor(style.background || 'panel', theme);
    const nodes = [
      shapeNode(sourceId, box, 'roundRect', fill, paintOrder, { radius: 0.08 }),
      ...(element.text
        ? [textNode(`${sourceId}-text`, box, String(element.text), style, theme, paintOrder)]
        : []),
    ];
    nodes.forEach((node) => { node.zIndex = zIndex; });
    return nodes;
  }
  if (element.type === 'list') {
    if (!Array.isArray(element.items)
      || !element.items.length
      || element.items.some((item) => !hasText(item))) {
      elementModelError(
        slideNumber,
        sourceId,
        'element_model_payload_invalid',
        `List element "${sourceId}" requires at least one non-empty item.`,
      );
    }
    const runs = (element.items || []).map((item) => ({
      text: String(item),
      options: { bullet: { type: 'bullet' }, breakLine: true },
    }));
    const nodes = [textNode(sourceId, box, runs, style, theme, paintOrder, 0)];
    nodes[0].zIndex = zIndex;
    return nodes;
  }
  if (element.type === 'metric') {
    if (!hasText(element.text) || !hasText(element.label)) {
      elementModelError(
        slideNumber,
        sourceId,
        'element_model_payload_invalid',
        `Metric element "${sourceId}" requires non-empty text and label values.`,
      );
    }
    const nodes = [
      shapeNode(`${sourceId}-panel`, box, 'roundRect', resolveColor(style.background || 'panel', theme), paintOrder, {
        radius: 0.08,
      }),
      textNode(sourceId, {
        ...box,
        y: box.y + 0.08,
        h: box.h * 0.48,
      }, String(element.text || ''), {
        ...style,
        color: style.color || 'primary',
        fontSize: style.fontSize || 42,
        fontWeight: 700,
      }, theme, paintOrder),
      textNode(`${sourceId}-label`, {
        ...box,
        y: box.y + box.h * 0.56,
        h: box.h * 0.34,
      }, String(element.label || ''), {
        color: 'muted',
        fontSize: 17,
      }, theme, paintOrder, 2),
    ];
    nodes.forEach((node) => { node.zIndex = zIndex; });
    return nodes;
  }
  if (element.type === 'chart') {
    if (!Array.isArray(element.data)
      || !element.data.length
      || element.data.some((point) => !hasText(point?.label)
        || !Number.isFinite(Number(point?.value)))) {
      elementModelError(
        slideNumber,
        sourceId,
        'element_model_payload_invalid',
        `Chart element "${sourceId}" requires labeled finite data points.`,
      );
    }
    const data = element.data;
    const max = Math.max(1, ...data.map((point) => Number(point.value) || 0));
    const chartX = box.x + 0.18;
    const chartY = box.y + 0.68;
    const chartW = box.w - 0.36;
    const chartH = box.h - 0.95;
    const gap = 0.1;
    const barW = Math.max(0.08, (chartW - gap * (data.length - 1)) / data.length);
    const nodes = [
      shapeNode(`${sourceId}-panel`, box, 'roundRect', resolveColor(style.background || 'panel', theme), paintOrder, {
        radius: 0.08,
      }),
      textNode(`${sourceId}-title`, { ...box, y: box.y + 0.1, h: 0.32 },
        String(element.text || ''), { ...style, fontSize: 19, fontWeight: 700 }, theme, paintOrder, 1),
    ];
    data.forEach((point, dataIndex) => {
      const height = Math.max(0.15, ((Number(point.value) || 0) / max) * chartH);
      const x = chartX + dataIndex * (barW + gap);
      nodes.push(shapeNode(`${sourceId}-bar-${dataIndex + 1}`, {
        x,
        y: chartY + chartH - height,
        w: barW,
        h: height,
      }, 'rect', dataIndex % 2 ? theme.accent : theme.primary, paintOrder, {
        line: null,
      }));
      nodes.at(-1).subOrder = 2 + dataIndex * 2;
      nodes.push(textNode(`${sourceId}-label-${dataIndex + 1}`, {
        x: x - 0.03,
        y: chartY + chartH + 0.04,
        w: barW + 0.06,
        h: 0.2,
      }, String(point.label || ''), {
        color: 'muted',
        fontSize: 12,
        align: 'center',
      }, theme, paintOrder, 3 + dataIndex * 2));
    });
    nodes.forEach((node) => { node.zIndex = zIndex; });
    return nodes;
  }
  if (element.type === 'text') {
    if (!hasText(element.text)) {
      elementModelError(
        slideNumber,
        sourceId,
        'element_model_payload_invalid',
        `Text element "${sourceId}" requires non-empty text.`,
      );
    }
    const nodes = [];
    if (style.background && style.background !== 'transparent') {
      nodes.push(shapeNode(`${sourceId}-background`, box, 'roundRect',
        resolveColor(style.background, theme), paintOrder, { radius: 0.08 }));
    }
    nodes.push(textNode(sourceId, box, element.text, style, theme, paintOrder, nodes.length));
    nodes.forEach((node) => { node.zIndex = zIndex; });
    return nodes;
  }
  elementModelError(
    slideNumber,
    sourceId,
    'element_model_type_unsupported',
    `Element type "${String(element.type)}" is not supported by editable PPTX export.`,
  );
}

export function normalizeElementSlideToEditableScene(sourceSlide = {}, { slideNumber = 1 } = {}) {
  const theme = themeFor(sourceSlide);
  const nodes = [
    shapeNode('legacy-slide-background', {
      x: 0, y: 0, w: SLIDE_W, h: SLIDE_H,
    }, 'rect', theme.background, 0),
    shapeNode('legacy-slide-accent', {
      x: 0, y: 0, w: 0.12, h: SLIDE_H,
    }, 'rect', theme.primary, 1),
  ];
  let paintOrder = 2;
  if (sourceSlide.kicker) {
    nodes.push(shapeNode('legacy-kicker-mark', {
      x: 0.96, y: 0.48, w: 0.22, h: 0.07,
    }, 'roundRect', theme.primary, paintOrder, { radius: 0.035 }));
    nodes.push(textNode('legacy-kicker', {
      x: 1.24, y: 0.36, w: 2.4, h: 0.28,
    }, String(sourceSlide.kicker).toUpperCase(), {
      color: 'primary', fontSize: 12, fontWeight: 700,
    }, theme, paintOrder, 1));
    paintOrder += 1;
  }
  (sourceSlide.elements || []).forEach((element, index) => {
    nodes.push(...normalizeElement(element, index, theme, paintOrder, slideNumber));
    paintOrder += 1;
  });
  if (sourceSlide.sourceNote) {
    nodes.push(textNode('legacy-source-note', {
      x: 0.96, y: 7.05, w: 10.7, h: 0.2,
    }, String(sourceSlide.sourceNote), {
      color: 'muted', fontSize: 10,
    }, theme, paintOrder));
  }
  return validateEditableSlideScene({
    slideNumber,
    width: SLIDE_W,
    height: SLIDE_H,
    nodes,
  });
}
