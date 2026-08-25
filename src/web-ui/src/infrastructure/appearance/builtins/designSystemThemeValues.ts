import {
  themes,
  type ThemeTokenName,
} from '@bitfun/theme-bitfun';

type BuiltinThemeMode = 'dark' | 'light';

export function getDesignSystemThemeString(
  mode: BuiltinThemeMode,
  name: ThemeTokenName,
): string {
  const value = themes[mode][name];
  if (typeof value !== 'string') {
    throw new Error(`Design-system theme token ${name} must be a string`);
  }
  return value;
}

export function getDesignSystemThemeNumber(
  mode: BuiltinThemeMode,
  name: ThemeTokenName,
): number {
  const value = themes[mode][name];
  if (typeof value !== 'number') {
    throw new Error(`Design-system theme token ${name} must be a number`);
  }
  return value;
}
