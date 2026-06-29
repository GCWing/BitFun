import type {
  BorderColors,
  ElementBackgrounds,
  GitColors,
  RadiusConfig,
  ScrollbarColors,
  ThemeConfig,
  WindowControlsConfig,
} from '../types';

type WindowControlState = WindowControlsConfig['minimize'];

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

export function createWindowControls(config: {
  standard: WindowControlState;
  close: WindowControlState;
  common: WindowControlsConfig['common'];
}): WindowControlsConfig {
  return {
    minimize: { ...config.standard },
    maximize: { ...config.standard },
    close: { ...config.close },
    common: { ...config.common },
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
      '5xl': '32px',
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
      '5xl': '32px',
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
    accelerate: 'cubic-bezier(0.4, 0, 1, 1)',
    bounce: 'cubic-bezier(0.68, -0.55, 0.265, 1.55)',
    smooth,
  };
}

export function createDarkNeutralBorder(): BorderColors {
  return {
    subtle: 'rgba(255, 255, 255, 0.12)',
    base: 'rgba(255, 255, 255, 0.18)',
    medium: 'rgba(255, 255, 255, 0.24)',
    strong: 'rgba(255, 255, 255, 0.32)',
    prominent: 'rgba(255, 255, 255, 0.4)',
  };
}

export function createDarkNeutralElement(): ElementBackgrounds {
  return {
    subtle: 'rgba(255, 255, 255, 0.05)',
    soft: 'rgba(255, 255, 255, 0.07)',
    base: 'rgba(255, 255, 255, 0.095)',
    medium: 'rgba(255, 255, 255, 0.125)',
    strong: 'rgba(255, 255, 255, 0.155)',
    elevated: 'rgba(255, 255, 255, 0.19)',
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
    thumb: 'rgba(255, 255, 255, 0.15)',
    thumbHover: 'rgba(255, 255, 255, 0.28)',
  };
}
