import { themeCssVariables } from '@bitfun/theme-bitfun';

import {
  APPEARANCE_ROOT_TOKEN_NAMES,
  APPEARANCE_SCOPED_TOKEN_NAMES,
  APPEARANCE_THEME_SCOPE_SELECTORS,
} from '../appearanceTokenContract';
import type {
  AppearanceRendererAdapter,
  AppearanceThemeScopeId,
  AppearanceThemeTokenName,
  ThemeTokenAppearanceSettings,
} from '../types';

const ROOT_ALLOWED_TOKEN_NAMES = new Set<string>(APPEARANCE_ROOT_TOKEN_NAMES);
const SCOPED_ALLOWED_TOKEN_NAMES = new Set<string>(APPEARANCE_SCOPED_TOKEN_NAMES);
const SCOPE_STYLE_ATTRIBUTE = 'data-bf-appearance-theme-scopes';
const ROOT_BACKGROUND_VARIABLE = themeCssVariables['color.surface.chrome'];
const FORBIDDEN_VALUE = /(?:url\s*\(|var\s*\(|expression\s*\(|[;{}<>])/i;

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function isSettings(value: unknown): value is ThemeTokenAppearanceSettings {
  if (!isRecord(value) || !isRecord(value.tokens)) return false;
  if (value.scopes === undefined) return true;
  if (!isRecord(value.scopes)) return false;
  return Object.entries(value.scopes).every(([scopeId, tokens]) => (
    scopeId in APPEARANCE_THEME_SCOPE_SELECTORS && isRecord(tokens)
  ));
}

function validateToken(
  name: string,
  value: unknown,
  allowedNames: ReadonlySet<string>,
  path: string,
): string | null {
  if (!allowedNames.has(name)) return `Unsupported ${path} token name: ${name}`;
  if (typeof value !== 'string' || value.length === 0 || value.length > 512) {
    return `${path} token ${name} must be a non-empty string of at most 512 characters`;
  }
  if (FORBIDDEN_VALUE.test(value)) return `${path} token ${name} contains a forbidden value`;
  return null;
}

function removeAppliedTokens(): void {
  const rootStyle = document.documentElement.style;
  APPEARANCE_ROOT_TOKEN_NAMES.forEach(name => rootStyle.removeProperty(name));
  document.querySelectorAll<HTMLStyleElement>(`style[${SCOPE_STYLE_ATTRIBUTE}]`)
    .forEach(node => node.remove());
}

function renderScopeStyles(settings: Readonly<ThemeTokenAppearanceSettings>): string {
  return Object.entries(settings.scopes ?? {}).flatMap(([scopeId, tokens]) => {
    if (!tokens) return [];
    const selector = APPEARANCE_THEME_SCOPE_SELECTORS[scopeId as AppearanceThemeScopeId];
    const declarations = Object.entries(tokens)
      .map(([name, value]) => `${name}:${value};`)
      .join('');
    return declarations ? [`${selector}{${declarations}}`] : [];
  }).join('\n');
}

export function getThemeAppearanceTokenValue(
  settings: Readonly<ThemeTokenAppearanceSettings> | undefined,
  name: AppearanceThemeTokenName,
  scope?: AppearanceThemeScopeId,
): string | undefined {
  return scope ? settings?.scopes?.[scope]?.[name] : settings?.tokens[name];
}

export const themeTokenAppearanceAdapter: AppearanceRendererAdapter<'theme-tokens'> = {
  id: 'theme-tokens',
  validate(settings) {
    if (!isSettings(settings)) return ['theme-tokens settings must contain canonical tokens'];
    const errors = Object.entries(settings.tokens)
      .map(([name, value]) => validateToken(name, value, ROOT_ALLOWED_TOKEN_NAMES, 'root'))
      .filter((error): error is string => error !== null);
    Object.entries(settings.scopes ?? {}).forEach(([scopeId, tokens]) => {
      Object.entries(tokens ?? {}).forEach(([name, value]) => {
        const error = validateToken(name, value, SCOPED_ALLOWED_TOKEN_NAMES, `scope ${scopeId}`);
        if (error) errors.push(error);
      });
    });
    return errors;
  },
  apply(next) {
    removeAppliedTokens();
    const rootStyle = document.documentElement.style;
    if (!isSettings(next)) {
      rootStyle.backgroundColor = '';
      if (document.body) document.body.style.backgroundColor = '';
      return;
    }
    Object.entries(next.tokens).forEach(([name, value]) => {
      if (value !== undefined) rootStyle.setProperty(name, value);
    });
    const scopeCss = renderScopeStyles(next);
    if (scopeCss) {
      const style = document.createElement('style');
      style.setAttribute(SCOPE_STYLE_ATTRIBUTE, 'true');
      style.textContent = scopeCss;
      document.head.appendChild(style);
    }
    const background = `var(${ROOT_BACKGROUND_VARIABLE})`;
    rootStyle.backgroundColor = background;
    if (document.body) document.body.style.backgroundColor = background;
  },
};
