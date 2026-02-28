/**
 * Icon and color mapping for the team scene
 * All visuals use lucide-react icons + CSS custom properties — no emoji
 */
import {
  Code2,
  BarChart2,
  LayoutTemplate,
  Rocket,
  FlaskConical,
  Bug,
  FileText,
  Globe,
  PenLine,
  Server,
  Eye,
  Layers,
  Bot,
  Users,
  Briefcase,
  Cpu,
  Terminal,
  Microscope,
  type LucideProps,
} from 'lucide-react';
import type React from 'react';

export type AgentIconKey =
  | 'code2' | 'eye' | 'flask' | 'bug' | 'filetext'
  | 'globe' | 'barchart' | 'layers' | 'penline' | 'server'
  | 'bot' | 'terminal' | 'microscope' | 'cpu';

export type TeamIconKey =
  | 'code' | 'chart' | 'layout' | 'rocket'
  | 'users' | 'briefcase' | 'layers';

export const AGENT_ICON_MAP: Record<AgentIconKey, React.FC<LucideProps>> = {
  code2:       Code2,
  eye:         Eye,
  flask:       FlaskConical,
  bug:         Bug,
  filetext:    FileText,
  globe:       Globe,
  barchart:    BarChart2,
  layers:      Layers,
  penline:     PenLine,
  server:      Server,
  bot:         Bot,
  terminal:    Terminal,
  microscope:  Microscope,
  cpu:         Cpu,
};

export const TEAM_ICON_MAP: Record<TeamIconKey, React.FC<LucideProps>> = {
  code:      Code2,
  chart:     BarChart2,
  layout:    LayoutTemplate,
  rocket:    Rocket,
  users:     Users,
  briefcase: Briefcase,
  layers:    Layers,
};

// Accent color per agent capability (used as CSS color values)
export const CAPABILITY_ACCENT: Record<string, string> = {
  编码: '#60a5fa',
  文档: '#6eb88c',
  分析: '#8b5cf6',
  测试: '#c9944d',
  创意: '#e879a0',
  运维: '#5ea3a3',
};

// Each team has a deterministic accent derived from its id
const TEAM_ACCENTS = [
  '#60a5fa', // blue
  '#6eb88c', // green
  '#8b5cf6', // purple
  '#c9944d', // amber
  '#e879a0', // pink
  '#5ea3a3', // teal
];

export function getTeamAccent(id: string): string {
  let hash = 0;
  for (let i = 0; i < id.length; i++) hash = (hash * 31 + id.charCodeAt(i)) >>> 0;
  return TEAM_ACCENTS[hash % TEAM_ACCENTS.length];
}
