const escapeHtml = (value) => String(value ?? '')
  .replaceAll('&', '&amp;')
  .replaceAll('<', '&lt;')
  .replaceAll('>', '&gt;')
  .replaceAll('"', '&quot;')
  .replaceAll("'", '&#039;');

const DEFAULT_THEME = {
  background: '#ffffff',
  ink: '#111111',
  muted: '#666666',
  primary: '#0f766e',
  accent: '#f97316',
  panel: '#ffffff',
};

function colorMix(hex, alpha) {
  const raw = String(hex || DEFAULT_THEME.primary).replace('#', '');
  const normalized = raw.length === 3 ? raw.split('').map((value) => value + value).join('') : raw;
  const parsed = Number.parseInt(normalized, 16);
  if (!Number.isFinite(parsed)) return `rgba(15, 118, 110, ${alpha})`;
  return `rgba(${(parsed >> 16) & 255}, ${(parsed >> 8) & 255}, ${parsed & 255}, ${alpha})`;
}

export function resolveElementColor(value, theme = {}) {
  const palette = { ...DEFAULT_THEME, ...theme };
  if (!value || value === 'transparent') return 'transparent';
  if (value === 'soft') return colorMix(palette.primary, 0.1);
  if (Object.hasOwn(palette, value)) return palette[value];
  return String(value);
}

function editorFontSize(value) {
  const size = Math.max(8, Number(value) || 24);
  const cqw = Math.round((size / 10.2) * 1000) / 1000;
  return `clamp(8px, ${cqw}cqw, ${size}px)`;
}

function elementStyle(element, theme, mode) {
  const style = element?.style || {};
  const fontSize = Math.max(8, Number(style.fontSize) || 24);
  return [
    `left:${Number(element?.x) || 0}%`,
    `top:${Number(element?.y) || 0}%`,
    `width:${Number(element?.w) || 0}%`,
    `height:${Number(element?.h) || 0}%`,
    `font-size:${mode === 'editor' ? editorFontSize(fontSize) : `${fontSize}px`}`,
    `font-weight:${Number(style.fontWeight) || 600}`,
    `color:${resolveElementColor(style.color || 'ink', theme)}`,
    `text-align:${style.align || 'left'}`,
    `background:${resolveElementColor(style.background || 'transparent', theme)}`,
    `opacity:${Number.isFinite(Number(style.opacity)) ? Number(style.opacity) : 1}`,
    `border-radius:${Math.max(0, Number(style.borderRadius) || 0)}px`,
  ].join(';');
}

function semanticElementContent(element, mediaPlaceholder) {
  const type = String(element?.type || 'text');
  if (type === 'list') {
    return `<ul>${(element.items || []).map((item) => `<li><p>${escapeHtml(item)}</p></li>`).join('')}</ul>`;
  }
  if (type === 'metric') {
    return `<p class="metric-value">${escapeHtml(element.text || '')}</p><p class="metric-label">${escapeHtml(element.label || '')}</p>`;
  }
  if (type === 'chart') {
    const points = Array.isArray(element.data) ? element.data : [];
    const max = Math.max(1, ...points.map((point) => Math.abs(Number(point?.value) || 0)));
    return `<p class="chart-title">${escapeHtml(element.text || '')}</p><div class="chart-bars">${points.map((point) => {
      const value = Number(point?.value) || 0;
      const height = Math.max(8, Math.abs(value) / max * 100);
      return `<div class="chart-item"><div class="chart-bar" style="height:${height}%"></div><p class="chart-label">${escapeHtml(point?.label || '')}</p><p class="chart-value">${escapeHtml(value)}</p></div>`;
    }).join('')}</div>`;
  }
  if (type === 'media') {
    const source = element.src || element.url || element.dataUrl || element.imageUrl || '';
    const caption = element.text || element.label || mediaPlaceholder;
    return `<figure>${source ? `<img src="${escapeHtml(source)}" alt="${escapeHtml(caption)}">` : ''}<figcaption><p>${escapeHtml(caption)}</p></figcaption></figure>`;
  }
  const text = element.text || '';
  return text ? `<p class="element-text-content">${escapeHtml(text)}</p>` : '';
}

function editorElementContent(element, mediaPlaceholder, editable) {
  const type = String(element?.type || 'text');
  if (type === 'list') {
    return `<ul>${(element.items || []).map((item, index) => editable
      ? `<li data-edit-list="${escapeHtml(element.id)}" data-item-index="${index}" contenteditable="true" spellcheck="false">${escapeHtml(item)}</li>`
      : `<li>${escapeHtml(item)}</li>`).join('')}</ul>`;
  }
  if (type === 'metric') {
    return `<strong>${escapeHtml(element.text)}</strong><span>${escapeHtml(element.label)}</span>`;
  }
  if (type === 'chart') {
    const points = Array.isArray(element.data) ? element.data : [];
    const max = Math.max(1, ...points.map((point) => Number(point?.value) || 0));
    return `<b>${escapeHtml(element.text)}</b><div class="chart-bars">${points.map((point) => `<span><i style="height:${Math.max(8, (Number(point?.value) || 0) / max * 100)}%"></i><em>${escapeHtml(point?.label)}</em></span>`).join('')}</div>`;
  }
  if (type === 'media') return `<span>${escapeHtml(element.text || mediaPlaceholder)}</span>`;
  return editable
    ? `<span class="editable-text" data-edit-text="${escapeHtml(element.id)}" contenteditable="true" spellcheck="false">${escapeHtml(element.text || '')}</span>`
    : escapeHtml(element.text || '');
}

export function elementModelElementHtml(element = {}, theme = {}, {
  mode = 'semantic',
  editable = false,
  selectedId = '',
  mediaPlaceholder = 'Media placeholder',
} = {}) {
  const type = String(element.type || 'text');
  const selected = mode === 'editor' && editable && selectedId === element.id;
  const content = mode === 'editor'
    ? editorElementContent(element, mediaPlaceholder, editable)
    : semanticElementContent(element, mediaPlaceholder);
  return `<div class="slide-element element-${escapeHtml(type)}${selected ? ' is-selected' : ''}" data-element-id="${escapeHtml(element.id || '')}" data-element-type="${escapeHtml(type)}" data-editable="${editable ? 'true' : 'false'}" style="${elementStyle(element, theme, mode)}">${content}${selected ? '<i class="resize-handle"></i>' : ''}</div>`;
}

export function buildElementSlideHtml(slide = {}) {
  const theme = { ...DEFAULT_THEME, ...(slide.theme || {}) };
  const elements = Array.isArray(slide.elements) ? slide.elements : [];
  const fallback = elements.length
    ? ''
    : `<div class="slide-element element-text fallback-title" style="left:8%;top:32%;width:84%;height:36%;font-size:56px;font-weight:700;color:${theme.ink};text-align:left;background:transparent;opacity:1;border-radius:0"><p>${escapeHtml(slide.title || 'Slide')}</p>${slide.subtitle || slide.claim ? `<p class="fallback-subtitle">${escapeHtml(slide.subtitle || slide.claim)}</p>` : ''}</div>`;
  return `<!DOCTYPE html>
<html lang="${escapeHtml(slide.language || 'en')}">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=1280,height=720">
<title>${escapeHtml(slide.title || 'Slide')}</title>
<style>
  *, *::before, *::after { box-sizing: border-box; }
  html, body { margin: 0; padding: 0; width: 1280px; height: 720px; overflow: hidden; }
  body {
    position: relative;
    background: ${theme.background};
    color: ${theme.ink};
    font-family: system-ui, -apple-system, "PingFang SC", "Source Han Sans SC", sans-serif;
  }
  .slide-element { position: absolute; padding: 12px; overflow: hidden; line-height: 1.35; overflow-wrap: anywhere; }
  .slide-element p { margin: 0; }
  .element-list ul { margin: 0; padding-left: 1.1em; }
  .element-list li + li { margin-top: 8px; }
  .element-list li > p { display: inline; }
  .element-metric { display: flex; flex-direction: column; justify-content: center; gap: 4px; }
  .metric-value { font-size: 1em; font-weight: inherit; }
  .metric-label { font-size: 0.42em; color: ${theme.muted}; }
  .element-chart { display: flex; flex-direction: column; gap: 8px; }
  .chart-bars { display: flex; align-items: end; gap: 6px; min-height: 0; height: 100%; }
  .chart-item { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: flex-end; gap: 3px; height: 100%; }
  .chart-bar { width: 100%; min-height: 8px; background: ${theme.primary}; border-radius: 4px 4px 0 0; }
  .chart-label, .chart-value { font-size: 10px; color: ${theme.muted}; }
  .element-media figure { margin: 0; width: 100%; height: 100%; display: flex; flex-direction: column; gap: 6px; }
  .element-media img { display: block; width: 100%; min-height: 0; flex: 1; object-fit: contain; }
  .element-media figcaption { flex: none; }
  .fallback-subtitle { margin-top: 16px !important; font-size: 24px; color: ${theme.muted}; }
</style>
</head>
<body>
  ${elements.map((element) => elementModelElementHtml(element, theme)).join('\n  ')}
  ${fallback}
</body>
</html>`;
}
