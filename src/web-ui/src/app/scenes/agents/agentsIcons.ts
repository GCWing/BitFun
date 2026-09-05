/**
 * Icon and color mapping for the agents scene
 * Shared icon sources for agent identity. Rendering stays owned by @openbitfun/ui Icon.
 */
import type { IconSource } from '@openbitfun/ui';
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
} from 'lucide-react';
export { CAPABILITY_ACCENT } from './agentAppearance';

export type AgentIconKey =
  | 'code2' | 'eye' | 'flask' | 'bug' | 'filetext'
  | 'globe' | 'barchart' | 'layers' | 'penline' | 'server'
  | 'bot' | 'terminal' | 'microscope' | 'cpu';

export const AGENT_ICON_MAP: Record<AgentIconKey, IconSource> = {
  code2: { glyph: Code2 },
  eye: { name: 'eye' },
  flask: { glyph: FlaskConical },
  bug: { glyph: Bug },
  filetext: { glyph: FileText },
  globe: { name: 'browser' },
  barchart: { glyph: BarChart2 },
  layers: { glyph: Layers },
  penline: { name: 'edit' },
  server: { glyph: Server },
  bot: { glyph: Bot },
  terminal: { name: 'terminal' },
  microscope: { glyph: Microscope },
  cpu: { glyph: Cpu },
};
