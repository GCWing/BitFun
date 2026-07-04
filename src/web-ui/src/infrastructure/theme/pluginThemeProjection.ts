import type { ColorValue, ThemeConfig } from './types';

export const PLUGIN_THEME_COLOR_KEYS = [
  'primary',
  'secondary',
  'accent',
  'success',
  'warning',
  'error',
  'info',
] as const;

export type PluginThemeColorKey = typeof PLUGIN_THEME_COLOR_KEYS[number];
export type PluginThemeColorProjection = Record<PluginThemeColorKey, ColorValue>;

export function createPluginThemeColorProjection(
  theme: Pick<ThemeConfig, 'colors'>,
): PluginThemeColorProjection {
  const { colors } = theme;

  return {
    primary: colors.accent[500],
    secondary: colors.purple?.[500] ?? colors.accent[600],
    accent: colors.accent[600],
    success: colors.semantic.success,
    warning: colors.semantic.warning,
    error: colors.semantic.error,
    info: colors.semantic.info,
  };
}
