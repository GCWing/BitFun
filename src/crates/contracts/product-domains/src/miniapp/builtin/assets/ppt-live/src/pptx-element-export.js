import { createPptxDeck, buildSpeakerNotes } from './pptx-html-build.js';

const SLIDE_W = 13.333;
const SLIDE_H = 7.5;

function drawSlideBackdrop(pptx, slide, theme, index) {
  slide.addShape(pptx.ShapeType.rect, {
    x: 0,
    y: 0,
    w: SLIDE_W,
    h: SLIDE_H,
    fill: { color: hex(theme.background) },
    line: { color: hex(theme.background), transparency: 100 },
  });
  slide.addShape(pptx.ShapeType.ellipse, {
    x: 10.3,
    y: -0.45,
    w: 3.8,
    h: 3.8,
    fill: { color: hex(index % 2 ? theme.accent : theme.primary), transparency: 84 },
    line: { color: hex(theme.background), transparency: 100 },
  });
  slide.addShape(pptx.ShapeType.rect, {
    x: 0,
    y: 0,
    w: 0.12,
    h: SLIDE_H,
    fill: { color: hex(theme.primary), transparency: 0 },
    line: { color: hex(theme.primary), transparency: 100 },
  });
}

function drawSlideMethodology(pptx, slide, sourceSlide, theme) {
  if (sourceSlide.kicker) {
    slide.addShape(pptx.ShapeType.roundRect, {
      x: 0.96,
      y: 0.48,
      w: 0.22,
      h: 0.07,
      fill: { color: hex(theme.primary) },
      line: { color: hex(theme.primary), transparency: 100 },
    });
    slide.addText(String(sourceSlide.kicker).toUpperCase(), {
      x: 1.24,
      y: 0.36,
      w: 2.4,
      h: 0.28,
      margin: 0,
      fontFace: 'Aptos',
      fontSize: 7,
      bold: true,
      color: hex(theme.primary),
      fit: 'shrink',
    });
  }
  if (sourceSlide.proofObject) {
    slide.addText(String(sourceSlide.proofObject), {
      x: 9.26,
      y: 0.34,
      w: 2.95,
      h: 0.32,
      margin: 0.04,
      fontFace: 'Aptos',
      fontSize: 7,
      bold: true,
      color: hex(theme.muted),
      align: 'right',
      fit: 'shrink',
      fill: { color: hex(theme.panel), transparency: 8 },
      line: { color: hex(theme.primary), transparency: 78 },
    });
  }
  if (sourceSlide.sourceNote) {
    slide.addText(String(sourceSlide.sourceNote), {
      x: 0.96,
      y: 7.05,
      w: 10.7,
      h: 0.2,
      margin: 0,
      fontFace: 'Aptos',
      fontSize: 6,
      color: hex(theme.muted),
      fit: 'shrink',
    });
  }
}

function drawElement(pptx, slide, element, theme) {
  const box = toInches(element);
  const style = element.style || {};
  const common = {
    x: box.x,
    y: box.y,
    w: box.w,
    h: box.h,
    margin: 0.08,
    fit: 'shrink',
    color: hex(resolveColor(style.color, theme)),
    fontFace: 'Aptos',
    fontSize: pxToPt(style.fontSize || 22),
    bold: Number(style.fontWeight || 500) >= 700,
    align: style.align || 'left',
    valign: 'mid',
    breakLine: false,
  };
  if (element.type === 'shape') {
    slide.addShape(pptx.ShapeType.roundRect, {
      ...box,
      rectRadius: 0.08,
      fill: { color: hex(resolveColor(style.background, theme)), transparency: transparency(style.opacity) },
      line: { color: hex(resolveColor(style.background, theme)), transparency: 100 },
    });
    if (element.text) slide.addText(element.text, common);
    return;
  }
  if (element.type === 'list') {
    const runs = (element.items || []).map((item) => ({ text: item, options: { bullet: { type: 'bullet' }, breakLine: true } }));
    slide.addText(runs.length ? runs : [{ text: '' }], {
      ...common,
      valign: 'top',
      paraSpaceAfterPt: 6,
      fit: 'shrink',
    });
    return;
  }
  if (element.type === 'metric') {
    drawPanel(pptx, slide, box, style, theme);
    slide.addText(String(element.text || ''), {
      ...common,
      y: box.y + 0.08,
      h: box.h * 0.48,
      color: hex(resolveColor(style.color || 'primary', theme)),
      fontSize: pxToPt(style.fontSize || 42),
      bold: true,
    });
    slide.addText(String(element.label || ''), {
      ...common,
      y: box.y + box.h * 0.56,
      h: box.h * 0.34,
      color: hex(theme.muted),
      fontSize: 10,
      bold: false,
      valign: 'top',
    });
    return;
  }
  if (element.type === 'chart') {
    drawPanel(pptx, slide, box, style, theme);
    slide.addText(String(element.text || ''), {
      ...common,
      y: box.y + 0.1,
      h: 0.32,
      fontSize: 11,
      bold: true,
    });
    drawBars(pptx, slide, element, box, theme);
    return;
  }
  if (element.type === 'media') {
    slide.addShape(pptx.ShapeType.roundRect, {
      ...box,
      fill: { color: hex(resolveColor(style.background || 'soft', theme)), transparency: 10 },
      line: { color: hex(theme.primary), transparency: 55, dash: 'dash' },
    });
    slide.addText(String(element.text || 'Image placeholder'), {
      ...common,
      align: 'center',
      color: hex(theme.muted),
      fontSize: 12,
    });
    return;
  }
  drawTextBackground(pptx, slide, box, style, theme);
  slide.addText(String(element.text || ''), common);
}

function drawPanel(pptx, slide, box, style, theme) {
  slide.addShape(pptx.ShapeType.roundRect, {
    ...box,
    fill: { color: hex(resolveColor(style.background || 'panel', theme)), transparency: transparency(style.opacity) },
    line: { color: hex(theme.primary), transparency: 82 },
    shadow: { type: 'outer', color: '111827', opacity: 0.12, blur: 1, angle: 45, distance: 1 },
  });
}

function drawTextBackground(pptx, slide, box, style, theme) {
  const bg = style.background || 'transparent';
  if (bg === 'transparent') return;
  slide.addShape(pptx.ShapeType.roundRect, {
    ...box,
    fill: { color: hex(resolveColor(bg, theme)), transparency: transparency(style.opacity) },
    line: { color: hex(resolveColor(bg, theme)), transparency: 100 },
  });
}

function drawBars(pptx, slide, element, box, theme) {
  const data = Array.isArray(element.data) && element.data.length
    ? element.data
    : [{ label: 'A', value: 40 }, { label: 'B', value: 70 }];
  const max = Math.max(1, ...data.map((point) => Number(point.value) || 0));
  const gap = 0.1;
  const chartX = box.x + 0.18;
  const chartY = box.y + 0.68;
  const chartW = box.w - 0.36;
  const chartH = box.h - 0.95;
  const barW = Math.max(0.08, (chartW - gap * (data.length - 1)) / data.length);
  data.forEach((point, index) => {
    const value = Number(point.value) || 0;
    const h = Math.max(0.15, (value / max) * chartH);
    const x = chartX + index * (barW + gap);
    const y = chartY + chartH - h;
    slide.addShape(pptx.ShapeType.rect, {
      x,
      y,
      w: barW,
      h,
      fill: { color: hex(index % 2 ? theme.accent : theme.primary) },
      line: { color: hex(index % 2 ? theme.accent : theme.primary), transparency: 100 },
    });
    slide.addText(String(point.label || ''), {
      x: x - 0.03,
      y: chartY + chartH + 0.04,
      w: barW + 0.06,
      h: 0.2,
      fontSize: 7,
      color: hex(theme.muted),
      align: 'center',
      margin: 0,
      fit: 'shrink',
    });
  });
}

function normalizeTheme(theme = {}) {
  return {
    background: theme.background || '#fbfcff',
    ink: theme.ink || '#111827',
    muted: theme.muted || '#5b6575',
    primary: theme.primary || '#0f766e',
    accent: theme.accent || '#f97316',
    panel: theme.panel || '#ffffff',
  };
}

function toInches(element) {
  return {
    x: pct(element.x) * SLIDE_W,
    y: pct(element.y) * SLIDE_H,
    w: pct(element.w) * SLIDE_W,
    h: pct(element.h) * SLIDE_H,
  };
}

function pct(value) {
  return Math.max(0, Math.min(100, Number(value) || 0)) / 100;
}

function pxToPt(value) {
  return Math.max(6, Math.min(66, Math.round((Number(value) || 22) * 0.58)));
}

function resolveColor(value, theme) {
  if (!value || value === 'transparent') return theme.background;
  if (value === 'ink') return theme.ink;
  if (value === 'muted') return theme.muted;
  if (value === 'primary') return theme.primary;
  if (value === 'accent') return theme.accent;
  if (value === 'panel') return theme.panel;
  if (value === 'soft') return theme.primary;
  if (value === 'background') return theme.background;
  return value;
}

function hex(value) {
  const raw = String(value || '#111827').trim();
  if (/^#[0-9a-f]{6}$/i.test(raw)) return raw.slice(1).toUpperCase();
  if (/^[0-9a-f]{6}$/i.test(raw)) return raw.toUpperCase();
  if (/^#[0-9a-f]{3}$/i.test(raw)) {
    return raw.slice(1).split('').map((part) => part + part).join('').toUpperCase();
  }
  return '111827';
}

function transparency(opacity) {
  return Math.round((1 - Math.max(0, Math.min(1, Number(opacity ?? 1)))) * 100);
}

export async function exportElementDeckToPptx(deck) {
  const slides = Array.isArray(deck.slides) && deck.slides.length > 0 ? deck.slides : [];
  const pptx = createPptxDeck(deck);
  slides.forEach((sourceSlide, index) => {
    const slide = pptx.addSlide();
    const theme = normalizeTheme(sourceSlide.theme);
    slide.background = { color: hex(theme.background) };
    drawSlideBackdrop(pptx, slide, theme, index);
    drawSlideMethodology(pptx, slide, sourceSlide, theme);
    (sourceSlide.elements || []).forEach((element) => drawElement(pptx, slide, element, theme));
    const notes = buildSpeakerNotes(sourceSlide);
    if (notes && typeof slide.addNotes === 'function') slide.addNotes(notes);
  });
  return pptx;
}
