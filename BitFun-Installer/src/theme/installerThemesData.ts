import type { ThemeId } from '../types/installer';

type AccentStop = '50' | '100' | '200' | '300' | '400' | '500' | '600';
type SecondaryAccentStop = Exclude<AccentStop, '300'> | '800';
type AccentRamp = Record<AccentStop, string>;
type SecondaryAccentRamp = Record<SecondaryAccentStop, string>;
type RampAlphas = readonly [string, string, string, string, string];
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
      tertiary: string;
      quaternary: string;
      elevated: string;
      workbench: string;
      flowchat: string;
      tooltip: string;
    };
    text: {
      primary: string;
      secondary: string;
      muted: string;
      disabled: string;
    };
    accent: AccentRamp;
    purple: SecondaryAccentRamp;
    semantic: {
      success: string;
      warning: string;
      error: string;
      info: string;
      highlight: string;
      highlightBg: string;
    };
    border: {
      subtle: string;
      base: string;
      medium: string;
      strong: string;
      prominent: string;
    };
    element: {
      subtle: string;
      soft: string;
      base: string;
      medium: string;
      strong: string;
      elevated: string;
    };
  };
};

const DEFAULT_RAMP_ALPHAS: RampAlphas = ['0.04', '0.08', '0.15', '0.25', '0.4'];
const DEFAULT_BLUE_RGB = '96, 165, 250';
const DEFAULT_BLUE_500 = '#60a5fa';
const DEFAULT_BLUE_600 = '#3b82f6';

function alpha(rgb: string, opacity: string): string {
  return `rgba(${rgb}, ${opacity})`;
}

function createAccentRamp(
  rgb: string,
  solid500: string,
  solid600: string,
  alphas: RampAlphas = DEFAULT_RAMP_ALPHAS,
): AccentRamp {
  return {
    '50': alpha(rgb, alphas[0]),
    '100': alpha(rgb, alphas[1]),
    '200': alpha(rgb, alphas[2]),
    '300': alpha(rgb, alphas[3]),
    '400': alpha(rgb, alphas[4]),
    '500': solid500,
    '600': solid600,
  };
}

function createSecondaryRampFromAccent(ramp: AccentRamp): SecondaryAccentRamp {
  return {
    '50': ramp['50'],
    '100': ramp['100'],
    '200': ramp['200'],
    '400': ramp['400'],
    '500': ramp['500'],
    '600': ramp['600'],
    '800': ramp['600'],
  };
}

type TonePreset = {
  text: TextColors;
  semantic: Omit<SemanticColors, 'info' | 'highlight'>;
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
    tooltipRgb: string;
    tooltipAlpha?: string;
  };
  accentRgb: string;
  accent500: string;
  accent600: string;
  semantic?: Partial<SemanticColors>;
};

function createBackground(seed: ThemeSeed['background']): BackgroundColors {
  const secondary = seed.secondary ?? seed.primary;
  return {
    primary: seed.primary,
    secondary,
    tertiary: seed.primary,
    quaternary: secondary,
    elevated: secondary,
    workbench: seed.primary,
    flowchat: seed.primary,
    tooltip: alpha(seed.tooltipRgb, seed.tooltipAlpha ?? '0.95'),
  };
}

function createBorderRamp(rgb: string, alphas: readonly [string, string, string, string, string]): BorderColors {
  return {
    subtle: alpha(rgb, alphas[0]),
    base: alpha(rgb, alphas[1]),
    medium: alpha(rgb, alphas[2]),
    strong: alpha(rgb, alphas[3]),
    prominent: alpha(rgb, alphas[4]),
  };
}

function createElementRamp(rgb: string, elevated: string): ElementColors {
  return {
    subtle: alpha(rgb, '0.06'),
    soft: alpha(rgb, '0.12'),
    base: alpha(rgb, '0.12'),
    medium: alpha(rgb, '0.18'),
    strong: alpha(rgb, '0.24'),
    elevated,
  };
}

const DARK_TONE: TonePreset = {
  text: { primary: '#e8e8e8', secondary: '#b0b0b0', muted: '#858585', disabled: '#555555' },
  semantic: {
    success: '#34d399',
    warning: '#f59e0b',
    error: '#ef4444',
    highlightBg: alpha('245, 158, 11', '0.15'),
  },
  border: createBorderRamp('255, 255, 255', ['0.12', '0.18', '0.24', '0.32', '0.45']),
  element: createElementRamp('255, 255, 255', alpha('255, 255, 255', '0.24')),
};

const LIGHT_TONE: TonePreset = {
  text: { primary: '#1e293b', secondary: '#3d4f66', muted: '#64748b', disabled: '#94a3b8' },
  semantic: {
    success: '#5b9a6f',
    warning: '#c08c42',
    error: '#c26565',
    highlightBg: alpha('192, 140, 66', '0.12'),
  },
  border: createBorderRamp('100, 116, 139', ['0.15', '0.22', '0.32', '0.42', '0.52']),
  element: createElementRamp('71, 102, 143', alpha('255, 255, 255', '0.92')),
};

function createInstallerTheme(seed: ThemeSeed): InstallerTheme {
  const accent = createAccentRamp(seed.accentRgb, seed.accent500, seed.accent600);
  const tone = seed.type === 'light' ? LIGHT_TONE : DARK_TONE;

  return {
    id: seed.id,
    name: seed.name,
    type: seed.type,
    colors: {
      background: createBackground(seed.background),
      text: { ...tone.text },
      accent,
      purple: createSecondaryRampFromAccent(accent),
      semantic: {
        success: tone.semantic.success,
        warning: tone.semantic.warning,
        error: tone.semantic.error,
        info: accent['500'],
        highlight: tone.semantic.warning,
        highlightBg: tone.semantic.highlightBg,
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
      primary: '#121214',
      secondary: '#1a1c1e',
      tooltipRgb: '30, 30, 32',
      tooltipAlpha: '0.92',
    },
    accentRgb: DEFAULT_BLUE_RGB,
    accent500: DEFAULT_BLUE_500,
    accent600: DEFAULT_BLUE_600,
  }),
  createInstallerTheme({
    id: 'bitfun-light',
    name: 'Light',
    type: 'light',
    background: { primary: '#f7f8fa', secondary: '#ffffff', tooltipRgb: '255, 255, 255', tooltipAlpha: '0.98' },
    accentRgb: '71, 102, 143',
    accent500: '#5a7bb2',
    accent600: '#4a6694',
  }),
  createInstallerTheme({
    id: 'bitfun-midnight',
    name: 'Midnight',
    type: 'dark',
    background: { primary: '#2b2d30', secondary: '#1e1f22', tooltipRgb: '43, 45, 48', tooltipAlpha: '0.94' },
    accentRgb: DEFAULT_BLUE_RGB,
    accent500: DEFAULT_BLUE_500,
    accent600: DEFAULT_BLUE_600,
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
    background: { primary: '#faf8f0', secondary: '#f5f3e8', tooltipRgb: '250, 248, 240', tooltipAlpha: '0.96' },
    accentRgb: '46, 94, 138',
    accent500: '#2e5e8a',
    accent600: '#234a6d',
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
    background: { primary: '#1a1814', secondary: '#212019', tooltipRgb: '26, 24, 20' },
    accentRgb: '115, 165, 204',
    accent500: '#73a5cc',
    accent600: '#5a8bb3',
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
    background: { primary: '#0e0e10', secondary: '#151515', tooltipRgb: '14, 14, 16' },
    accentRgb: '0, 230, 255',
    accent500: '#00e6ff',
    accent600: '#00ccff',
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
    background: { primary: '#1a1b26', secondary: '#16161e', tooltipRgb: '22, 22, 30', tooltipAlpha: '0.94' },
    accentRgb: '122, 162, 247',
    accent500: '#7aa2f7',
    accent600: '#6183bb',
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
    background: { primary: '#1a1c1e', tooltipRgb: '42, 45, 48', tooltipAlpha: '0.96' },
    accentRgb: '122, 176, 238',
    accent500: '#7ab0ee',
    accent600: '#689ad8',
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
