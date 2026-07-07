import type { ThemeId } from '../types/installer';

type BackgroundColors = InstallerTheme['colors']['background'];
type TextColors = InstallerTheme['colors']['text'];
type SemanticColors = InstallerTheme['colors']['semantic'];
type BorderColors = InstallerTheme['colors']['border'];
type ElementColors = InstallerTheme['colors']['element'];

export type InstallerTheme = {
  id: ThemeId;
  name: string;
  type: 'dark' | 'light';
  colors: {
    background: {
      primary: string;
      secondary: string;
    };
    text: {
      primary: string;
      secondary: string;
      muted: string;
    };
    accent: string;
    semantic: {
      success: string;
      warning: string;
      error: string;
    };
    border: {
      subtle: string;
      base: string;
    };
    element: {
      subtle: string;
      soft: string;
      medium: string;
    };
  };
};

const DEFAULT_BLUE = '#60a5fa';
const DARK_CARD_BACKGROUND = '#121214';
const DARK_CARD_SURFACE = '#1a1c1e';
const MIDNIGHT_CARD_BACKGROUND = '#2b2d30';

function alpha(rgb: string, opacity: string): string {
  return `rgba(${rgb}, ${opacity})`;
}

type TonePreset = {
  text: TextColors;
  semantic: SemanticColors;
  border: BorderColors;
  element: ElementColors;
};

type ThemeSeed = {
  id: ThemeId;
  name: string;
  type: 'dark' | 'light';
  background: {
    primary: string;
    secondary?: string;
  };
  accent: string;
  semantic?: Partial<SemanticColors>;
};

function createBackground(seed: ThemeSeed['background']): BackgroundColors {
  const secondary = seed.secondary ?? seed.primary;
  return {
    primary: seed.primary,
    secondary,
  };
}

function createBorderRamp(rgb: string, alphas: readonly [string, string]): BorderColors {
  return {
    subtle: alpha(rgb, alphas[0]),
    base: alpha(rgb, alphas[1]),
  };
}

function createElementRamp(rgb: string): ElementColors {
  return {
    subtle: alpha(rgb, '0.06'),
    soft: alpha(rgb, '0.12'),
    medium: alpha(rgb, '0.18'),
  };
}

const DARK_TONE: TonePreset = {
  text: { primary: '#e8e8e8', secondary: '#b0b0b0', muted: '#858585' },
  semantic: {
    success: '#34d399',
    warning: '#f59e0b',
    error: '#ef4444',
  },
  border: createBorderRamp('255, 255, 255', ['0.12', '0.18']),
  element: createElementRamp('255, 255, 255'),
};

const LIGHT_TONE: TonePreset = {
  text: { primary: '#1e293b', secondary: '#3d4f66', muted: '#64748b' },
  semantic: {
    success: '#5b9a6f',
    warning: '#c08c42',
    error: '#c26565',
  },
  border: createBorderRamp('100, 116, 139', ['0.15', '0.22']),
  element: createElementRamp('71, 102, 143'),
};

function createInstallerTheme(seed: ThemeSeed): InstallerTheme {
  const tone = seed.type === 'light' ? LIGHT_TONE : DARK_TONE;

  return {
    id: seed.id,
    name: seed.name,
    type: seed.type,
    colors: {
      background: createBackground(seed.background),
      text: { ...tone.text },
      accent: seed.accent,
      semantic: {
        success: tone.semantic.success,
        warning: tone.semantic.warning,
        error: tone.semantic.error,
        ...seed.semantic,
      },
      border: { ...tone.border },
      element: { ...tone.element },
    },
  };
}

export const THEMES: InstallerTheme[] = [
  createInstallerTheme({
    id: 'bitfun-dark',
    name: 'Dark',
    type: 'dark',
    background: {
      primary: DARK_CARD_BACKGROUND,
      secondary: DARK_CARD_SURFACE,
    },
    accent: DEFAULT_BLUE,
  }),
  createInstallerTheme({
    id: 'bitfun-light',
    name: 'Light',
    type: 'light',
    background: { primary: '#f7f8fa', secondary: '#ffffff' },
    accent: '#5a7bb2',
  }),
  createInstallerTheme({
    id: 'bitfun-midnight',
    name: 'Midnight',
    type: 'dark',
    background: { primary: MIDNIGHT_CARD_BACKGROUND, secondary: DARK_CARD_SURFACE },
    accent: DEFAULT_BLUE,
    semantic: {
      success: '#6aab73',
      warning: '#e0a055',
      error: '#cc7f7a',
    },
  }),
  createInstallerTheme({
    id: 'bitfun-china-style',
    name: 'Ink Charm',
    type: 'light',
    background: { primary: '#faf8f0', secondary: '#f5f3e8' },
    accent: '#2e5e8a',
    semantic: {
      success: '#52ad5a',
      warning: '#f0a020',
      error: '#c8102e',
    },
  }),
  createInstallerTheme({
    id: 'bitfun-china-night',
    name: 'Ink Night',
    type: 'dark',
    background: { primary: '#1a1814', secondary: DARK_CARD_SURFACE },
    accent: '#73a5cc',
    semantic: {
      success: '#6bc072',
      warning: '#f5b555',
      error: '#e85555',
    },
  }),
  createInstallerTheme({
    id: 'bitfun-cyber',
    name: 'Cyber',
    type: 'dark',
    background: { primary: '#0e0e10', secondary: DARK_CARD_SURFACE },
    accent: '#00e6ff',
    semantic: {
      success: '#00ff9f',
      warning: '#ffcc00',
      error: '#ff0055',
    },
  }),
  createInstallerTheme({
    id: 'bitfun-tokyo-night',
    name: 'Tokyo Night',
    type: 'dark',
    background: { primary: '#1a1b26', secondary: DARK_CARD_SURFACE },
    accent: '#7aa2f7',
    semantic: {
      success: '#9ece6a',
      warning: '#e0af68',
      error: '#f7768e',
    },
  }),
  createInstallerTheme({
    id: 'bitfun-slate',
    name: 'Slate',
    type: 'dark',
    background: { primary: DARK_CARD_SURFACE },
    accent: '#7ab0ee',
    semantic: {
      success: '#7eb09b',
      warning: '#f59e0b',
      error: '#c9878d',
    },
  }),
];

export const THEME_DISPLAY_ORDER: ThemeId[] = [
  'bitfun-light',
  'bitfun-slate',
  'bitfun-dark',
  'bitfun-midnight',
  'bitfun-china-style',
  'bitfun-china-night',
  'bitfun-cyber',
  'bitfun-tokyo-night',
];

export function findInstallerThemeById(id: ThemeId): InstallerTheme {
  return THEMES.find((theme) => theme.id === id)
    ?? THEMES.find((theme) => theme.id === 'bitfun-light')
    ?? THEMES[0];
}
