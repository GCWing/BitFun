/**
 * Icon and color mapping for the agents scene
 * All visuals use lucide-react icons + CSS custom properties.
 */
import {
  Code2,
  FlaskConical,
  Bug,
  FileText,
  Globe,
  BarChart2,
  PenLine,
  Server,
  Eye,
  Layers,
  Bot,
  Cpu,
  Terminal,
  Microscope,
  type LucideProps,
} from 'lucide-react';
import type React from 'react';
export { CAPABILITY_ACCENT } from './agentTheme';

export type AgentIconKey =
  | 'code2' | 'eye' | 'flask' | 'bug' | 'filetext'
  | 'globe' | 'barchart' | 'layers' | 'penline' | 'server'
  | 'bot' | 'terminal' | 'microscope' | 'cpu';

export const AGENT_ICON_MAP: Record<AgentIconKey, React.FC<LucideProps>> = {
  code2: Code2,
  eye: Eye,
  flask: FlaskConical,
  bug: Bug,
  filetext: FileText,
  globe: Globe,
  barchart: BarChart2,
  layers: Layers,
  penline: PenLine,
  server: Server,
  bot: Bot,
  terminal: Terminal,
  microscope: Microscope,
  cpu: Cpu,
};
