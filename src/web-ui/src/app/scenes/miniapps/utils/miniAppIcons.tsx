import React from 'react';
import {
  AppWindow,
  Box,
  Bot,
  Code,
  Database,
  FileText,
  Globe,
  Image,
  LayoutGrid,
  Rocket,
  Settings,
  Sparkles,
  Terminal,
  Workflow,
  Wrench,
  type LucideIcon,
} from 'lucide-react';

const ICON_GRADIENTS = [
  'linear-gradient(135deg, rgba(59, 130, 246, 0.35) 0%, rgba(139, 92, 246, 0.25) 100%)',
  'linear-gradient(135deg, rgba(16, 185, 129, 0.3) 0%, rgba(59, 130, 246, 0.25) 100%)',
  'linear-gradient(135deg, rgba(245, 158, 11, 0.3) 0%, rgba(239, 68, 68, 0.2) 100%)',
  'linear-gradient(135deg, rgba(139, 92, 246, 0.35) 0%, rgba(236, 72, 153, 0.2) 100%)',
  'linear-gradient(135deg, rgba(6, 182, 212, 0.3) 0%, rgba(59, 130, 246, 0.25) 100%)',
  'linear-gradient(135deg, rgba(239, 68, 68, 0.25) 0%, rgba(245, 158, 11, 0.2) 100%)',
];

const MINI_APP_ICONS = {
  AppWindow,
  Box,
  Bot,
  Code,
  Database,
  FileText,
  Globe,
  Image,
  LayoutGrid,
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
