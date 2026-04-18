export type AppKind = 'mode-app' | 'standalone-agent-app';

export interface BaseAppEntity {
  id: string;
  kind: AppKind;
  nameKey: string;
  descriptionKey: string;
  badgeKey: string;
}

export interface ModeAppEntity extends BaseAppEntity {
  kind: 'mode-app';
  agentIds: string[];
}

export interface StandaloneAgentAppEntity extends BaseAppEntity {
  kind: 'standalone-agent-app';
  agentId: string;
}

export type AppEntity = ModeAppEntity | StandaloneAgentAppEntity;

export const APP_REGISTRY: readonly AppEntity[] = [
  {
    id: 'coding-app',
    kind: 'mode-app',
    nameKey: 'apps.coding.name',
    descriptionKey: 'apps.coding.description',
    badgeKey: 'apps.badges.modeApp',
    agentIds: ['agentic', 'Plan', 'debug'],
  },
  {
    id: 'cowork-app',
    kind: 'standalone-agent-app',
    nameKey: 'apps.cowork.name',
    descriptionKey: 'apps.cowork.description',
    badgeKey: 'apps.badges.standaloneAgentApp',
    agentId: 'Cowork',
  },
  {
    id: 'claw-app',
    kind: 'standalone-agent-app',
    nameKey: 'apps.claw.name',
    descriptionKey: 'apps.claw.description',
    badgeKey: 'apps.badges.standaloneAgentApp',
    agentId: 'Claw',
  },
] as const;

export const HIDDEN_AGENT_IDS = new Set<string>(['Dispatcher']);

export function isPrimaryAgentMode(agent: { id: string; agentKind?: string }): boolean {
  if (HIDDEN_AGENT_IDS.has(agent.id)) return false;
  return agent.agentKind === 'mode';
}
