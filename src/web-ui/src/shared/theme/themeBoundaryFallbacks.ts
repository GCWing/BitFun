// Last-resort values for isolated surfaces that can render before root theme
// variables are available. Keep these values exact and boundary-scoped.
export const WIDGET_IFRAME_FALLBACK_COLOR = {
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
  borderSubtle: 'rgba(255, 255, 255, 0.12)',
  borderBase: 'rgba(255, 255, 255, 0.12)',
  borderMedium: 'rgba(255, 255, 255, 0.24)',
  elementBgSubtle: 'rgba(255, 255, 255, 0.06)',
  elementBgBase: 'rgba(255, 255, 255, 0.12)',
  elementBgMedium: 'rgba(255, 255, 255, 0.12)',
  shadowBase: 'rgba(0, 0, 0, 0.4)',
} as const;

export const MINI_APP_SCROLLBAR_FALLBACKS = {
  dark: {
    thumb: 'rgba(255, 255, 255, 0.12)',
    thumbHover: 'rgba(255, 255, 255, 0.24)',
  },
  light: {
    thumb: 'rgba(0, 0, 0, 0.15)',
    thumbHover: 'rgba(0, 0, 0, 0.3)',
  },
} as const;

export const FLOWCHAT_CAPTURE_FALLBACK_COLOR = {
  background: '#121214',
} as const;
