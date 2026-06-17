// Concrete exception accents shared by UI metadata that must preserve existing visuals.
// Prefer theme tokens for theme-aware surfaces; do not treat this as a token contract.
export const UI_EXCEPTION_ACCENTS = {
  contextCompression: '#a855f7',
  generativeUi: '#38bdf8',
  miniApp: '#7c8cef',
  tealAction: '#14b8a6',
  todo: '#0d9488',
  textStroke: [
    '#eab308',
    '#ef4444',
    '#3b82f6',
    '#06b6d4',
    '#8b5cf6',
  ],
  inspectorOverlay: {
    activeBorder: '#3b82f6',
    activeBackground: 'rgba(59, 130, 246, 0.12)',
    activeBorderSubtle: 'rgba(59, 130, 246, 0.4)',
    selectedBorder: '#22c55e',
    selectedBackground: 'rgba(34, 197, 94, 0.18)',
    browserTooltipBackground: 'rgba(10, 10, 10, 0.92)',
    mainTooltipBackground: 'rgba(15, 23, 42, 0.95)',
    tooltipText: '#e2e8f0',
    tooltipShadow: 'rgba(0, 0, 0, 0.5)',
    staticWhite: '#ffffff',
  },
} as const;
