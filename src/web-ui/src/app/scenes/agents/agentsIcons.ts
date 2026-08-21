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
  LayoutTemplate,
  Rocket,
  Users,
  Briefcase,
  type LucideProps,
} from 'lucide-react';
import type React from 'react';
import { APPEARANCE_DOMAIN_TOKENS } from '@/infrastructure/appearance/appearanceDomainTokens';
export { CAPABILITY_ACCENT } from './agentAppearance';

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

export type AgentTeamIconKey =
  | 'code' | 'chart' | 'layout' | 'rocket'
  | 'users' | 'briefcase' | 'layers';

export const AGENT_TEAM_ICON_MAP: Record<AgentTeamIconKey, React.FC<LucideProps>> = {
  code: Code2,
  chart: BarChart2,
  layout: LayoutTemplate,
  rocket: Rocket,
  users: Users,
  briefcase: Briefcase,
  layers: Layers,
};

// Each agent team has a deterministic accent derived from its id.
const AGENT_TEAM_ACCENTS = APPEARANCE_DOMAIN_TOKENS.agentTeam.accents;

export function getAgentTeamAccent(id: string): string {
  let hash = 0;
  for (let i = 0; i < id.length; i++) hash = (hash * 31 + id.charCodeAt(i)) >>> 0;
  return AGENT_TEAM_ACCENTS[hash % AGENT_TEAM_ACCENTS.length];
}
