// Last-resort values for isolated surfaces that can render before root theme
// variables are available. Keep these values exact and boundary-scoped.
const BOUNDARY_FALLBACK_COLOR = {
  textPrimary: '#e8e8e8',
  textSecondary: '#b0b0b0',
  textMuted: '#858585',
  accent500: '#60a5fa',
  accent600: '#3b82f6',
  bgSecondary: '#1c1c1f',
  success: '#34d399',
  warning: '#f59e0b',
  error: '#ef4444',
  staticWhite: '#ffffff',
  staticBlack: '#000000',
  overlayWhite08: 'rgba(255, 255, 255, 0.08)',
  overlayWhite12: 'rgba(255, 255, 255, 0.12)',
  overlayWhite24: 'rgba(255, 255, 255, 0.24)',
  overlayBlack15: 'rgba(0, 0, 0, 0.15)',
  overlayBlack30: 'rgba(0, 0, 0, 0.3)',
  shadowBase: 'rgba(0, 0, 0, 0.4)',
  captureBackground: '#121214',
} as const;

export const WIDGET_IFRAME_FALLBACK_COLOR = {
  textPrimary: BOUNDARY_FALLBACK_COLOR.textPrimary,
  textSecondary: BOUNDARY_FALLBACK_COLOR.textSecondary,
  textMuted: BOUNDARY_FALLBACK_COLOR.textMuted,
  accent500: BOUNDARY_FALLBACK_COLOR.accent500,
  accent600: BOUNDARY_FALLBACK_COLOR.accent600,
  bgSecondary: BOUNDARY_FALLBACK_COLOR.bgSecondary,
  success: BOUNDARY_FALLBACK_COLOR.success,
  warning: BOUNDARY_FALLBACK_COLOR.warning,
  error: BOUNDARY_FALLBACK_COLOR.error,
  staticWhite: BOUNDARY_FALLBACK_COLOR.staticWhite,
  staticBlack: BOUNDARY_FALLBACK_COLOR.staticBlack,
  borderSubtle: BOUNDARY_FALLBACK_COLOR.overlayWhite12,
  borderBase: BOUNDARY_FALLBACK_COLOR.overlayWhite12,
  borderMedium: BOUNDARY_FALLBACK_COLOR.overlayWhite24,
  elementBgSubtle: BOUNDARY_FALLBACK_COLOR.overlayWhite08,
  elementBgBase: BOUNDARY_FALLBACK_COLOR.overlayWhite12,
  elementBgMedium: BOUNDARY_FALLBACK_COLOR.overlayWhite12,
  shadowBase: BOUNDARY_FALLBACK_COLOR.shadowBase,
} as const;

export const MINI_APP_SCROLLBAR_FALLBACKS = {
  dark: {
    thumb: BOUNDARY_FALLBACK_COLOR.overlayWhite12,
    thumbHover: BOUNDARY_FALLBACK_COLOR.overlayWhite24,
  },
  light: {
    thumb: BOUNDARY_FALLBACK_COLOR.overlayBlack15,
    thumbHover: BOUNDARY_FALLBACK_COLOR.overlayBlack30,
  },
} as const;

export const FLOWCHAT_CAPTURE_FALLBACK_COLOR = {
  background: BOUNDARY_FALLBACK_COLOR.captureBackground,
} as const;
