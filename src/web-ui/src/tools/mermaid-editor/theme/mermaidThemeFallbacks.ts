export const MERMAID_THEME_MODES = ['dark', 'light'] as const;

export type MermaidThemeMode = typeof MERMAID_THEME_MODES[number];

const DARK_NODE_FILL = '#1c1e23';
const DARK_NODE_FILL_HOVER = '#262830';
const DARK_NODE_TEXT = '#e0e2e8';
const DARK_NODE_STROKE = '#5a5e6a';
const DARK_NODE_BORDER = '#4a4e58';
const DARK_NODE_STROKE_SUBTLE = '#3a3e48';
const DARK_NODE_STROKE_HOVER = '#6a6e7a';
const DARK_CLUSTER_TEXT = '#9ca3af';
const MERMAID_SUCCESS = '#34d399';
const MERMAID_SUCCESS_FILL = 'rgba(52, 211, 153, 0.1)';
const MERMAID_WARNING = '#f59e0b';
const MERMAID_ERROR = '#ef4444';
const MERMAID_ERROR_FILL = 'rgba(239, 68, 68, 0.1)';
const MERMAID_INFO = '#60a5fa';
const MERMAID_INFO_FILL = 'rgba(96, 165, 250, 0.15)';
const MERMAID_PIE_PURPLE = '#8b5cf6';
const MERMAID_PIE_PINK = '#ec4899';
const MERMAID_PIE_CYAN = '#06b6d4';
const MERMAID_PIE_LIME = '#84cc16';

const LIGHT_NODE_FILL = '#e8eaef';
const LIGHT_NODE_FILL_HOVER = '#e0e2e8';
const LIGHT_NODE_STROKE = '#9ca3af';
const LIGHT_NODE_STROKE_MUTED = '#d1d5db';
const LIGHT_NODE_STROKE_HOVER = '#64748b';
const MERMAID_SOFT_LIGHT_SURFACE = '#f3f4f6';
const LIGHT_EDGE_LABEL_BG = MERMAID_SOFT_LIGHT_SURFACE;
const LIGHT_SECTION_ALT_FILL = LIGHT_NODE_FILL_HOVER;
const LIGHT_TITLE_TEXT = '#111827';
const LIGHT_HIGHLIGHT_STROKE = '#334155';
const LIGHT_CLUSTER_FILL = 'rgba(229, 231, 235, 0.7)';

export const MERMAID_THEME_FALLBACKS = {
  dark: {
    nodeFill: DARK_NODE_FILL,
    nodeFillRuntime: 'rgba(28, 30, 35, 0.9)',
    nodeFillHover: DARK_NODE_FILL_HOVER,
    nodeFillHoverRuntime: 'rgba(38, 40, 48, 0.95)',
    nodeText: DARK_NODE_TEXT,
    nodeTextStrong: MERMAID_SOFT_LIGHT_SURFACE,
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
    edgeLabelBg: DARK_NODE_FILL,
    edgeLabelBgStrong: DARK_NODE_FILL_HOVER,
    edgeLabelBgRuntime: 'rgba(26, 28, 32, 0.95)',
    edgeLabelBgHover: DARK_NODE_FILL_HOVER,
    noteFill: DARK_NODE_FILL_HOVER,
    noteText: DARK_CLUSTER_TEXT,
    noteStroke: DARK_NODE_BORDER,
    activationFill: MERMAID_INFO_FILL,
    activationStroke: DARK_NODE_STROKE,
    sectionFill: DARK_NODE_FILL,
    sectionAltFill: DARK_NODE_FILL_HOVER,
    gridStroke: DARK_NODE_STROKE_SUBTLE,
    doneFill: MERMAID_SUCCESS_FILL,
    doneStroke: MERMAID_SUCCESS,
    activeFill: MERMAID_INFO_FILL,
    activeStroke: MERMAID_INFO,
    critFill: MERMAID_ERROR_FILL,
    critStroke: MERMAID_ERROR,
    warning: MERMAID_WARNING,
    info: MERMAID_INFO,
    taskClickableInfo: MERMAID_INFO,
    pie5: MERMAID_PIE_PURPLE,
    pie6: MERMAID_PIE_PINK,
    pie7: MERMAID_PIE_CYAN,
    pie8: MERMAID_PIE_LIME,
    pieTitleText: DARK_NODE_TEXT,
    pieLegendText: DARK_CLUSTER_TEXT,
    pieStroke: DARK_NODE_FILL,
    errorFill: MERMAID_ERROR_FILL,
    error: MERMAID_ERROR,
    highlightStroke: '#a8acb8',
    highlightGlow: 'drop-shadow(0 0 6px rgba(168, 172, 184, 0.4))',
    highlightGlowStrong: 'drop-shadow(0 0 10px rgba(168, 172, 184, 0.5))',
  },
  light: {
    nodeFill: LIGHT_NODE_FILL,
    nodeFillRuntime: LIGHT_NODE_FILL,
    nodeFillHover: LIGHT_NODE_FILL_HOVER,
    nodeFillHoverRuntime: LIGHT_NODE_FILL_HOVER,
    nodeText: '#1e293b',
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
    clusterText: '#475569',
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
    doneFill: MERMAID_SUCCESS_FILL,
    doneStroke: MERMAID_SUCCESS,
    activeFill: 'rgba(15, 23, 42, 0.1)',
    activeStroke: LIGHT_HIGHLIGHT_STROKE,
    critFill: MERMAID_ERROR_FILL,
    critStroke: MERMAID_ERROR,
    warning: MERMAID_WARNING,
    info: '#64748b',
    taskClickableInfo: '#475569',
    pie5: MERMAID_PIE_PURPLE,
    pie6: MERMAID_PIE_PINK,
    pie7: MERMAID_PIE_CYAN,
    pie8: MERMAID_PIE_LIME,
    pieTitleText: LIGHT_TITLE_TEXT,
    pieLegendText: LIGHT_HIGHLIGHT_STROKE,
    pieStroke: MERMAID_SOFT_LIGHT_SURFACE,
    errorFill: MERMAID_ERROR_FILL,
    error: MERMAID_ERROR,
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
