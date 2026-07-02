export type MobileThemeVars = Record<string, string>;

export const TRANSPARENT = 'transparent';

export function alpha(rgb: string, opacity: string): string {
  return `rgba(${rgb}, ${opacity})`;
}

export function shadow(...layers: string[]): string {
  return layers.join(', ');
}

export function colorRamp(
  prefix: string,
  rgb: string,
  solid500: string,
  solid600: string,
  stops: readonly [string, string, string, string, string] = ['0.04', '0.08', '0.15', '0.25', '0.4'],
): MobileThemeVars {
  return {
    [`${prefix}-50`]: alpha(rgb, stops[0]),
    [`${prefix}-100`]: alpha(rgb, stops[1]),
    [`${prefix}-200`]: alpha(rgb, stops[2]),
    [`${prefix}-300`]: alpha(rgb, stops[3]),
    [`${prefix}-400`]: alpha(rgb, stops[4]),
    [`${prefix}-500`]: solid500,
    [`${prefix}-600`]: solid600,
  };
}

export const commonMobileThemeVars: MobileThemeVars = {
  '--color-static-white': '#ffffff',

  '--size-radius-sm': '6px',
  '--size-radius-base': '8px',
  '--size-radius-lg': '12px',
  '--size-radius-xl': '16px',
  '--size-radius-2xl': '20px',
  '--size-radius-full': '9999px',

  '--size-gap-1': '4px',
  '--size-gap-2': '8px',
  '--size-gap-3': '12px',
  '--size-gap-4': '16px',
  '--size-gap-5': '20px',
  '--size-gap-6': '24px',
  '--size-gap-8': '32px',
  '--size-gap-10': '40px',
  '--size-gap-12': '48px',
  '--size-gap-16': '64px',

  '--motion-instant': '0.1s',
  '--motion-fast': '0.15s',
  '--motion-base': '0.3s',
  '--motion-slow': '0.6s',

  '--easing-standard': 'cubic-bezier(0.4, 0, 0.2, 1)',
  '--easing-decelerate': 'cubic-bezier(0, 0, 0.2, 1)',
  '--easing-smooth': 'cubic-bezier(0.4, 0, 0.2, 1)',

  '--font-family-sans': "'Noto Sans SC', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'SF Pro Display', Roboto, sans-serif",
  '--font-family-mono': "'Menlo', 'SF Mono', 'Cascadia Code', 'Consolas', 'Liberation Mono', 'Courier New', 'Noto Sans Mono CJK SC', 'Noto Sans Mono', monospace",
  '--font-weight-normal': '400',
  '--font-weight-medium': '500',
  '--font-weight-semibold': '600',
  '--font-weight-bold': '700',

  '--font-size-xs': '12px',
  '--font-size-sm': '14px',
  '--font-size-base': '15px',
  '--font-size-lg': '16px',
  '--font-size-xl': '18px',
  '--font-size-2xl': '20px',
  '--font-size-3xl': '24px',

  '--line-height-tight': '1.2',
  '--line-height-base': '1.5',
  '--line-height-relaxed': '1.6',
};
