/**
 * Build Live App theme payload from main app ThemeConfig.
 * Maps to --bitfun-* CSS variables for iframe theme sync.
 */
import type { ThemeConfig, ThemeType } from '@/infrastructure/theme/types';

export interface LiveAppThemePayload {
  type: ThemeType;
  id: string;
  vars: Record<string, string>;
}

export function buildLiveAppThemeVars(theme: ThemeConfig | null): LiveAppThemePayload | null {
  if (!theme) return null;

  const { colors, effects, typography } = theme;
  const vars: Record<string, string> = {};

  vars['--bitfun-bg'] = colors.background.primary;
  vars['--bitfun-bg-secondary'] = colors.background.secondary;
  vars['--bitfun-bg-tertiary'] = colors.background.tertiary;
  vars['--bitfun-bg-elevated'] = colors.background.elevated;

  vars['--bitfun-text'] = colors.text.primary;
  vars['--bitfun-text-secondary'] = colors.text.secondary;
  vars['--bitfun-text-muted'] = colors.text.muted;

  vars['--bitfun-accent'] = colors.accent[500];
  vars['--bitfun-accent-hover'] = colors.accent[600];

  vars['--bitfun-success'] = colors.semantic.success;
  vars['--bitfun-warning'] = colors.semantic.warning;
  vars['--bitfun-error'] = colors.semantic.error;
  vars['--bitfun-info'] = colors.semantic.info;

  vars['--bitfun-border'] = colors.border.base;
  vars['--bitfun-border-subtle'] = colors.border.subtle;

  vars['--bitfun-element-bg'] = colors.element.base;
  vars['--bitfun-element-hover'] = colors.element.medium;

  if (effects?.radius) {
    vars['--bitfun-radius'] = effects.radius.base;
    vars['--bitfun-radius-lg'] = effects.radius.lg;
  }

  if (typography?.font) {
    vars['--bitfun-font-sans'] = typography.font.sans;
    vars['--bitfun-font-mono'] = typography.font.mono;
  }

  if (colors.scrollbar) {
    vars['--bitfun-scrollbar-thumb'] = colors.scrollbar.thumb;
    vars['--bitfun-scrollbar-thumb-hover'] = colors.scrollbar.thumbHover;
  } else {
    vars['--bitfun-scrollbar-thumb'] =
      theme.type === 'dark' ? 'rgba(255, 255, 255, 0.12)' : 'rgba(0, 0, 0, 0.15)';
    vars['--bitfun-scrollbar-thumb-hover'] =
      theme.type === 'dark' ? 'rgba(255, 255, 255, 0.22)' : 'rgba(0, 0, 0, 0.28)';
  }

  return {
    type: theme.type,
    id: theme.id,
    vars,
  };
}
