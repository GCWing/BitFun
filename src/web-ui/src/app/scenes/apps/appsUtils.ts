import type { SubagentSource } from '@/infrastructure/api/service-api/SubagentAPI';
import type { AgentWithCapabilities } from './hooks/useAppsData';

interface AppBadgeConfig {
  variant: 'accent' | 'info' | 'success' | 'purple' | 'neutral';
  label: string;
}

export function getAgentBadge(
  t: (key: string, options?: Record<string, unknown>) => string,
  source?: SubagentSource,
): AppBadgeConfig {
  switch (source) {
    case 'user':
      return { variant: 'success', label: t('agent.badges.userAgent') };
    case 'project':
      return { variant: 'purple', label: t('agent.badges.projectAgent') };
    default:
      return { variant: 'accent', label: t('agent.badges.agent') };
  }
}

export function enrichAgentCapabilities(agent: AgentWithCapabilities): AgentWithCapabilities {
  if (agent.capabilities.length > 0) return agent;

  const id = agent.id.toLowerCase();
  const name = agent.name.toLowerCase();

  if (id === 'agentic') {
    return { ...agent, iconKey: 'code2', capabilities: [{ category: '编码', level: 5 }, { category: '分析', level: 4 }] };
  }
  if (id === 'plan') {
    return { ...agent, iconKey: 'layers', capabilities: [{ category: '分析', level: 5 }, { category: '文档', level: 3 }] };
  }
  if (id === 'debug') {
    return { ...agent, iconKey: 'bug', capabilities: [{ category: '编码', level: 5 }, { category: '分析', level: 3 }] };
  }
  if (id === 'cowork') {
    return { ...agent, iconKey: 'briefcase', capabilities: [{ category: '文档', level: 4 }, { category: '创意', level: 3 }] };
  }
  if (id === 'claw') {
    return { ...agent, iconKey: 'bot', capabilities: [{ category: '分析', level: 4 }, { category: '文档', level: 4 }] };
  }

  if (name.includes('code') || name.includes('debug') || name.includes('test')) {
    return { ...agent, capabilities: [{ category: '编码', level: 4 }] };
  }
  if (name.includes('doc') || name.includes('write')) {
    return { ...agent, capabilities: [{ category: '文档', level: 4 }] };
  }

  return { ...agent, capabilities: [{ category: '分析', level: 3 }] };
}
