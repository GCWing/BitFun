import type { TFunction } from 'i18next';
import type { SubagentSource } from '@/infrastructure/api/service-api/SubagentAPI';
import type { AgentKind, AgentWithCapabilities } from './agentsStore';

interface AgentBadgeConfig {
  variant: 'accent' | 'info' | 'success' | 'purple' | 'neutral';
  label: string;
}

function getAgentBadge(
  t: TFunction<'scenes/agents'>,
  agentKind?: AgentKind,
  source?: SubagentSource,
): AgentBadgeConfig {
  if (agentKind === 'mode') {
    return { variant: 'accent', label: t('agentCard.badges.agent', 'Agent') };
  }

  switch (source) {
    case 'user':
      return { variant: 'success', label: t('agentCard.badges.userSubagent', '用户 Sub-Agent') };
    case 'project':
      return { variant: 'purple', label: t('agentCard.badges.projectSubagent', '项目 Sub-Agent') };
    default:
      return { variant: 'info', label: t('agentCard.badges.subagent', 'Sub-Agent') };
  }
}

function enrichCapabilities(agent: AgentWithCapabilities): AgentWithCapabilities {
  if (agent.capabilities?.length) return agent;
  const id = agent.id.toLowerCase();
  const name = agent.name.toLowerCase();

  if (agent.agentKind === 'mode') {
    if (id === 'agentic') return { ...agent, capabilities: [{ category: '编码', level: 5 }, { category: '分析', level: 4 }] };
    if (id === 'plan') return { ...agent, capabilities: [{ category: '分析', level: 5 }, { category: '文档', level: 3 }] };
    if (id === 'debug') return { ...agent, capabilities: [{ category: '编码', level: 5 }, { category: '分析', level: 3 }] };
    if (id === 'cowork') return { ...agent, capabilities: [{ category: '分析', level: 4 }, { category: '创意', level: 3 }] };
  }

  if (id === 'explore') return { ...agent, capabilities: [{ category: '分析', level: 4 }, { category: '编码', level: 3 }] };
  if (id === 'file_finder') return { ...agent, capabilities: [{ category: '分析', level: 3 }, { category: '编码', level: 2 }] };
  if (id === 'deepresearch') return {
    ...agent,
    capabilities: [
      { category: '分析', level: 5 },
    ],
  };

  if (name.includes('code') || name.includes('debug') || name.includes('test')) {
    return { ...agent, capabilities: [{ category: '编码', level: 4 }] };
  }
  if (name.includes('doc') || name.includes('write')) {
    return { ...agent, capabilities: [{ category: '文档', level: 4 }] };
  }

  return { ...agent, capabilities: [{ category: '分析', level: 3 }] };
}

export { getAgentBadge, enrichCapabilities };
export type { AgentBadgeConfig };
