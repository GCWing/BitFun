import type { AppearanceRendererAdapter, CssTokenAppearanceSettings } from '../types';
import { APPEARANCE_CSS_TOKEN_NAMES } from '../builtins/catalog';
import {
  DESIGN_SYSTEM_APPEARANCE_TOKEN_NAMES,
  projectDesignSystemAppearanceTokens,
} from '@/infrastructure/design-system/appearanceTokenProjection';

const ALLOWED_TOKEN_NAMES = new Set(APPEARANCE_CSS_TOKEN_NAMES);

const FORBIDDEN_VALUE = /(?:url\s*\(|var\s*\(|expression\s*\(|[;{}<>])/i;

function isSettings(value: unknown): value is CssTokenAppearanceSettings {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return Boolean(
    record.tokens
      && typeof record.tokens === 'object'
      && !Array.isArray(record.tokens)
      && typeof record.background === 'string',
  );
}

function validateToken(name: string, value: unknown): string | null {
  if (!ALLOWED_TOKEN_NAMES.has(name)) {
    return `Unsupported CSS token name: ${name}`;
  }
  if (typeof value !== 'string' || value.length === 0 || value.length > 512) {
    return `CSS token ${name} must be a non-empty string of at most 512 characters`;
  }
  if (FORBIDDEN_VALUE.test(value)) {
    return `CSS token ${name} contains a forbidden value`;
  }
  return null;
}

function removeTokens(settings: Readonly<CssTokenAppearanceSettings> | undefined): void {
  const rootStyle = document.documentElement.style;
  if (isSettings(settings)) {
    Object.keys(settings.tokens).forEach(name => rootStyle.removeProperty(name));
  }
  DESIGN_SYSTEM_APPEARANCE_TOKEN_NAMES.forEach(name => rootStyle.removeProperty(name));
}

export const cssTokenAppearanceAdapter: AppearanceRendererAdapter<'css-tokens'> = {
  id: 'css-tokens',
  validate(settings) {
    if (!isSettings(settings)) return ['css-tokens settings must contain tokens and background'];
    const errors = Object.entries(settings.tokens)
      .map(([name, value]) => validateToken(name, value))
      .filter((error): error is string => error !== null);
    if (FORBIDDEN_VALUE.test(settings.background)) {
      errors.push('css-tokens background contains a forbidden value');
    }
    return errors;
  },
  apply(next, previous) {
    removeTokens(previous);
    if (!isSettings(next)) {
      document.documentElement.style.backgroundColor = '';
      if (document.body) document.body.style.backgroundColor = '';
      return;
    }
    const rootStyle = document.documentElement.style;
    Object.entries(next.tokens).forEach(([name, value]) => rootStyle.setProperty(name, value));
    Object.entries(projectDesignSystemAppearanceTokens(next.tokens))
      .forEach(([name, value]) => rootStyle.setProperty(name, value));
    rootStyle.backgroundColor = next.background;
    if (document.body) document.body.style.backgroundColor = next.background;
  },
};
