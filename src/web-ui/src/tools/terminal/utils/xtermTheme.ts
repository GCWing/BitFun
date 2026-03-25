import type { ITheme } from '@xterm/xterm';
import type { ThemeConfig, ThemeType } from '@/infrastructure/theme/types';

export const DEFAULT_XTERM_MINIMUM_CONTRAST_RATIO = 6;

const LIGHT_ANSI: Required<Pick<ITheme,
  | 'black'
  | 'red'
  | 'green'
  | 'yellow'
  | 'blue'
  | 'magenta'
  | 'cyan'
  | 'white'
  | 'brightBlack'
  | 'brightRed'
  | 'brightGreen'
  | 'brightYellow'
  | 'brightBlue'
  | 'brightMagenta'
  | 'brightCyan'
  | 'brightWhite'
>> = {
  black: '#000000',
  red: '#cd3131',
  green: '#107C10',
  yellow: '#949800',
  blue: '#0451a5',
  magenta: '#bc05bc',
  cyan: '#0598bc',
  white: '#555555',
  brightBlack: '#666666',
  brightRed: '#cd3131',
  brightGreen: '#14CE14',
  brightYellow: '#b5ba00',
  brightBlue: '#0451a5',
  brightMagenta: '#bc05bc',
  brightCyan: '#0598bc',
  brightWhite: '#a5a5a5',
};

const DARK_ANSI: Required<Pick<ITheme,
  | 'black'
  | 'red'
  | 'green'
  | 'yellow'
  | 'blue'
  | 'magenta'
  | 'cyan'
  | 'white'
  | 'brightBlack'
  | 'brightRed'
  | 'brightGreen'
  | 'brightYellow'
  | 'brightBlue'
  | 'brightMagenta'
  | 'brightCyan'
  | 'brightWhite'
>> = {
  black: '#000000',
  red: '#cd3131',
  green: '#0dbc79',
  yellow: '#e5e510',
  blue: '#2472c8',
  magenta: '#bc3fbc',
  cyan: '#11a8cd',
  white: '#e5e5e5',
  brightBlack: '#666666',
  brightRed: '#f14c4c',
  brightGreen: '#23d18b',
  brightYellow: '#f5f543',
  brightBlue: '#3b8eea',
  brightMagenta: '#d670d6',
  brightCyan: '#29b8db',
  brightWhite: '#e5e5e5',
};

export function getXtermAnsiPalette(themeType: ThemeType): Required<Pick<ITheme,
  | 'black'
  | 'red'
  | 'green'
  | 'yellow'
  | 'blue'
  | 'magenta'
  | 'cyan'
  | 'white'
  | 'brightBlack'
  | 'brightRed'
  | 'brightGreen'
  | 'brightYellow'
  | 'brightBlue'
  | 'brightMagenta'
  | 'brightCyan'
  | 'brightWhite'
>> {
  return themeType === 'dark' ? DARK_ANSI : LIGHT_ANSI;
}

export function getXtermFontWeights(themeType: ThemeType): {
  fontWeight: 'normal' | '500';
  fontWeightBold: 'bold' | '700';
} {
  return themeType === 'dark'
    ? { fontWeight: 'normal', fontWeightBold: 'bold' }
    : { fontWeight: '500', fontWeightBold: '700' };
}

export function buildXtermTheme(
  theme: ThemeConfig,
  overrides: Partial<ITheme> = {},
): ITheme {
  const isDark = theme.type === 'dark';

  return {
    background: theme.colors.background.scene,
    foreground: theme.colors.text.primary,
    cursor: theme.colors.text.primary,
    cursorAccent: theme.colors.background.secondary,
    selectionBackground: isDark ? 'rgba(255, 255, 255, 0.30)' : 'rgba(173, 214, 255, 0.45)',
    selectionInactiveBackground: isDark ? 'rgba(255, 255, 255, 0.16)' : 'rgba(173, 214, 255, 0.25)',
    ...getXtermAnsiPalette(theme.type),
    ...overrides,
  };
}
