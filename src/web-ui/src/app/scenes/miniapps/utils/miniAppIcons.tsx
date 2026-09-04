import React from 'react';
import { Icon, type IconName, type IconSize } from '@openbitfun/ui';
import codingFootprintIcon from '@/assets/miniapps/catalog/coding-footprint.png';
import dailyDivinationIcon from '@/assets/miniapps/catalog/daily-divination.png';
import gomokuIcon from '@/assets/miniapps/catalog/gomoku.png';
import pptLiveIcon from '@/assets/miniapps/catalog/ppt-live.png';
import regexPlaygroundIcon from '@/assets/miniapps/catalog/regex-playground.png';
import codingFootprintShowcase from '@/assets/miniapps/showcases/coding-footprint.webp';
import dailyDivinationShowcase from '@/assets/miniapps/showcases/daily-divination.webp';
import gomokuShowcase from '@/assets/miniapps/showcases/gomoku.webp';
import pptLiveShowcase from '@/assets/miniapps/showcases/ppt-live.webp';
import regexPlaygroundShowcase from '@/assets/miniapps/showcases/regex-playground.webp';
import {
  Aperture,
  Box,
  Bot,
  Code,
  Database,
  FileText,
  GitPullRequest,
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
  'linear-gradient(135deg, color-mix(in srgb, var(--openbitfun-color-accent-hover) 35%, transparent) 0%, color-mix(in srgb, var(--openbitfun-color-accent-secondary) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--openbitfun-color-status-success-content) 30%, transparent) 0%, color-mix(in srgb, var(--openbitfun-color-accent-hover) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--openbitfun-color-status-warning-content) 30%, transparent) 0%, color-mix(in srgb, var(--openbitfun-color-status-danger-content) 20%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--openbitfun-color-accent-secondary) 35%, transparent) 0%, color-mix(in srgb, var(--openbitfun-color-status-danger-content) 20%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--openbitfun-domain-generative-ui) 30%, transparent) 0%, color-mix(in srgb, var(--openbitfun-color-accent-hover) 25%, transparent) 100%)',
  'linear-gradient(135deg, color-mix(in srgb, var(--openbitfun-color-status-danger-content) 25%, transparent) 0%, color-mix(in srgb, var(--openbitfun-color-status-warning-content) 20%, transparent) 100%)',
];

const MINI_APP_ICONS = {
  Aperture,
  Box,
  Bot,
  Code,
  Database,
  FileText,
  GitPullRequest,
  Grid3x3,
  LayoutGrid,
  Presentation,
  Regex,
  Rocket,
  Workflow,
  Wrench,
} satisfies Record<string, LucideIcon>;

const BUILTIN_MINI_APP_ICON_ASSETS: Readonly<Record<string, string>> = {
  'builtin-coding-selfie': codingFootprintIcon,
  'builtin-daily-divination': dailyDivinationIcon,
  'builtin-gomoku': gomokuIcon,
  'builtin-ppt-live': pptLiveIcon,
  'builtin-regex-playground': regexPlaygroundIcon,
};

const BUILTIN_MINI_APP_SHOWCASE_ASSETS: Readonly<Record<string, string>> = {
  'builtin-coding-selfie': codingFootprintShowcase,
  'builtin-daily-divination': dailyDivinationShowcase,
  'builtin-gomoku': gomokuShowcase,
  'builtin-ppt-live': pptLiveShowcase,
  'builtin-regex-playground': regexPlaygroundShowcase,
};

export function getMiniAppIconAsset(id: string): string | undefined {
  return BUILTIN_MINI_APP_ICON_ASSETS[id];
}

export function getMiniAppShowcaseAsset(id: string): string | undefined {
  return BUILTIN_MINI_APP_SHOWCASE_ASSETS[id];
}

export function renderMiniAppIcon(name: string, size = 28): React.ReactNode {
  const key = name
    .split('-')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('') as keyof typeof MINI_APP_ICONS;
  const catalogName = MINI_APP_CATALOG[key];
  if (catalogName) {
    return (
      <Icon
        name={catalogName}
        size={catalogSize(size)}
        style={{ width: size, height: size }}
      />
    );
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
