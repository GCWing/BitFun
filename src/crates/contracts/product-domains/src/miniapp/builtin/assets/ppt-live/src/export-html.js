import { escapeHtml } from './state.js';
import { getLocale } from './i18n.js';
import { normalizeSlideDocument, slideHtml } from './render.js';
import { sanitizeSlideMarkup } from './sanitize-slide-markup.js';

export function buildHtmlDeck(state) {
  if ((state.slides || []).some((slide) => slide.html)) {
    return buildSourceHtmlDeck(state);
  }
  const slides = state.slides
    .map((slide) => `<section class="deck-slide">${slideHtml(slide)}</section>`)
    .join('\n');
  return `<!DOCTYPE html>
<html lang="${getLocale()}">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>${escapeHtml(state.title || 'PPT Live')}</title>
<style>
${deckCss()}
</style>
</head>
<body>
<main class="deck">${slides}</main>
</body>
</html>`;
}

function buildSourceHtmlDeck(state) {
  const slides = (state.slides || [])
    .map((slide, index) => `<section class="deck-slide" data-index="${index + 1}">
  <iframe class="source-frame" sandbox="allow-same-origin" srcdoc="${escapeHtml(sanitizeSlideMarkup(normalizeSlideDocument(slide.html || slideHtml(slide))))}"></iframe>
</section>`)
    .join('\n');
  return `<!DOCTYPE html>
<html lang="${getLocale()}">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>${escapeHtml(state.title || 'PPT Live')}</title>
<style>
html,body{margin:0;background:#111;color:#fff;font-family:system-ui,-apple-system,"Segoe UI",sans-serif}
.deck{min-height:100vh;display:grid;gap:32px;padding:32px}
.deck-slide{height:calc(100vh - 64px);display:grid;place-items:center;break-after:page}
.source-frame{width:min(100%,177.777vh);aspect-ratio:16/9;height:auto;border:0;background:#fff;box-shadow:0 24px 70px rgba(0,0,0,.34)}
@media print{.deck{display:block;padding:0}.deck-slide{height:100vh;break-after:page}.source-frame{width:100vw;height:56.25vw;box-shadow:none}}
</style>
</head>
<body>
<main class="deck">${slides}</main>
</body>
</html>`;
}

export function downloadHtmlDeck(state) {
  const blob = new Blob([buildHtmlDeck(state)], { type: 'text/html;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  const filename = `${fileSafe(state.title || 'ppt-live')}.html`;
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
  return filename;
}

export function downloadBase64File(base64, filename, mimeType) {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  const blob = new Blob([bytes], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

export function fileSafe(value) {
  return String(value || 'ppt-live').replace(/[\\/:*?"<>|]+/g, '-').slice(0, 96);
}

function deckCss() {
  return `
body{margin:0;background:#111827;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
.deck{display:grid;gap:24px;padding:24px}.deck-slide{display:grid;place-items:center;min-height:100vh}
.slide{position:relative;width:min(100%,1280px);aspect-ratio:16/9;color:var(--slide-ink);background:var(--slide-bg);overflow:hidden;box-shadow:0 24px 70px rgba(0,0,0,.34)}
.free-slide{background:radial-gradient(circle at 84% 18%,color-mix(in srgb,var(--slide-primary) 14%,transparent),transparent 28%),linear-gradient(135deg,color-mix(in srgb,var(--slide-accent) 8%,transparent),transparent 42%),var(--slide-bg)}
.slide-element{position:absolute;padding:12px;overflow:hidden;white-space:pre-wrap;line-height:1.14;box-sizing:border-box}
.element-text{display:flex;align-items:center}.element-list{line-height:1.36}.element-list ul{margin:0;padding-left:1.1em;display:grid;gap:.5em}
.element-metric{display:flex;flex-direction:column;justify-content:center;gap:6px;box-shadow:0 18px 38px rgba(17,24,39,.12)}
.element-metric strong{font-size:inherit;line-height:.95}.element-metric span{color:var(--slide-muted);font-size:14px;line-height:1.25}
.element-media{display:grid;place-items:center;border:1px dashed color-mix(in srgb,var(--slide-primary) 40%,transparent)}
.element-chart{display:flex;flex-direction:column;gap:10px}.element-chart b{font-size:16px}.chart-bars{display:flex;align-items:end;gap:8px;flex:1;min-height:0}.chart-bars span{display:flex;flex:1;height:100%;align-items:end;gap:4px;flex-direction:column}.chart-bars i{display:block;width:100%;border-radius:5px 5px 0 0;background:var(--slide-primary)}.chart-bars em{font-size:10px;color:var(--slide-muted);font-style:normal}
`;
}
