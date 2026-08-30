import {
  cssVariables as systemCssVariables,
  type TokenName as SystemTokenName,
} from '@bitfun/design-tokens';
import {
  themeCssVariables,
  type ThemeTokenName,
} from '@bitfun/theme-bitfun';

export type LegacyAppearanceTokenName = `--bf-appearance-token-${string}`;
type DesignSystemCssVariableName = `--bf-${string}`;

export interface AppearanceTokenProjectionEntry {
  source: LegacyAppearanceTokenName;
  target: DesignSystemCssVariableName;
}

const systemTokenSources = {
  'space.1': '--bf-appearance-token-size-gap-1',
  'space.2': '--bf-appearance-token-size-gap-2',
  'space.3': '--bf-appearance-token-size-gap-3',
  'space.4': '--bf-appearance-token-size-gap-4',
  'space.5': '--bf-appearance-token-size-gap-5',
  'space.6': '--bf-appearance-token-size-gap-6',
  'space.8': '--bf-appearance-token-size-gap-8',
  'space.10': '--bf-appearance-token-size-gap-10',
  'space.12': '--bf-appearance-token-size-gap-12',
  'space.16': '--bf-appearance-token-size-gap-16',
  'radius.sm': '--bf-appearance-token-size-radius-sm',
  'radius.base': '--bf-appearance-token-size-radius-base',
  'radius.md': '--bf-appearance-token-size-radius-md',
  'radius.lg': '--bf-appearance-token-size-radius-lg',
  'radius.xl': '--bf-appearance-token-size-radius-xl',
  'radius.2xl': '--bf-appearance-token-size-radius-2xl',
  'radius.pill': '--bf-appearance-token-size-radius-full',
  'motion.duration.instant': '--bf-appearance-token-motion-instant',
  'motion.duration.fast': '--bf-appearance-token-motion-fast',
  'motion.duration.base': '--bf-appearance-token-motion-base',
  'motion.duration.normal': '--bf-appearance-token-motion-base',
  'motion.duration.slow': '--bf-appearance-token-motion-slow',
  'motion.duration.lazy': '--bf-appearance-token-motion-lazy',
  'motion.easing.standard': '--bf-appearance-token-easing-standard',
  'motion.easing.decelerate': '--bf-appearance-token-easing-decelerate',
  'motion.easing.smooth': '--bf-appearance-token-easing-smooth',
  'motion.easing.accelerate': '--bf-appearance-token-easing-accelerate',
  'motion.easing.enter': '--bf-appearance-token-easing-decelerate',
} as const satisfies Partial<Record<SystemTokenName, LegacyAppearanceTokenName>>;

const themeTokenSources = {
  'color.surface.canvas': '--bf-appearance-token-color-bg-primary',
  'color.surface.panel': '--bf-appearance-token-color-bg-secondary',
  'color.surface.raised': '--bf-appearance-token-color-bg-elevated',
  'color.surface.subtle': '--bf-appearance-token-element-bg-subtle',
  'color.content.primary': '--bf-appearance-token-color-text-primary',
  'color.content.secondary': '--bf-appearance-token-color-text-secondary',
  'color.content.muted': '--bf-appearance-token-color-text-muted',
  'color.content.disabled': '--bf-appearance-token-color-text-disabled',
  'color.content.inverse': '--bf-appearance-token-btn-primary-color',
  'color.accent.default': '--bf-appearance-token-color-accent-500',
  'color.accent.hover': '--bf-appearance-token-color-accent-600',
  'color.border.subtle': '--bf-appearance-token-border-subtle',
  'color.border.default': '--bf-appearance-token-border-base',
  'color.border.strong': '--bf-appearance-token-border-strong',
  'color.action.neutral.border': '--bf-appearance-token-border-base',
  'color.action.neutral.content': '--bf-appearance-token-color-text-secondary',
  'color.action.neutral.contentDisabled': '--bf-appearance-token-color-text-disabled',
  'color.action.neutral.fillBorder': '--bf-appearance-token-element-bg-base',
  'color.action.neutral.surface': '--bf-appearance-token-element-bg-base',
  'color.action.neutral.surfaceHover': '--bf-appearance-token-element-bg-medium',
  'color.action.neutral.surfacePressed': '--bf-appearance-token-element-bg-strong',
  'color.action.primary.background': '--bf-appearance-token-btn-primary-bg',
  'color.action.primary.hover': '--bf-appearance-token-btn-primary-hover-bg',
  'color.action.primary.pressed': '--bf-appearance-token-btn-primary-active-bg',
  'color.action.primary.content': '--bf-appearance-token-btn-primary-color',
  'color.action.secondary.background': '--bf-appearance-token-color-accent-100',
  'color.action.secondary.hover': '--bf-appearance-token-color-accent-200',
  'color.action.secondary.pressed': '--bf-appearance-token-color-accent-300',
  'color.action.secondary.content': '--bf-appearance-token-color-accent-600',
  'color.action.quiet.hover': '--bf-appearance-token-element-bg-soft',
  'color.action.quiet.pressed': '--bf-appearance-token-element-bg-base',
  'color.action.quiet.content': '--bf-appearance-token-color-text-secondary',
  'color.field.background': '--bf-appearance-token-color-bg-secondary',
  'color.field.backgroundHover': '--bf-appearance-token-element-bg-subtle',
  'color.field.border': '--bf-appearance-token-border-base',
  'color.field.borderHover': '--bf-appearance-token-border-medium',
  'color.field.borderFocus': '--bf-appearance-token-color-accent-500',
  'color.focus.ring': '--bf-appearance-token-color-accent-500',
  'color.status.info.content': '--bf-appearance-token-color-info',
  'color.status.info.surface': '--bf-appearance-token-color-info-bg',
  'color.status.info.border': '--bf-appearance-token-color-info-border',
  'color.status.success.content': '--bf-appearance-token-color-success',
  'color.status.success.surface': '--bf-appearance-token-color-success-bg',
  'color.status.success.border': '--bf-appearance-token-color-success-border',
  'color.status.warning.content': '--bf-appearance-token-color-warning',
  'color.status.warning.surface': '--bf-appearance-token-color-warning-bg',
  'color.status.warning.border': '--bf-appearance-token-color-warning-border',
  'color.status.danger.content': '--bf-appearance-token-color-error',
  'color.status.danger.surface': '--bf-appearance-token-color-error-bg',
  'color.status.danger.border': '--bf-appearance-token-color-error-border',
  'shadow.xs': '--bf-appearance-token-shadow-xs',
  'shadow.sm': '--bf-appearance-token-shadow-sm',
  'shadow.base': '--bf-appearance-token-shadow-base',
  'shadow.lg': '--bf-appearance-token-shadow-lg',
  'shadow.xl': '--bf-appearance-token-shadow-xl',
  'shadow.raised': '--bf-appearance-token-shadow-sm',
  'shadow.overlay': '--bf-appearance-token-shadow-lg',
  'effect.blur.subtle': '--bf-appearance-token-blur-subtle',
  'effect.blur.base': '--bf-appearance-token-blur-base',
  'effect.blur.medium': '--bf-appearance-token-blur-medium',
  'opacity.disabled': '--bf-appearance-token-opacity-disabled',
  'opacity.hover': '--bf-appearance-token-opacity-hover',
  'opacity.focus': '--bf-appearance-token-opacity-hover',
  'opacity.muted': '--bf-appearance-token-opacity-hover',
} as const satisfies Partial<Record<ThemeTokenName, LegacyAppearanceTokenName>>;

function createProjection<TokenName extends string>(
  sources: Partial<Record<TokenName, LegacyAppearanceTokenName>>,
  targets: Readonly<Record<TokenName, DesignSystemCssVariableName>>,
): AppearanceTokenProjectionEntry[] {
  return Object.entries(sources).map(([name, source]) => ({
    source: source as LegacyAppearanceTokenName,
    target: targets[name as TokenName],
  }));
}

export const DESIGN_SYSTEM_APPEARANCE_TOKEN_PROJECTION = Object.freeze([
  ...createProjection(systemTokenSources, systemCssVariables),
  ...createProjection(themeTokenSources, themeCssVariables),
]);

export const DESIGN_SYSTEM_APPEARANCE_TOKEN_NAMES = Object.freeze(
  DESIGN_SYSTEM_APPEARANCE_TOKEN_PROJECTION.map(({ target }) => target),
);

export function projectDesignSystemAppearanceTokens(
  legacyTokens: Readonly<Record<string, string>>,
): Readonly<Record<DesignSystemCssVariableName, string>> {
  return Object.fromEntries(
    DESIGN_SYSTEM_APPEARANCE_TOKEN_PROJECTION.flatMap(({ source, target }) => {
      const value = legacyTokens[source];
      return value === undefined ? [] : [[target, value]];
    }),
  );
}
