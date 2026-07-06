import React from 'react';
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
  Image,
  LayoutGrid,
  Presentation,
  Regex,
  Rocket,
  Settings,
  Sparkles,
  Terminal,
  Workflow,
  Wrench,
  type LucideIcon,
} from 'lucide-react';

const ICON_GRADIENTS = [
  'linear-gradient(135deg, color-mix(in srgb, var(--color-accent-600) 35%, transparent) 0%, color-mix(in srgb, var(--color-purple-500) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--color-success) 30%, transparent) 0%, color-mix(in srgb, var(--color-accent-600) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--color-warning) 30%, transparent) 0%, color-mix(in srgb, var(--color-error) 20%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--color-purple-500) 35%, transparent) 0%, color-mix(in srgb, var(--color-error) 20%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--color-cyan-500) 30%, transparent) 0%, color-mix(in srgb, var(--color-accent-600) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--color-error) 25%, transparent) 0%, color-mix(in srgb, var(--color-warning) 20%, transparent) 100%)',
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
  Image,
  LayoutGrid,
  Presentation,
  Regex,
  Rocket,
  Settings,
  Sparkles,
  Terminal,
  Workflow,
  Wrench,
} satisfies Record<string, LucideIcon>;

export function renderMiniAppIcon(name: string, size = 28): React.ReactNode {
  const key = name
    .split('-')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('') as keyof typeof MINI_APP_ICONS;
  const Icon = MINI_APP_ICONS[key];

  return Icon
    ? <Icon size={size} strokeWidth={1.5} />
    : <Box size={size} strokeWidth={1.5} />;
}

export function getMiniAppIconGradient(icon: string): string {
  const idx = (icon.charCodeAt(0) || 0) % ICON_GRADIENTS.length;
  return ICON_GRADIENTS[idx];
}
