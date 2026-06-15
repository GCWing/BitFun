const SVG_ATTRS = 'xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false"';

/** Recognizable file-type marks for export format cards (inline SVG, no external assets). */
const ICONS = {
  pptx: `<svg ${SVG_ATTRS}><rect x="4" y="2" width="16" height="20" rx="2.2" fill="#fff" fill-opacity=".18"/><path d="M7 6.5h6.4c2.45 0 4.1 1.35 4.1 3.65S15.85 13.8 13.4 13.8H10v3.7H7V6.5z" fill="#fff"/><path d="M10 9.2h2.9c1.05 0 1.65.55 1.65 1.45s-.6 1.45-1.65 1.45H10V9.2z" fill="#fff" fill-opacity=".92"/></svg>`,
  pdf: `<svg ${SVG_ATTRS}><path d="M8 3.5h6.8L18 6.7V19.5A1.5 1.5 0 0 1 16.5 21h-9A1.5 1.5 0 0 1 6 19.5V5A1.5 1.5 0 0 1 8 3.5z" fill="#fff" fill-opacity=".22"/><path d="M14.2 3.8V7h3.8" stroke="#fff" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/><text x="12" y="15.8" text-anchor="middle" fill="#fff" font-family="system-ui,-apple-system,Arial,sans-serif" font-size="6.8" font-weight="800" letter-spacing=".35">PDF</text></svg>`,
  html: `<svg ${SVG_ATTRS}><path d="M8.25 7.25 5.5 12l2.75 4.75" stroke="#fff" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"/><path d="M15.75 7.25 18.5 12l-2.75 4.75" stroke="#fff" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"/><path d="M13.35 7.6 10.65 16.4" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg>`,
  png: `<svg ${SVG_ATTRS}><rect x="4" y="5" width="16" height="14" rx="2" fill="#fff" fill-opacity=".18"/><path d="M7 16.5l3.2-3.8 2.4 2.2L15.5 11 19 16.5H7z" fill="#fff"/><circle cx="9" cy="9.5" r="1.35" fill="#fff"/></svg>`,
};

export function exportFormatIcon(formatId) {
  return ICONS[formatId] || ICONS.html;
}

export function exportFormatTone(formatId) {
  const tones = {
    pptx: '#d44726',
    pdf: '#e01e3c',
    html: '#0d9488',
    png: '#2563eb',
  };
  return tones[formatId] || '#475569';
}
