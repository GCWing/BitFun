import React from 'react';
import { Icon, type IconName, type IconSize } from '@bitfun/ui';
import {
  Aperture,
  AppWindow,
  Box,
  Bot,
  Code,
  Database,
  FileText,
  GitPullRequest,
  Globe,
  Grid3x3,
  LayoutGrid,
  Presentation,
  Regex,
  Rocket,
  Workflow,
  Wrench,
  type LucideIcon,
} from 'lucide-react';

const MINI_APP_CATALOG: Record<string, IconName> = {
  AppWindow: 'floating-window',
  Globe: 'browser',
  Image: 'image',
  Settings: 'settings',
  Sparkles: 'spark',
  Terminal: 'terminal',
};

function catalogSize(size: number): IconSize {
  if (size <= 11) return '2xs';
  if (size <= 13) return 'xs';
  if (size <= 15) return 'sm';
  if (size <= 17) return 'md';
  return 'lg';
}

const ICON_GRADIENTS = [
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-appearance-token-color-accent-600) 35%, transparent) 0%, color-mix(in srgb, var(--bf-appearance-token-color-purple-500) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-appearance-token-color-success) 30%, transparent) 0%, color-mix(in srgb, var(--bf-appearance-token-color-accent-600) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-appearance-token-color-warning) 30%, transparent) 0%, color-mix(in srgb, var(--bf-appearance-token-color-error) 20%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-appearance-token-color-purple-500) 35%, transparent) 0%, color-mix(in srgb, var(--bf-appearance-token-color-error) 20%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-appearance-token-color-cyan-500) 30%, transparent) 0%, color-mix(in srgb, var(--bf-appearance-token-color-accent-600) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--bf-appearance-token-color-error) 25%, transparent) 0%, color-mix(in srgb, var(--bf-appearance-token-color-warning) 20%, transparent) 100%)',
];

const MINI_APP_ICONS = {
  Aperture,
  AppWindow,
  Box,
  Bot,
  Code,
  Database,
  FileText,
  GitPullRequest,
  Globe,
  Grid3x3,
  LayoutGrid,
  Presentation,
  Regex,
  Rocket,
  Workflow,
  Wrench,
} satisfies Record<string, LucideIcon>;

export function renderMiniAppIcon(name: string, size = 28): React.ReactNode {
  const key = name
    .split('-')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('') as keyof typeof MINI_APP_ICONS;
  const catalogName = MINI_APP_CATALOG[key];
  if (catalogName) {
    return <Icon name={catalogName} size={catalogSize(size)} />;
  }
  const LucideGlyph = MINI_APP_ICONS[key];

  return LucideGlyph
    ? <LucideGlyph size={size} strokeWidth={1.5} />
    : <Box size={size} strokeWidth={1.5} />;
}

export function getMiniAppIconGradient(icon: string): string {
  const idx = (icon.charCodeAt(0) || 0) % ICON_GRADIENTS.length;
  return ICON_GRADIENTS[idx];
}
