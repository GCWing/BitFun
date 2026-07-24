/**
 * Agents scene state management
 */
import { create } from 'zustand';
import type { SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import type { SubagentModelSelection } from '@/infrastructure/config/types';
import {
  CAPABILITY_ACCENT,
  CAPABILITY_CATEGORIES,
  type CapabilityCategory,
} from './agentTheme';

export { CAPABILITY_CATEGORIES };
export type { CapabilityCategory };

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
  visibleSubagentCount?: number;
  /** Explicit model selection for this Subagent, if it overrides the shared default. */
  subagentModelOverride?: SubagentModelSelection;
  /** Display name for an explicitly configured Subagent model override. */
  subagentModelDisplayName?: string;
}

export const CAPABILITY_COLORS: Record<CapabilityCategory, string> = CAPABILITY_ACCENT;

export type AgentsScenePage = 'home' | 'createAgent';
export type AgentEditorMode = 'create' | 'edit';
export type AgentFilterLevel = 'all' | 'builtin' | 'user' | 'project' | 'external';
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
}));
