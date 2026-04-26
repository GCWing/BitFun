/**
 * Agents scene state management
 */
import { create } from 'zustand';
import type { SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';

export const CAPABILITY_CATEGORIES = ['coding', 'docs', 'analysis', 'testing', 'creative', 'ops'] as const;
export type CapabilityCategory = (typeof CAPABILITY_CATEGORIES)[number];

/** 'mode' = primary agent mode (e.g. Agentic/Plan/Debug); 'subagent' = sub-agent */
export type AgentKind = 'mode' | 'subagent';

export interface AgentCapability {
  category: CapabilityCategory;
  level: number;
}

export interface AgentWithCapabilities extends SubagentInfo {
  capabilities: AgentCapability[];
  iconKey?: string;
  /** Distinguishes primary agent mode from sub-agent */
  agentKind?: AgentKind;
}

export const CAPABILITY_COLORS: Record<CapabilityCategory, string> = {
  coding: '#60a5fa',
  docs: '#6eb88c',
  analysis: '#8b5cf6',
  testing: '#c9944d',
  creative: '#e879a0',
  ops: '#5ea3a3',
};

export type AgentsScenePage = 'home' | 'createAgent' | 'reviewTeam';
export type AgentEditorMode = 'create' | 'edit';
export type AgentFilterLevel = 'all' | 'builtin' | 'user' | 'project';
export type AgentFilterType = 'all' | 'mode' | 'subagent';

interface AgentsStoreState {
  page: AgentsScenePage;
  agentEditorMode: AgentEditorMode;
  editingAgentId: string | null;
  searchQuery: string;
  agentFilterLevel: AgentFilterLevel;
  agentFilterType: AgentFilterType;
  setPage: (page: AgentsScenePage) => void;
  setSearchQuery: (query: string) => void;
  setAgentFilterLevel: (filter: AgentFilterLevel) => void;
  setAgentFilterType: (filter: AgentFilterType) => void;
  openHome: () => void;
  openCreateAgent: () => void;
  openEditAgent: (agentId: string) => void;
  openReviewTeam: () => void;
  agentSoloEnabled: Record<string, boolean>;
  setAgentSoloEnabled: (agentId: string, enabled: boolean) => void;
}

export const useAgentsStore = create<AgentsStoreState>((set) => ({
  page: 'home',
  agentEditorMode: 'create',
  editingAgentId: null,
  searchQuery: '',
  agentFilterLevel: 'all',
  agentFilterType: 'all',
  setPage: (page) => set({ page }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  setAgentFilterLevel: (filter) => set({ agentFilterLevel: filter }),
  setAgentFilterType: (filter) => set({ agentFilterType: filter }),
  openHome: () => set({ page: 'home', agentEditorMode: 'create', editingAgentId: null }),
  openCreateAgent: () => set({
    page: 'createAgent',
    agentEditorMode: 'create',
    editingAgentId: null,
  }),
  openEditAgent: (agentId: string) => set({
    page: 'createAgent',
    agentEditorMode: 'edit',
    editingAgentId: agentId,
  }),
  openReviewTeam: () => set({
    page: 'reviewTeam',
    agentEditorMode: 'create',
    editingAgentId: null,
  }),
  agentSoloEnabled: {},
  setAgentSoloEnabled: (agentId, enabled) =>
    set((s) => ({
      agentSoloEnabled: {
        ...s.agentSoloEnabled,
        [agentId]: enabled,
      },
    })),
}));
