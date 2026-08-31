import { APPEARANCE_DOMAIN_TOKENS } from '@/infrastructure/appearance/appearanceDomainTokens';

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
  coding: 'var(--bf-color-accent-default)',
  docs: APPEARANCE_DOMAIN_TOKENS.agentCapability.docs,
  analysis: 'var(--bf-color-accent-secondary)',
  testing: APPEARANCE_DOMAIN_TOKENS.agentCapability.testing,
  creative: APPEARANCE_DOMAIN_TOKENS.agentCapability.creative,
  ops: APPEARANCE_DOMAIN_TOKENS.agentCapability.ops,
};

export function getCapabilityAccentBorder(category: CapabilityCategory): string {
  return getAlphaColor(CAPABILITY_ACCENT[category]);
}

export const CORE_AGENT_ACCENTS = {
  agentic: {
    accentColor: 'var(--bf-color-accent-default)',
    accentBg: getAlphaColor('var(--bf-color-accent-default)', '1a', 10),
  },
  Cowork: {
    accentColor: APPEARANCE_DOMAIN_TOKENS.tealAction,
    accentBg: getAlphaColor(APPEARANCE_DOMAIN_TOKENS.tealAction, '1a', 10),
  },
  ComputerUse: {
    accentColor: 'var(--bf-color-status-warning-content)',
    accentBg: 'var(--bf-color-status-warning-surface)',
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
