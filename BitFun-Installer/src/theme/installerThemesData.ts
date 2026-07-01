import type { ThemeId } from '../types/installer';

type AccentStop = '50' | '100' | '200' | '300' | '400' | '500' | '600';
type SecondaryAccentStop = Exclude<AccentStop, '300'> | '800';
type AccentRamp = Record<AccentStop, string>;
type SecondaryAccentRamp = Record<SecondaryAccentStop, string>;
type RampAlphas = readonly [string, string, string, string, string];

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
const LIGHT_RAMP_ALPHAS: RampAlphas = ['0.04', '0.08', '0.14', '0.22', '0.36'];
const CYBER_RAMP_ALPHAS: RampAlphas = ['0.05', '0.1', '0.18', '0.3', '0.45'];
const PURPLE_800_ALPHA = {
  default: '0.9',
  cyber: '0.95',
} as const;

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

function createSecondaryRamp(
  rgb: string,
  solid500: string,
  solid600: string,
  strongRgb = rgb,
  alphas: RampAlphas = DEFAULT_RAMP_ALPHAS,
  highAlpha: string = PURPLE_800_ALPHA.default,
): SecondaryAccentRamp {
  const ramp = createAccentRamp(rgb, solid500, solid600, alphas);
  return {
    '50': ramp['50'],
    '100': ramp['100'],
    '200': ramp['200'],
    '400': ramp['400'],
    '500': ramp['500'],
    '600': ramp['600'],
    '800': alpha(strongRgb, highAlpha),
  };
}

export const THEMES: InstallerTheme[] = [
  {
    id: 'bitfun-dark',
    name: 'Dark',
    type: 'dark',
    colors: {
      background: { primary: '#121214', secondary: '#18181a', tertiary: '#121214', quaternary: '#202024', elevated: '#18181a', workbench: '#121214', flowchat: '#121214', tooltip: alpha('30, 30, 32', '0.92') },
      text: { primary: '#e8e8e8', secondary: '#b0b0b0', muted: '#858585', disabled: '#555555' },
      accent: createAccentRamp('96, 165, 250', '#60a5fa', '#3b82f6'),
      purple: createSecondaryRamp('139, 92, 246', '#8b5cf6', '#7c3aed', '124, 58, 237'),
      semantic: { success: '#34d399', warning: '#f59e0b', error: '#ef4444', info: '#E1AB80', highlight: '#d4a574', highlightBg: alpha('212, 165, 116', '0.15') },
      border: { subtle: alpha('255, 255, 255', '0.12'), base: alpha('255, 255, 255', '0.18'), medium: alpha('255, 255, 255', '0.24'), strong: alpha('255, 255, 255', '0.32'), prominent: alpha('225, 171, 128', '0.50') },
      element: { subtle: alpha('255, 255, 255', '0.06'), soft: alpha('255, 255, 255', '0.10'), base: alpha('255, 255, 255', '0.13'), medium: alpha('255, 255, 255', '0.17'), strong: alpha('255, 255, 255', '0.21'), elevated: alpha('255, 255, 255', '0.25') },
    },
  },
  {
    id: 'bitfun-light',
    name: 'Light',
    type: 'light',
    colors: {
      background: { primary: '#f7f8fa', secondary: '#ffffff', tertiary: '#f3f5f8', quaternary: '#ebeef3', elevated: '#ffffff', workbench: '#f7f8fa', flowchat: '#f7f8fa', tooltip: alpha('255, 255, 255', '0.98') },
      text: { primary: '#1e293b', secondary: '#3d4f66', muted: '#64748b', disabled: '#94a3b8' },
      accent: createAccentRamp('71, 102, 143', '#5a7bb2', '#4a6694', LIGHT_RAMP_ALPHAS),
      purple: createSecondaryRamp('107, 90, 137', '#7c6b99', '#655680', '101, 86, 128', LIGHT_RAMP_ALPHAS),
      semantic: { success: '#5b9a6f', warning: '#c08c42', error: '#c26565', info: '#5a7bb2', highlight: '#b8863a', highlightBg: alpha('184, 134, 58', '0.12') },
      border: { subtle: alpha('100, 116, 139', '0.15'), base: alpha('100, 116, 139', '0.22'), medium: alpha('100, 116, 139', '0.32'), strong: alpha('100, 116, 139', '0.42'), prominent: alpha('100, 116, 139', '0.52') },
      element: { subtle: alpha('71, 102, 143', '0.05'), soft: alpha('71, 102, 143', '0.08'), base: alpha('71, 102, 143', '0.11'), medium: alpha('71, 102, 143', '0.15'), strong: alpha('71, 102, 143', '0.20'), elevated: alpha('255, 255, 255', '0.92') },
    },
  },
  {
    id: 'bitfun-midnight',
    name: 'Midnight',
    type: 'dark',
    colors: {
      background: { primary: '#2b2d30', secondary: '#1e1f22', tertiary: '#313335', quaternary: '#3c3f41', elevated: '#2b2d30', workbench: '#212121', flowchat: '#2b2d30', tooltip: alpha('43, 45, 48', '0.94') },
      text: { primary: '#bcbec4', secondary: '#9da0a8', muted: '#6f737a', disabled: '#4e5157' },
      accent: createAccentRamp('88, 166, 255', '#58a6ff', '#3b82f6'),
      purple: createSecondaryRamp('156, 120, 255', '#9c78ff', '#8b5cf6', '139, 92, 246'),
      semantic: { success: '#6aab73', warning: '#e0a055', error: '#cc7f7a', info: '#58a6ff', highlight: '#d4a574', highlightBg: alpha('212, 165, 116', '0.15') },
      border: { subtle: alpha('255, 255, 255', '0.08'), base: alpha('255, 255, 255', '0.14'), medium: alpha('255, 255, 255', '0.20'), strong: alpha('255, 255, 255', '0.26'), prominent: alpha('255, 255, 255', '0.35') },
      element: { subtle: alpha('255, 255, 255', '0.04'), soft: alpha('255, 255, 255', '0.06'), base: alpha('255, 255, 255', '0.09'), medium: alpha('255, 255, 255', '0.12'), strong: alpha('255, 255, 255', '0.15'), elevated: alpha('255, 255, 255', '0.18') },
    },
  },
  {
    id: 'bitfun-china-style',
    name: 'Ink Charm',
    type: 'light',
    colors: {
      background: { primary: '#faf8f0', secondary: '#f5f3e8', tertiary: '#f0ede0', quaternary: '#ebe8d8', elevated: '#ebe9e3', workbench: '#faf8f0', flowchat: '#faf8f0', tooltip: alpha('250, 248, 240', '0.96') },
      text: { primary: '#1a1a1a', secondary: '#3d3d3d', muted: '#6a6a6a', disabled: '#9a9a9a' },
      accent: createAccentRamp('46, 94, 138', '#2e5e8a', '#234a6d'),
      purple: createSecondaryRamp('126, 176, 155', '#7eb09b', '#5a9078', '90, 144, 120'),
      semantic: { success: '#52ad5a', warning: '#f0a020', error: '#c8102e', info: '#2e5e8a', highlight: '#f0a020', highlightBg: alpha('240, 160, 32', '0.12') },
      border: { subtle: alpha('106, 92, 70', '0.12'), base: alpha('106, 92, 70', '0.20'), medium: alpha('106, 92, 70', '0.28'), strong: alpha('106, 92, 70', '0.36'), prominent: alpha('106, 92, 70', '0.48') },
      element: { subtle: alpha('46, 94, 138', '0.03'), soft: alpha('46, 94, 138', '0.06'), base: alpha('46, 94, 138', '0.10'), medium: alpha('46, 94, 138', '0.14'), strong: alpha('46, 94, 138', '0.18'), elevated: alpha('255, 255, 255', '0.85') },
    },
  },
  {
    id: 'bitfun-china-night',
    name: 'Ink Night',
    type: 'dark',
    colors: {
      background: { primary: '#1a1814', secondary: '#212019', tertiary: '#262420', quaternary: '#2d2926', elevated: '#2d2926', workbench: '#1a1814', flowchat: '#1a1814', tooltip: alpha('26, 24, 20', '0.95') },
      text: { primary: '#e8e6e1', secondary: '#c5c3be', muted: '#928f89', disabled: '#5f5d59' },
      accent: createAccentRamp('115, 165, 204', '#73a5cc', '#5a8bb3'),
      purple: createSecondaryRamp('150, 198, 180', '#96c6b4', '#7aab98', '122, 171, 152'),
      semantic: { success: '#6bc072', warning: '#f5b555', error: '#e85555', info: '#73a5cc', highlight: '#e6a84a', highlightBg: alpha('230, 168, 74', '0.15') },
      border: { subtle: alpha('232, 230, 225', '0.10'), base: alpha('232, 230, 225', '0.16'), medium: alpha('232, 230, 225', '0.22'), strong: alpha('232, 230, 225', '0.28'), prominent: alpha('232, 230, 225', '0.38') },
      element: { subtle: alpha('115, 165, 204', '0.06'), soft: alpha('115, 165, 204', '0.09'), base: alpha('115, 165, 204', '0.12'), medium: alpha('115, 165, 204', '0.16'), strong: alpha('115, 165, 204', '0.20'), elevated: alpha('45, 41, 38', '0.95') },
    },
  },
  {
    id: 'bitfun-cyber',
    name: 'Cyber',
    type: 'dark',
    colors: {
      background: { primary: '#101010', secondary: '#151515', tertiary: '#1a1a1a', quaternary: '#1f1f1f', elevated: '#0d0d0d', workbench: '#101010', flowchat: '#101010', tooltip: alpha('16, 16, 16', '0.95') },
      text: { primary: '#e0f2ff', secondary: '#c7e7ff', muted: '#7fadcc', disabled: '#4a5a66' },
      accent: createAccentRamp('0, 230, 255', '#00e6ff', '#00ccff', CYBER_RAMP_ALPHAS),
      purple: createSecondaryRamp('138, 43, 226', '#8a2be2', '#7928ca', '121, 40, 202', CYBER_RAMP_ALPHAS, PURPLE_800_ALPHA.cyber),
      semantic: { success: '#00ff9f', warning: '#ffcc00', error: '#ff0055', info: '#00e6ff', highlight: '#ffdd44', highlightBg: alpha('255, 221, 68', '0.15') },
      border: { subtle: alpha('0, 230, 255', '0.14'), base: alpha('0, 230, 255', '0.20'), medium: alpha('0, 230, 255', '0.28'), strong: alpha('0, 230, 255', '0.36'), prominent: alpha('0, 230, 255', '0.50') },
      element: { subtle: alpha('0, 230, 255', '0.06'), soft: alpha('0, 230, 255', '0.09'), base: alpha('0, 230, 255', '0.13'), medium: alpha('0, 230, 255', '0.17'), strong: alpha('0, 230, 255', '0.22'), elevated: alpha('0, 230, 255', '0.27') },
    },
  },
  {
    id: 'bitfun-tokyo-night',
    name: 'Tokyo Night',
    type: 'dark',
    colors: {
      background: { primary: '#1a1b26', secondary: '#16161e', tertiary: '#14141b', quaternary: '#1e202e', elevated: '#20222c', workbench: '#16161e', flowchat: '#1a1b26', tooltip: alpha('22, 22, 30', '0.94') },
      text: { primary: '#c0caf5', secondary: '#a9b1d6', muted: '#787c99', disabled: '#545c7e' },
      accent: createAccentRamp('122, 162, 247', '#7aa2f7', '#6183bb', ['0.05', '0.08', '0.15', '0.25', '0.4']),
      purple: createSecondaryRamp('187, 154, 247', '#bb9af7', '#9d7cd8', '157, 124, 216', ['0.05', '0.08', '0.15', '0.25', '0.4'], PURPLE_800_ALPHA.cyber),
      semantic: { success: '#9ece6a', warning: '#e0af68', error: '#f7768e', info: '#7dcfff', highlight: '#e0af68', highlightBg: alpha('224, 175, 104', '0.15') },
      border: { subtle: alpha('54, 59, 84', '0.45'), base: alpha('54, 59, 84', '0.6'), medium: alpha('54, 59, 84', '0.72'), strong: alpha('54, 59, 84', '0.85'), prominent: alpha('122, 162, 247', '0.45') },
      element: { subtle: alpha('122, 162, 247', '0.06'), soft: alpha('122, 162, 247', '0.08'), base: alpha('122, 162, 247', '0.11'), medium: alpha('122, 162, 247', '0.14'), strong: alpha('122, 162, 247', '0.18'), elevated: alpha('122, 162, 247', '0.22') },
    },
  },
  {
    id: 'bitfun-slate',
    name: 'Slate',
    type: 'dark',
    colors: {
      background: { primary: '#1a1c1e', secondary: '#1a1c1e', tertiary: '#1a1c1e', quaternary: '#32363a', elevated: '#1a1c1e', workbench: '#1a1c1e', flowchat: '#1a1c1e', tooltip: alpha('42, 45, 48', '0.96') },
      text: { primary: '#eef0f3', secondary: '#c8ccd2', muted: '#9ea4ab', disabled: '#65696f' },
      accent: createAccentRamp('122, 176, 238', '#7ab0ee', '#689ad8'),
      purple: createSecondaryRamp('184, 198, 255', '#b8c4ff', '#9dacf5', '157, 172, 245'),
      semantic: { success: '#7fb899', warning: '#d4a574', error: '#c9878d', info: '#7ab0ee', highlight: '#e2e4e7', highlightBg: alpha('212, 214, 216', '0.12') },
      border: { subtle: alpha('255, 255, 255', '0.12'), base: alpha('255, 255, 255', '0.18'), medium: alpha('255, 255, 255', '0.24'), strong: alpha('255, 255, 255', '0.32'), prominent: alpha('255, 255, 255', '0.45') },
      element: { subtle: alpha('255, 255, 255', '0.06'), soft: alpha('255, 255, 255', '0.10'), base: alpha('255, 255, 255', '0.13'), medium: alpha('255, 255, 255', '0.17'), strong: alpha('255, 255, 255', '0.21'), elevated: alpha('255, 255, 255', '0.25') },
    },
  },
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
