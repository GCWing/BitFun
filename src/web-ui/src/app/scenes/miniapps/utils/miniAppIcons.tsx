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
  'var(--miniapp-icon-gradient-0)',
  'var(--miniapp-icon-gradient-1)',
  'var(--miniapp-icon-gradient-2)',
  'var(--miniapp-icon-gradient-3)',
  'var(--miniapp-icon-gradient-4)',
  'var(--miniapp-icon-gradient-5)',
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
