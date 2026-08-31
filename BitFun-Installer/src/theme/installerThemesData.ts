import { themes, type ThemeDataName, type ThemeTokenName } from '@bitfun/theme-bitfun';
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

function themeValue(theme: ThemeDataName, token: ThemeTokenName): string {
  return String(themes[theme][token]);
}

const DEFAULT_BLUE = themeValue('dark', 'color.accent.default');
const DARK_CARD_SURFACE = themeValue('dark', 'color.surface.panel');
const MIDNIGHT_CARD_BACKGROUND = '#2b2d30';

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

function createTone(theme: Extract<ThemeDataName, 'dark' | 'light'>): TonePreset {
  return {
    text: {
      primary: themeValue(theme, 'color.content.primary'),
      secondary: themeValue(theme, 'color.content.secondary'),
      muted: themeValue(theme, 'color.content.muted'),
    },
    semantic: {
      success: themeValue(theme, 'color.status.success.content'),
      warning: themeValue(theme, 'color.status.warning.content'),
      error: themeValue(theme, 'color.status.danger.content'),
    },
    border: {
      subtle: themeValue(theme, 'color.border.subtle'),
      base: themeValue(theme, 'color.border.default'),
    },
    element: {
      subtle: themeValue(theme, 'color.surface.subtle'),
      soft: themeValue(theme, 'color.action.neutral.surface'),
      medium: themeValue(theme, 'color.action.neutral.surfacePressed'),
    },
  };
}

const DARK_TONE = createTone('dark');
const LIGHT_TONE = createTone('light');

function createBuiltinInstallerTheme(
  id: Extract<ThemeId, 'bitfun-dark' | 'bitfun-light'>,
  name: string,
  type: Extract<ThemeDataName, 'dark' | 'light'>,
): InstallerTheme {
  const tone = type === 'light' ? LIGHT_TONE : DARK_TONE;
  return {
    id,
    name,
    type,
    colors: {
      background: {
        primary: themeValue(type, 'color.surface.canvas'),
        secondary: themeValue(type, 'color.surface.panel'),
      },
      text: { ...tone.text },
      accent: themeValue(type, 'color.accent.default'),
      semantic: { ...tone.semantic },
      border: { ...tone.border },
      element: { ...tone.element },
    },
  };
}

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
  createBuiltinInstallerTheme('bitfun-dark', 'Dark', 'dark'),
  createBuiltinInstallerTheme('bitfun-light', 'Light', 'light'),
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
