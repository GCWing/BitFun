import { UI_EXCEPTION_ACCENTS } from '@/shared/theme/uiExceptionAccents';

export type AgentAccentStyle = {
  accentColor: string;
  accentBg: string;
};

export const CAPABILITY_CATEGORIES = ['coding', 'docs', 'analysis', 'testing', 'creative', 'ops'] as const;
export type CapabilityCategory = (typeof CAPABILITY_CATEGORIES)[number];

export function getAlphaColor(color: string, alphaHex = '44', percent = 27): string {
  if (color.startsWith('var(') || color.startsWith('color-mix(')) {
    return `color-mix(in srgb, ${color} ${percent}%, transparent)`;
  }
  if (/^#[0-9a-fA-F]{6}$/.test(color)) {
    return `${color}${alphaHex}`;
  }
  return color;
}

export const CAPABILITY_ACCENT: Record<CapabilityCategory, string> = {
  coding: 'var(--color-accent-500)',
  docs: UI_EXCEPTION_ACCENTS.agentCapability.docs,
  analysis: 'var(--color-purple-500)',
  testing: UI_EXCEPTION_ACCENTS.agentCapability.testing,
  creative: UI_EXCEPTION_ACCENTS.agentCapability.creative,
  ops: UI_EXCEPTION_ACCENTS.agentCapability.ops,
};

export function getCapabilityAccentBorder(category: CapabilityCategory): string {
  return getAlphaColor(CAPABILITY_ACCENT[category]);
}

export const CORE_AGENT_ACCENTS = {
  agentic: {
    accentColor: 'var(--color-indigo-500)',
    accentBg: getAlphaColor('var(--color-indigo-500)', '1a', 10),
  },
  Cowork: {
    accentColor: UI_EXCEPTION_ACCENTS.tealAction,
    accentBg: getAlphaColor(UI_EXCEPTION_ACCENTS.tealAction, '1a', 10),
  },
  ComputerUse: {
    accentColor: 'var(--color-warning)',
    accentBg: 'var(--color-warning-bg)',
  },
} as const satisfies Record<string, AgentAccentStyle>;

export const DEFAULT_CORE_AGENT_ACCENT: AgentAccentStyle = CORE_AGENT_ACCENTS.agentic;

export const AGENT_TEAM_TAG_COLORS = [
  {
    color: CORE_AGENT_ACCENTS.ComputerUse.accentColor,
    border: getAlphaColor(CORE_AGENT_ACCENTS.ComputerUse.accentColor),
  },
  {
    color: CORE_AGENT_ACCENTS.Cowork.accentColor,
    border: getAlphaColor(CORE_AGENT_ACCENTS.Cowork.accentColor),
  },
  {
    color: CORE_AGENT_ACCENTS.agentic.accentColor,
    border: getAlphaColor(CORE_AGENT_ACCENTS.agentic.accentColor),
  },
] as const;
