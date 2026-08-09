import type { AppearancePalette, ColorValue } from '../builtins/AppearancePalette';

export const PLUGIN_APPEARANCE_COLOR_KEYS = [
  'primary',
  'secondary',
  'accent',
  'success',
  'warning',
  'error',
  'info',
] as const;

export type PluginAppearanceColorKey = typeof PLUGIN_APPEARANCE_COLOR_KEYS[number];
export type PluginAppearanceColorProjection = Record<PluginAppearanceColorKey, ColorValue>;

export function createPluginAppearanceColorProjection(
  palette: Pick<AppearancePalette, 'colors'>,
): PluginAppearanceColorProjection {
  const { colors } = palette;

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
