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

// Keep this allowlist aligned with the native MiniApp gallery. Marketplace
// metadata stores a Lucide icon identifier (for example, "Aperture"), not
// display text. Unknown identifiers deliberately fall back to Box so untrusted
// metadata can never become oversized text inside the icon slot.
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

function normalizeIconName(name: string | null | undefined): string {
  return (name || 'Box')
    .trim()
    .split('-')
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('');
}

export function resolveMiniAppIcon(name: string | null | undefined): LucideIcon {
  const key = normalizeIconName(name) as keyof typeof MINI_APP_ICONS;
  return MINI_APP_ICONS[key] || Box;
}

export function MiniAppIcon({ name }: { name: string | null | undefined }) {
  const Icon = resolveMiniAppIcon(name);
  return <Icon size={24} strokeWidth={1.5} aria-hidden="true" focusable="false" />;
}
