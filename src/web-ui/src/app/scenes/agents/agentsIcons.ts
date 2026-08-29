/**
 * Icon and color mapping for the agents scene
 * Catalog-mapped glyphs render through @bitfun/ui Icon; the rest stay lucide.
 */
import { Icon, type IconName, type IconSize } from '@bitfun/ui';
import {
  Code2,
  FlaskConical,
  Bug,
  FileText,
  BarChart2,
  Server,
  Layers,
  Bot,
  Cpu,
  Microscope,
  type LucideProps,
} from 'lucide-react';
import React from 'react';
export { CAPABILITY_ACCENT } from './agentAppearance';

function catalogSize(size?: number | string): IconSize {
  const n = typeof size === 'number' ? size : 21;
  if (n <= 11) return '2xs';
  if (n <= 13) return 'xs';
  if (n <= 15) return 'sm';
  if (n <= 17) return 'md';
  return 'lg';
}

function catalogIcon(name: IconName): React.FC<LucideProps> {
  return function CatalogIcon({ size }) {
    return React.createElement(Icon, { name, size: catalogSize(size) });
  };
}

export type AgentIconKey =
  | 'code2' | 'eye' | 'flask' | 'bug' | 'filetext'
  | 'globe' | 'barchart' | 'layers' | 'penline' | 'server'
  | 'bot' | 'terminal' | 'microscope' | 'cpu';

export const AGENT_ICON_MAP: Record<AgentIconKey, React.FC<LucideProps>> = {
  code2: Code2,
  eye: catalogIcon('eye'),
  flask: FlaskConical,
  bug: Bug,
  filetext: FileText,
  globe: catalogIcon('browser'),
  barchart: BarChart2,
  layers: Layers,
  penline: catalogIcon('edit'),
  server: Server,
  bot: Bot,
  terminal: catalogIcon('terminal'),
  microscope: Microscope,
  cpu: Cpu,
};
