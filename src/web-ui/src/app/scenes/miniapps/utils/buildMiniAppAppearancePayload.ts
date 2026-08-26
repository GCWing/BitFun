import type { AppearanceMode, ResolvedAppearance } from '@/infrastructure/appearance';

export interface MiniAppAppearancePayload {
  mode: AppearanceMode;
  id: string;
  vars: Record<string, string>;
}

const TOKEN_MAP: Readonly<Record<string, string>> = {
  '--bitfun-bg': '--bf-appearance-token-color-bg-primary',
  '--bitfun-bg-secondary': '--bf-appearance-token-color-bg-secondary',
  '--bitfun-bg-tertiary': '--bf-appearance-token-color-bg-tertiary',
  '--bitfun-bg-elevated': '--bf-appearance-token-color-bg-elevated',
  '--bitfun-text': '--bf-appearance-token-color-text-primary',
  '--bitfun-text-secondary': '--bf-appearance-token-color-text-secondary',
  '--bitfun-text-muted': '--bf-appearance-token-color-text-muted',
  '--bitfun-accent': '--bf-appearance-token-color-accent-500',
  '--bitfun-accent-hover': '--bf-appearance-token-color-accent-600',
  '--bitfun-success': '--bf-appearance-token-color-success',
  '--bitfun-warning': '--bf-appearance-token-color-warning',
  '--bitfun-error': '--bf-appearance-token-color-error',
  '--bitfun-info': '--bf-appearance-token-color-info',
  '--bitfun-border': '--bf-appearance-token-border-base',
  '--bitfun-border-subtle': '--bf-appearance-token-border-subtle',
  '--bitfun-element-bg': '--bf-appearance-token-element-bg-base',
  '--bitfun-element-hover': '--bf-appearance-token-element-bg-medium',
  '--bitfun-radius': '--bf-appearance-token-size-radius-base',
  '--bitfun-radius-lg': '--bf-appearance-token-size-radius-lg',
  '--bitfun-font-sans': '--bf-appearance-token-font-family-sans',
  '--bitfun-font-mono': '--bf-appearance-token-font-family-mono',
  '--bitfun-scrollbar-thumb': '--bf-appearance-token-scrollbar-thumb',
  '--bitfun-scrollbar-thumb-hover': '--bf-appearance-token-scrollbar-thumb-hover',
};

function getCssTokens(appearance: ResolvedAppearance): Record<string, string> | null {
  const settings = appearance.renderers['css-tokens'];
  const tokens = settings?.tokens;
  if (!tokens || typeof tokens !== 'object' || Array.isArray(tokens)) return null;
  return tokens as Record<string, string>;
}

function rootAppearanceMode(): AppearanceMode {
  if (typeof document === 'undefined') return 'dark';
  const mode = document.documentElement.getAttribute('data-bf-appearance-mode');
  return mode === 'light' || mode === 'dark' ? mode : 'dark';
}

function rootAppearanceId(): string {
  if (typeof document === 'undefined') return 'host';
  return document.documentElement.getAttribute('data-bf-appearance') || 'host';
}

function rootCssTokens(): Record<string, string> | null {
  if (typeof window === 'undefined' || typeof document === 'undefined') return null;
  const computed = window.getComputedStyle(document.documentElement);
  const tokens: Record<string, string> = {};
  Object.values(TOKEN_MAP).forEach((source) => {
    const value = computed.getPropertyValue(source).trim();
    if (value) tokens[source] = value;
  });
  return tokens;
}

export function resolveMiniAppAppearanceMode(
  appearance: ResolvedAppearance | null,
): AppearanceMode {
  return appearance?.mode ?? rootAppearanceMode();
}

export function buildMiniAppAppearancePayload(
  appearance: ResolvedAppearance | null,
): MiniAppAppearancePayload | null {
  const tokens = appearance ? getCssTokens(appearance) : rootCssTokens();
  if (!tokens && !appearance) return null;
  const vars: Record<string, string> = {};
  Object.entries(TOKEN_MAP).forEach(([target, source]) => {
    const value = tokens?.[source];
    if (typeof value === 'string') vars[target] = value;
  });
  return {
    mode: resolveMiniAppAppearanceMode(appearance),
    id: appearance?.id ?? rootAppearanceId(),
    vars,
  };
}
