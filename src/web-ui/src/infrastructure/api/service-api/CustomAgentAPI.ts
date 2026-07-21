import { api } from './ApiClient';
import { globalEventBus } from '@/infrastructure/event-bus';

export type AgentSource = 'builtin' | 'project' | 'user' | 'external';
export type CustomAgentKind = 'mode' | 'subagent';
export type CustomAgentLevel = 'user' | 'project';
export type UserContextSection =
  | 'workspace_context'
  | 'workspace_instructions'
  | 'project_layout';

export interface CustomAgentDetail {
  agentId: string;
  kind: CustomAgentKind;
  name: string;
  description: string;
  prompt: string;
  tools: string[];
  readonly: boolean;
  review: boolean;
  model: string;
  path: string;
  level: CustomAgentLevel;
  userContextPolicy: UserContextSection[];
}

export interface GetCustomAgentDetailPayload {
  agentId: string;
  workspacePath?: string;
}

export interface CreateCustomAgentPayload {
  kind: CustomAgentKind;
  level?: CustomAgentLevel;
  id: string;
  name: string;
  description: string;
  prompt: string;
  tools?: string[];
  readonly?: boolean;
  review?: boolean;
  model?: string;
  userContextPolicy?: UserContextSection[];
  workspacePath?: string;
}

export interface UpdateCustomAgentPayload {
  agentId: string;
  name: string;
  description: string;
  prompt: string;
  tools?: string[];
  readonly?: boolean;
  review?: boolean;
  model?: string;
  userContextPolicy?: UserContextSection[];
  workspacePath?: string;
}

function emitCustomAgentCatalogUpdated(payload: {
  agentId?: string;
  kind?: CustomAgentKind;
  workspacePath?: string;
}) {
  globalEventBus.emit('custom-agent:updated', payload);
  globalEventBus.emit('mode:config:updated', {
    reason: 'custom-agent-catalog-updated',
    ...payload,
  });
}

export const CustomAgentAPI = {
  async getCustomAgentDetail(
    payload: GetCustomAgentDetailPayload,
  ): Promise<CustomAgentDetail> {
    return api.invoke<CustomAgentDetail>('get_custom_agent_detail', {
      request: payload,
    });
  },

  async createCustomAgent(payload: CreateCustomAgentPayload): Promise<void> {
    await api.invoke('create_custom_agent', {
      request: payload,
    });
    emitCustomAgentCatalogUpdated({
      agentId: payload.id,
      kind: payload.kind,
      workspacePath: payload.workspacePath,
    });
  },

  async updateCustomAgent(payload: UpdateCustomAgentPayload): Promise<void> {
    await api.invoke('update_custom_agent', {
      request: payload,
    });
    emitCustomAgentCatalogUpdated({
      agentId: payload.agentId,
      workspacePath: payload.workspacePath,
    });
  },

  async deleteCustomAgent(agentId: string, workspacePath?: string): Promise<void> {
    await api.invoke('delete_custom_agent', {
      request: { agentId, workspacePath },
    });
    emitCustomAgentCatalogUpdated({ agentId, workspacePath });
  },

  async reloadCustomAgents(workspacePath?: string): Promise<void> {
    await api.invoke('reload_custom_agents', {
      request: { workspacePath },
    });
    emitCustomAgentCatalogUpdated({ workspacePath });
  },
};
