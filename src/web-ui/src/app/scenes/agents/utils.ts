import type { TFunction } from 'i18next';
import type { SubagentSource } from '@/infrastructure/api/service-api/SubagentAPI';
import type { AgentKind, AgentWithCapabilities, CapabilityCategory } from './agentsStore';

const MODE_DESCRIPTION_KEY_BY_ID: Record<string, string> = {
  agentic: 'Agentic',
  plan: 'Plan',
  debug: 'Debug',
  cowork: 'Cowork',
  computeruse: 'ComputerUse',
  deepresearch: 'DeepResearch',
};

interface AgentBadgeConfig {
  variant: 'accent' | 'info' | 'success' | 'purple' | 'neutral';
  label: string;
}

const LEGACY_CAPABILITY_CATEGORY_MAP: Record<string, CapabilityCategory> = {
  '\u7f16\u7801': 'coding',
  '\u6587\u6863': 'docs',
  '\u5206\u6790': 'analysis',
  '\u6d4b\u8bd5': 'testing',
  '\u521b\u610f': 'creative',
  '\u8fd0\u7ef4': 'ops',
};

function normalizeCapabilityCategory(category: string): CapabilityCategory {
  return LEGACY_CAPABILITY_CATEGORY_MAP[category] ?? 'analysis';
}

function getCapabilityLabel(
  t: TFunction<'scenes/agents'>,
  category: CapabilityCategory,
): string {
  return t(`capabilityCategories.${category}`);
}

function getAgentBadge(
  t: TFunction<'scenes/agents'>,
  agentKind?: AgentKind,
  source?: SubagentSource,
): AgentBadgeConfig {
  if (agentKind === 'mode') {
    return { variant: 'accent', label: t('agentCard.badges.agent') };
  }

  switch (source) {
    case 'user':
      return { variant: 'success', label: t('agentCard.badges.userSubagent') };
    case 'project':
      return { variant: 'purple', label: t('agentCard.badges.projectSubagent') };
    default:
      return { variant: 'info', label: t('agentCard.badges.subagent') };
  }
}

function getAgentDescription(
  t: TFunction<'scenes/agents'>,
  agent: Pick<AgentWithCapabilities, 'id' | 'name' | 'description'>,
): string {
  const fallback = agent.description?.trim() || '—';
  const canonicalModeKey = MODE_DESCRIPTION_KEY_BY_ID[agent.id.toLowerCase()];
  const candidates = Array.from(new Set([
    agent.id,
    canonicalModeKey,
    agent.name,
  ].filter(Boolean)));

  for (const key of candidates) {
    const translated = t(`agentDescriptions.${key}`, { defaultValue: '' }).trim();
    if (translated) {
      return translated;
    }
  }

  return fallback;
}

function enrichCapabilities(agent: AgentWithCapabilities): AgentWithCapabilities {
  if (agent.capabilities?.length) {
    return {
      ...agent,
      capabilities: agent.capabilities.map((cap) => ({
        ...cap,
        category: normalizeCapabilityCategory(cap.category),
      })),
    };
  }
  const id = agent.id.toLowerCase();
  const name = agent.name.toLowerCase();

  if (agent.agentKind === 'mode') {
    if (id === 'agentic') return { ...agent, capabilities: [{ category: 'coding', level: 5 }, { category: 'analysis', level: 4 }] };
    if (id === 'plan') return { ...agent, capabilities: [{ category: 'analysis', level: 5 }, { category: 'docs', level: 3 }] };
    if (id === 'debug') return { ...agent, capabilities: [{ category: 'coding', level: 5 }, { category: 'analysis', level: 3 }] };
    if (id === 'cowork') return { ...agent, capabilities: [{ category: 'analysis', level: 4 }, { category: 'creative', level: 3 }] };
    if (id === 'computeruse') return { ...agent, capabilities: [{ category: 'ops', level: 5 }, { category: 'analysis', level: 3 }] };
    if (id === 'deepresearch') return { ...agent, capabilities: [{ category: 'analysis', level: 5 }, { category: 'docs', level: 4 }] };
  }

  if (id === 'explore') return { ...agent, capabilities: [{ category: 'analysis', level: 4 }, { category: 'coding', level: 3 }] };
  if (id === 'file_finder') return { ...agent, capabilities: [{ category: 'analysis', level: 3 }, { category: 'coding', level: 2 }] };

  if (name.includes('code') || name.includes('debug') || name.includes('test')) {
    return { ...agent, capabilities: [{ category: 'coding', level: 4 }] };
  }
  if (name.includes('doc') || name.includes('write')) {
    return { ...agent, capabilities: [{ category: 'docs', level: 4 }] };
  }

  return { ...agent, capabilities: [{ category: 'analysis', level: 3 }] };
}

export { getAgentBadge, getCapabilityLabel, getAgentDescription, enrichCapabilities };
export type { AgentBadgeConfig };
