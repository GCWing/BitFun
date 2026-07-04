// Concrete exception accents shared by UI metadata. Keep this palette compact:
// theme-aware surfaces should use theme tokens, and metadata-only accents should
// map back to one of these reviewed semantic colors instead of adding one-off hexes.
const EXCEPTION_ACCENT = {
  primary: '#3b82f6',
  secondary: '#8b5cf6',
  info: '#06b6d4',
  success: '#22c55e',
  warning: '#f59e0b',
  error: '#ef4444',
  neutral: '#64748b',
  teal: '#14b8a6',
  strokeYellow: '#eab308',
  textOnDark: '#e2e8f0',
  staticWhite: '#ffffff',
} as const;

export const UI_EXCEPTION_ACCENTS = {
  contextCompression: EXCEPTION_ACCENT.secondary,
  generativeUi: EXCEPTION_ACCENT.info,
  miniApp: EXCEPTION_ACCENT.secondary,
  mermaidDiagram: EXCEPTION_ACCENT.success,
  agentCapability: {
    docs: EXCEPTION_ACCENT.success,
    testing: EXCEPTION_ACCENT.warning,
    creative: EXCEPTION_ACCENT.secondary,
    ops: EXCEPTION_ACCENT.info,
  },
  insights: {
    positive: EXCEPTION_ACCENT.success,
    time: EXCEPTION_ACCENT.secondary,
    neutral: EXCEPTION_ACCENT.warning,
    issue: EXCEPTION_ACCENT.error,
  },
  progress: {
    compacting: EXCEPTION_ACCENT.teal,
  },
  templateContext: {
    memories: EXCEPTION_ACCENT.secondary,
  },
  reviewTeam: {
    memberDefault: EXCEPTION_ACCENT.neutral,
    businessLogic: EXCEPTION_ACCENT.primary,
    performance: EXCEPTION_ACCENT.warning,
    security: EXCEPTION_ACCENT.error,
    architecture: EXCEPTION_ACCENT.info,
    frontend: EXCEPTION_ACCENT.success,
    judge: EXCEPTION_ACCENT.secondary,
  },
  tealAction: EXCEPTION_ACCENT.teal,
  todo: EXCEPTION_ACCENT.teal,
  textStroke: [
    EXCEPTION_ACCENT.strokeYellow,
    EXCEPTION_ACCENT.error,
    EXCEPTION_ACCENT.primary,
    EXCEPTION_ACCENT.info,
    EXCEPTION_ACCENT.secondary,
  ],
  inspectorOverlay: {
    activeBorder: EXCEPTION_ACCENT.primary,
    activeBackground: 'rgba(59, 130, 246, 0.15)',
    activeBorderSubtle: 'rgba(59, 130, 246, 0.4)',
    selectedBorder: EXCEPTION_ACCENT.success,
    selectedBackground: 'rgba(34, 197, 94, 0.18)',
    browserTooltipBackground: 'rgba(10, 10, 10, 0.92)',
    mainTooltipBackground: 'rgba(15, 23, 42, 0.95)',
    tooltipText: EXCEPTION_ACCENT.textOnDark,
    tooltipShadow: 'rgba(0, 0, 0, 0.5)',
    staticWhite: EXCEPTION_ACCENT.staticWhite,
  },
} as const;
