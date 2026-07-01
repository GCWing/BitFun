import type {
  AccentColors,
  BorderColors,
  ElementBackgrounds,
  GitColors,
  RadiusConfig,
  ScrollbarColors,
  SecondaryAccentColors,
  SemanticColors,
  ThemeConfig,
  WindowControlsConfig,
} from '../types';

export const STATIC_BLACK = '#000000';
export const STATIC_WHITE = '#ffffff';

function hexToRgbChannels(hex: string): [number, number, number] {
  const raw = hex.trim().replace(/^#/, '');
  const expanded = raw.length === 3
    ? raw.split('').map(channel => channel + channel).join('')
    : raw;
  if (!/^[0-9a-f]{6}$/i.test(expanded)) {
    throw new Error(`Invalid hex color: ${hex}`);
  }
  const value = Number.parseInt(expanded, 16);
  return [
    (value >> 16) & 255,
    (value >> 8) & 255,
    value & 255,
  ];
}

export function rgbFromHex(hex: string): string {
  const [r, g, b] = hexToRgbChannels(hex);
  return `rgb(${r}, ${g}, ${b})`;
}

export function rgbaFromHex(hex: string, alpha: number | string): string {
  const [r, g, b] = hexToRgbChannels(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function overlayBlack(alpha: number | string): string {
  return rgbaFromHex(STATIC_BLACK, alpha);
}

export function overlayWhite(alpha: number | string): string {
  return rgbaFromHex(STATIC_WHITE, alpha);
}

export interface AccentScaleInput {
  base: string;
  hover?: string;
  stops?: Partial<AccentColors>;
  alpha?: Partial<Record<50 | 100 | 200 | 300 | 400 | 700 | 800, number | string>>;
}

export interface SecondaryAccentScaleInput {
  base: string;
  hover?: string;
  stops?: Partial<SecondaryAccentColors>;
  alpha?: Partial<Record<50 | 100 | 200 | 400 | 800, number | string>>;
}

export interface SemanticColorsInput {
  success: string;
  warning: string;
  error: string;
  info: string;
  bgAlpha?: number | string;
  borderAlpha?: number | string;
  overrides?: Partial<SemanticColors>;
}

export function createAccentScale(input: AccentScaleInput): AccentColors {
  const hover = input.hover ?? input.base;
  const alpha = {
    50: 0.04,
    100: 0.08,
    200: 0.15,
    300: 0.25,
    400: 0.4,
    700: 0.8,
    800: 0.9,
    ...input.alpha,
  };

  return {
    50: rgbaFromHex(input.base, alpha[50]),
    100: rgbaFromHex(input.base, alpha[100]),
    200: rgbaFromHex(input.base, alpha[200]),
    300: rgbaFromHex(input.base, alpha[300]),
    400: rgbaFromHex(input.base, alpha[400]),
    500: input.base,
    600: hover,
    700: rgbaFromHex(hover, alpha[700]),
    800: rgbaFromHex(hover, alpha[800]),
    ...input.stops,
  };
}

export function createSecondaryAccentScale(input: SecondaryAccentScaleInput): SecondaryAccentColors {
  const hover = input.hover ?? input.base;
  const alpha = {
    50: 0.04,
    100: 0.08,
    200: 0.15,
    400: 0.4,
    800: 0.9,
    ...input.alpha,
  };

  return {
    50: rgbaFromHex(input.base, alpha[50]),
    100: rgbaFromHex(input.base, alpha[100]),
    200: rgbaFromHex(input.base, alpha[200]),
    400: rgbaFromHex(input.base, alpha[400]),
    500: input.base,
    600: hover,
    800: rgbaFromHex(hover, alpha[800]),
    ...input.stops,
  };
}

export function createSemanticColors(input: SemanticColorsInput): SemanticColors {
  const bgAlpha = input.bgAlpha ?? 0.1;
  const borderAlpha = input.borderAlpha ?? 0.3;

  return {
    success: input.success,
    successBg: rgbaFromHex(input.success, bgAlpha),
    successBorder: rgbaFromHex(input.success, borderAlpha),
    warning: input.warning,
    warningBg: rgbaFromHex(input.warning, bgAlpha),
    warningBorder: rgbaFromHex(input.warning, borderAlpha),
    error: input.error,
    errorBg: rgbaFromHex(input.error, bgAlpha),
    errorBorder: rgbaFromHex(input.error, borderAlpha),
    info: input.info,
    infoBg: rgbaFromHex(input.info, bgAlpha),
    infoBorder: rgbaFromHex(input.info, borderAlpha),
    ...input.overrides,
  };
}

export function createWindowControls(closeHoverColor: WindowControlsConfig['close']['hoverColor']): WindowControlsConfig {
  return {
    close: { hoverColor: closeHoverColor },
  };
}

export function createStandardTypography(): ThemeConfig['typography'] {
  return {
    font: {
      sans: "'Noto Sans SC', -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Hiragino Sans GB', 'Segoe UI', 'Microsoft YaHei UI', 'Microsoft YaHei', 'Helvetica Neue', Helvetica, Arial, sans-serif",
      mono: "'JetBrains Mono', 'FiraCode', ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Monaco, 'Cascadia Mono', 'Cascadia Code', Consolas, 'Liberation Mono', 'Courier New', monospace",
    },
    weight: {
      normal: 400,
      medium: 500,
      semibold: 600,
      bold: 700,
    },
    size: {
      xs: '12px',
      sm: '13px',
      base: '14px',
      lg: '15px',
      xl: '16px',
      '2xl': '18px',
      '3xl': '22px',
      '4xl': '26px',
    },
    lineHeight: {
      tight: 1.2,
      base: 1.5,
      relaxed: 1.6,
    },
  };
}

export function createExpressiveTypography(): ThemeConfig['typography'] {
  return {
    ...createStandardTypography(),
    lineHeight: {
      tight: 1.3,
      base: 1.5,
      relaxed: 1.65,
    },
  };
}

export function createChinaTypography(): ThemeConfig['typography'] {
  return {
    font: {
      sans: "'Noto Sans SC', 'Source Han Sans CN', -apple-system, BlinkMacSystemFont, 'PingFang SC', 'Hiragino Sans GB', 'Segoe UI', 'Microsoft YaHei UI', 'Microsoft YaHei', 'Helvetica Neue', Helvetica, Arial, sans-serif",
      mono: "'Source Han Mono CN', 'Noto Sans Mono CJK SC', 'JetBrains Mono', 'FiraCode', ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Monaco, 'Cascadia Mono', 'Cascadia Code', Consolas, 'Liberation Mono', 'Courier New', monospace",
    },
    weight: {
      normal: 400,
      medium: 500,
      semibold: 600,
      bold: 700,
    },
    size: {
      xs: '12px',
      sm: '13px',
      base: '14px',
      lg: '15px',
      xl: '16px',
      '2xl': '18px',
      '3xl': '22px',
      '4xl': '26px',
    },
    lineHeight: {
      tight: 1.3,
      base: 1.6,
      relaxed: 1.8,
    },
  };
}

export function createStandardSpacing(): ThemeConfig['effects']['spacing'] {
  return {
    1: '4px',
    2: '8px',
    3: '12px',
    4: '16px',
    5: '20px',
    6: '24px',
    8: '32px',
    10: '40px',
    12: '48px',
    16: '64px',
  };
}

export function createStandardRadius(): RadiusConfig {
  return {
    sm: '6px',
    base: '8px',
    lg: '12px',
    xl: '16px',
    '2xl': '20px',
    full: '9999px',
  };
}

export function createCompactRadius(): RadiusConfig {
  return {
    sm: '4px',
    base: '6px',
    lg: '10px',
    xl: '14px',
    '2xl': '18px',
    full: '9999px',
  };
}

export function createSlateRadius(): RadiusConfig {
  return {
    sm: '4px',
    base: '6px',
    lg: '8px',
    xl: '12px',
    '2xl': '16px',
    full: '9999px',
  };
}

export function createStandardEasing(smooth = 'cubic-bezier(0.4, 0, 0.2, 1)'): ThemeConfig['motion']['easing'] {
  return {
    standard: 'cubic-bezier(0.4, 0, 0.2, 1)',
    decelerate: 'cubic-bezier(0, 0, 0.2, 1)',
    smooth,
  };
}

export function createDarkNeutralBorder(): BorderColors {
  return {
    subtle: overlayWhite(0.12),
    base: overlayWhite(0.18),
    medium: overlayWhite(0.24),
    strong: overlayWhite(0.3),
    prominent: overlayWhite(0.4),
  };
}

export function createDarkNeutralElement(): ElementBackgrounds {
  return {
    subtle: overlayWhite(0.06),
    soft: overlayWhite(0.06),
    base: overlayWhite(0.1),
    medium: overlayWhite(0.12),
    strong: overlayWhite(0.15),
    elevated: overlayWhite(0.2),
  };
}

export function createGitColors(
  config: Omit<GitColors, 'staged' | 'stagedBg'> & Partial<Pick<GitColors, 'staged' | 'stagedBg'>>,
): GitColors {
  return {
    ...config,
    staged: config.staged ?? config.added,
    stagedBg: config.stagedBg ?? config.addedBg,
  };
}

export function createDarkNeutralScrollbar(): ScrollbarColors {
  return {
    thumb: overlayWhite(0.15),
    thumbHover: overlayWhite(0.3),
  };
}
