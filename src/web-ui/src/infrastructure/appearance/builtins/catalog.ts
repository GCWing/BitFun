import {
  builtinAppearancePalettes,
  DEFAULT_DARK_APPEARANCE_ID,
  DEFAULT_LIGHT_APPEARANCE_ID,
} from './palettes';
import type {
  AppearanceCatalogEntry,
  AppearancePackage,
  AppearanceThemeTokenName,
} from '../types';
import { buildBuiltinAppearance } from './buildBuiltinAppearance';
export { WIDGET_APPEARANCE_VARIABLE_NAMES } from '../adapters/widgetAppearanceVariables';

export { DEFAULT_DARK_APPEARANCE_ID, DEFAULT_LIGHT_APPEARANCE_ID } from './palettes';

export const builtinAppearancePackages: readonly AppearancePackage[] = Object.freeze(
  builtinAppearancePalettes.map(palette => Object.freeze(buildBuiltinAppearance(palette))),
);

const builtinById = new Map(builtinAppearancePackages.map(pkg => [pkg.id, pkg]));

function collectRendererVariableNames(
  rendererId: 'theme-tokens' | 'generative-widget',
): readonly string[] {
  const names = new Set<string>();
  builtinAppearancePackages.forEach(pkg => {
    const values = rendererId === 'theme-tokens'
      ? pkg.renderers?.['theme-tokens']?.settings.tokens
      : pkg.renderers?.['generative-widget']?.settings.vars;
    if (values && typeof values === 'object' && !Array.isArray(values)) {
      Object.keys(values).forEach(name => names.add(name));
    }
  });
  return Object.freeze([...names].sort());
}

export const APPEARANCE_THEME_TOKEN_NAMES = collectRendererVariableNames('theme-tokens');

export const builtinAppearanceCatalog: readonly AppearanceCatalogEntry[] = Object.freeze(
  builtinAppearancePackages.map(pkg => Object.freeze({
    id: pkg.id,
    name: pkg.name,
    author: pkg.author,
    description: pkg.description,
    version: pkg.version,
    mode: pkg.mode,
    source: 'builtin' as const,
  })),
);

export function getBuiltinAppearance(id: string): AppearancePackage | null {
  return builtinById.get(id) ?? null;
}

export function getBuiltinAppearanceThemeTokens(
  id: string = DEFAULT_DARK_APPEARANCE_ID,
): Readonly<Partial<Record<AppearanceThemeTokenName, string>>> {
  const settings = getBuiltinAppearance(id)?.renderers?.['theme-tokens']?.settings;
  const tokens = settings?.tokens;
  if (!tokens || typeof tokens !== 'object' || Array.isArray(tokens)) {
    throw new Error(`Builtin Appearance ${id} has no theme-tokens renderer settings`);
  }
  return tokens;
}

export function getBuiltinAppearanceThemeToken(
  name: AppearanceThemeTokenName,
  id: string = DEFAULT_DARK_APPEARANCE_ID,
): string {
  const value = getBuiltinAppearanceThemeTokens(id)[name];
  if (!value) throw new Error(`Builtin Appearance ${id} does not define ${name}`);
  return value;
}

export function getSystemAppearanceId(): string {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return DEFAULT_LIGHT_APPEARANCE_ID;
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches
    ? DEFAULT_DARK_APPEARANCE_ID
    : DEFAULT_LIGHT_APPEARANCE_ID;
}
