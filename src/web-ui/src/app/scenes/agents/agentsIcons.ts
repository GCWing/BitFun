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
import type { CapabilityCategory } from './agentsStore';

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

export const CAPABILITY_ACCENT: Record<CapabilityCategory, string> = {
  coding: '#60a5fa',
  docs: '#6eb88c',
  analysis: '#8b5cf6',
  testing: '#c9944d',
  creative: '#e879a0',
  ops: '#5ea3a3',
};
