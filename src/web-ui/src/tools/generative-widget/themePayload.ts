export type WidgetThemePayload = {
  id: string;
  type: string;
  vars: Record<string, string>;
};

const THEME_VAR_NAMES = [
  '--color-bg-primary',
  '--color-bg-secondary',
  '--color-bg-tertiary',
  '--color-bg-elevated',
  '--color-bg-workbench',
  '--color-bg-scene',
  '--color-bg-tooltip',
  '--color-text-primary',
  '--color-text-secondary',
  '--color-text-muted',
  '--color-text-disabled',
  '--color-accent-50',
  '--color-accent-100',
  '--color-accent-200',
  '--color-accent-300',
  '--color-accent-400',
  '--color-accent-500',
  '--color-accent-600',
  '--color-primary',
  '--color-primary-hover',
  '--color-success',
  '--color-success-bg',
  '--color-warning',
  '--color-warning-bg',
  '--color-error',
  '--color-error-bg',
  '--color-info',
  '--color-info-bg',
  '--border-subtle',
  '--border-base',
  '--border-medium',
  '--border-strong',
  '--border-prominent',
  '--element-bg-subtle',
  '--element-bg-soft',
  '--element-bg-base',
  '--element-bg-medium',
  '--element-bg-strong',
  '--element-bg-elevated',
  '--shadow-xs',
  '--shadow-sm',
  '--shadow-base',
  '--shadow-lg',
  '--shadow-xl',
  '--radius-sm',
  '--radius-base',
  '--radius-lg',
  '--radius-xl',
  '--spacing-2',
  '--spacing-3',
  '--spacing-4',
  '--spacing-6',
  '--motion-fast',
  '--motion-base',
  '--easing-standard',
  '--font-sans',
  '--font-mono',
] as const;

export function readWidgetThemePayload(): WidgetThemePayload | null {
  if (typeof window === 'undefined' || typeof document === 'undefined') {
    return null;
  }

  const root = document.documentElement;
  const styles = window.getComputedStyle(root);
  const vars: Record<string, string> = {};

  for (const name of THEME_VAR_NAMES) {
    const value = styles.getPropertyValue(name).trim();
    if (value) {
      vars[name] = value;
    }
  }

  return {
    id: root.getAttribute('data-theme') || 'unknown',
    type: root.getAttribute('data-theme-type') || 'dark',
    vars,
  };
}
