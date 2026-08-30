import {
  cssVariables,
  tokens,
  type TokenName,
} from '@bitfun/design-tokens';

export type FontFamilyTokenName = Extract<TokenName, `font.family.${string}`>;
export type FontSizeTokenName = Extract<TokenName, `font.size.${string}`>;
export type FontWeightTokenName = Extract<TokenName, `font.weight.${string}`>;
export type LineHeightTokenName = Extract<TokenName, `lineHeight.${string}`>;
export type LetterSpacingTokenName = Extract<TokenName, `letterSpacing.${string}`>;
export type TypographyTokenName =
  | FontFamilyTokenName
  | FontSizeTokenName
  | FontWeightTokenName
  | LineHeightTokenName
  | LetterSpacingTokenName;

function canonicalValue(name: TypographyTokenName): string {
  return String(tokens[name]);
}

function requireFiniteNumber(name: TypographyTokenName, value: string): number {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed)) {
    throw new TypeError(`Typography token ${name} must resolve to a finite number.`);
  }
  return parsed;
}

/** Canonical value for APIs that cannot consume CSS custom properties. */
export function getTypographyTokenValue(name: TypographyTokenName): string {
  return canonicalValue(name);
}

/** Canonical pixel value for Monaco, xterm, Mermaid, canvas, and similar APIs. */
export function getTypographyTokenPx(name: FontSizeTokenName): number {
  const value = canonicalValue(name);
  if (!value.endsWith('px')) {
    throw new TypeError(`Typography token ${name} must resolve to pixels.`);
  }
  return requireFiniteNumber(name, value);
}

/** Canonical unitless value for renderer options such as line height and weight. */
export function getTypographyTokenNumber(
  name: FontWeightTokenName | LineHeightTokenName,
): number {
  return requireFiniteNumber(name, canonicalValue(name));
}

/**
 * Reads the active root override when a renderer is expected to track the
 * global font preference, and falls back to the generated canonical token.
 */
export function readActiveTypographyTokenValue(name: TypographyTokenName): string {
  if (typeof document !== 'undefined' && typeof getComputedStyle === 'function') {
    const value = getComputedStyle(document.documentElement)
      .getPropertyValue(cssVariables[name])
      .trim();
    if (value) return value;
  }
  return canonicalValue(name);
}

export function readActiveTypographyTokenPx(name: FontSizeTokenName): number {
  const value = readActiveTypographyTokenValue(name);
  if (!value.endsWith('px')) {
    throw new TypeError(`Typography token ${name} must resolve to pixels.`);
  }
  return requireFiniteNumber(name, value);
}
