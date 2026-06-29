export const MERMAID_THEME_MODES = ['dark', 'light'] as const;

export type MermaidThemeMode = typeof MERMAID_THEME_MODES[number];

const DARK_NODE_FILL = '#1c1e23';
const DARK_NODE_FILL_HOVER = '#262830';
const DARK_NODE_TEXT = '#e0e2e8';
const DARK_NODE_STROKE = '#5a5e6a';
const DARK_NODE_BORDER = '#4a4e58';
const DARK_NODE_STROKE_SUBTLE = '#3a3e48';
const DARK_NODE_STROKE_HOVER = '#6a6e7a';
const DARK_CLUSTER_TEXT = '#9a9ea8';
const DARK_INFO = '#78a8d8';
const DARK_ERROR = '#e87878';

const LIGHT_NODE_FILL = '#e8eaef';
const LIGHT_NODE_FILL_HOVER = '#e0e2e8';
const LIGHT_NODE_STROKE = '#9ca3af';
const LIGHT_NODE_STROKE_MUTED = '#d1d5db';
const LIGHT_NODE_STROKE_HOVER = '#6b7280';
const LIGHT_EDGE_LABEL_BG = '#f3f4f6';
const LIGHT_SECTION_ALT_FILL = '#e5e7eb';
const LIGHT_TITLE_TEXT = '#111827';
const LIGHT_HIGHLIGHT_STROKE = '#334155';
const LIGHT_ERROR = '#dc2626';
const LIGHT_WARNING = '#f59e0b';
const LIGHT_CLUSTER_FILL = 'rgba(229, 231, 235, 0.7)';

export const MERMAID_THEME_FALLBACKS = {
  dark: {
    nodeFill: DARK_NODE_FILL,
    nodeFillRuntime: 'rgba(28, 30, 35, 0.9)',
    nodeFillHover: DARK_NODE_FILL_HOVER,
    nodeFillHoverRuntime: 'rgba(38, 40, 48, 0.95)',
    nodeText: DARK_NODE_TEXT,
    nodeTextStrong: '#f0f2f8',
    nodeStroke: DARK_NODE_STROKE,
    nodeBorder: DARK_NODE_BORDER,
    nodeStrokeMuted: DARK_NODE_BORDER,
    nodeStrokeSubtle: DARK_NODE_STROKE_SUBTLE,
    nodeStrokeHover: DARK_NODE_STROKE_HOVER,
    nodeStrokeHoverStrong: '#8a8e9a',
    edgeStroke: DARK_NODE_STROKE,
    edgeLabelBorderHover: DARK_NODE_STROKE,
    clusterFill: '#16181c',
    clusterFillRuntime: 'rgba(24, 26, 30, 0.6)',
    clusterFillHover: 'rgba(34, 36, 42, 0.7)',
    clusterText: DARK_CLUSTER_TEXT,
    textMuted: DARK_NODE_STROKE_HOVER,
    arrow: '#7a7e8a',
    edgeLabelBg: '#1a1c20',
    edgeLabelBgStrong: DARK_NODE_FILL_HOVER,
    edgeLabelBgRuntime: 'rgba(26, 28, 32, 0.95)',
    edgeLabelBgHover: 'rgba(36, 38, 45, 0.98)',
    noteFill: '#222428',
    noteText: DARK_CLUSTER_TEXT,
    noteStroke: DARK_NODE_BORDER,
    activationFill: '#2a2c32',
    activationStroke: DARK_NODE_STROKE,
    sectionFill: DARK_NODE_FILL,
    sectionAltFill: DARK_NODE_FILL_HOVER,
    gridStroke: DARK_NODE_STROKE_SUBTLE,
    doneFill: 'rgba(109, 212, 160, 0.15)',
    doneStroke: '#6dd4a0',
    activeFill: 'rgba(120, 168, 216, 0.15)',
    activeStroke: DARK_INFO,
    critFill: 'rgba(232, 120, 120, 0.15)',
    critStroke: DARK_ERROR,
    warning: '#e8b060',
    info: DARK_INFO,
    taskClickableInfo: DARK_INFO,
    pie5: '#a090d8',
    pie6: '#d890b8',
    pie7: '#68c8d8',
    pie8: '#a8d868',
    pieTitleText: DARK_NODE_TEXT,
    pieLegendText: DARK_CLUSTER_TEXT,
    pieStroke: DARK_NODE_FILL,
    errorFill: 'rgba(232, 120, 120, 0.12)',
    error: DARK_ERROR,
    highlightStroke: '#a8acb8',
    highlightGlow: 'drop-shadow(0 0 6px rgba(168, 172, 184, 0.4))',
    highlightGlowStrong: 'drop-shadow(0 0 10px rgba(168, 172, 184, 0.5))',
  },
  light: {
    nodeFill: LIGHT_NODE_FILL,
    nodeFillRuntime: LIGHT_NODE_FILL,
    nodeFillHover: LIGHT_NODE_FILL_HOVER,
    nodeFillHoverRuntime: LIGHT_NODE_FILL_HOVER,
    nodeText: '#1f2937',
    nodeTextStrong: LIGHT_TITLE_TEXT,
    nodeStroke: LIGHT_NODE_STROKE,
    nodeBorder: LIGHT_NODE_STROKE,
    nodeStrokeMuted: LIGHT_NODE_STROKE_MUTED,
    nodeStrokeSubtle: LIGHT_NODE_STROKE_MUTED,
    nodeStrokeHover: LIGHT_NODE_STROKE_HOVER,
    nodeStrokeHoverStrong: LIGHT_NODE_STROKE_HOVER,
    edgeStroke: LIGHT_NODE_STROKE,
    edgeLabelBorderHover: LIGHT_NODE_STROKE,
    clusterFill: LIGHT_CLUSTER_FILL,
    clusterFillRuntime: LIGHT_CLUSTER_FILL,
    clusterFillHover: 'rgba(209, 213, 219, 0.8)',
    clusterText: '#4b5563',
    textMuted: LIGHT_NODE_STROKE_HOVER,
    arrow: LIGHT_NODE_STROKE_HOVER,
    edgeLabelBg: LIGHT_EDGE_LABEL_BG,
    edgeLabelBgStrong: LIGHT_EDGE_LABEL_BG,
    edgeLabelBgRuntime: LIGHT_EDGE_LABEL_BG,
    edgeLabelBgHover: LIGHT_SECTION_ALT_FILL,
    noteFill: '#fef3c7',
    noteText: '#92400e',
    noteStroke: '#f59e0b',
    activationFill: 'rgba(147, 197, 253, 0.25)',
    activationStroke: '#93c5fd',
    sectionFill: LIGHT_EDGE_LABEL_BG,
    sectionAltFill: LIGHT_SECTION_ALT_FILL,
    gridStroke: 'rgba(156, 163, 175, 0.3)',
    doneFill: 'rgba(34, 197, 94, 0.2)',
    doneStroke: '#16a34a',
    activeFill: 'rgba(15, 23, 42, 0.08)',
    activeStroke: LIGHT_HIGHLIGHT_STROKE,
    critFill: 'rgba(239, 68, 68, 0.2)',
    critStroke: LIGHT_ERROR,
    warning: LIGHT_WARNING,
    info: '#64748b',
    taskClickableInfo: '#475569',
    pie5: '#8b5cf6',
    pie6: '#ec4899',
    pie7: '#06b6d4',
    pie8: '#84cc16',
    pieTitleText: LIGHT_TITLE_TEXT,
    pieLegendText: '#374151',
    pieStroke: '#f9fafb',
    errorFill: 'rgba(239, 68, 68, 0.15)',
    error: LIGHT_ERROR,
    highlightStroke: LIGHT_HIGHLIGHT_STROKE,
    highlightGlow: 'drop-shadow(0 0 6px rgba(15, 23, 42, 0.18))',
    highlightGlowStrong: 'drop-shadow(0 0 10px rgba(15, 23, 42, 0.22))',
  },
} as const;

export type MermaidThemeFallbackKey = keyof typeof MERMAID_THEME_FALLBACKS.dark;

export function getMermaidThemeFallback(
  mode: MermaidThemeMode,
  key: MermaidThemeFallbackKey,
): string {
  return MERMAID_THEME_FALLBACKS[mode][key];
}

export function getMermaidThemeFallbackPair(key: MermaidThemeFallbackKey): {
  dark: string;
  light: string;
} {
  return {
    dark: MERMAID_THEME_FALLBACKS.dark[key],
    light: MERMAID_THEME_FALLBACKS.light[key],
  };
}
