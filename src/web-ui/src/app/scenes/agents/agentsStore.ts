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
} from './agentAppearance';

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

// ─── Agent team model (recovered from afc8c0aa1~1, adapted to HEAD) ───────────

export type MemberRole = 'leader' | 'member' | 'reviewer';
export type AgentTeamStrategy = 'sequential' | 'collaborative' | 'free';
export type AgentTeamViewMode = 'formation' | 'list';

/**
 * Seven-state member projection, aligned with the backend SessionDisplayState
 * values (standby/processing/completed/hung/interrupted/pending_attention/
 * viewed) so the persisted team backend can plug in later. Consumed by the
 * formation DAG nodes via the Badge/variant mapping.
 */
export type MemberDisplayState =
  | 'standby'
  | 'processing'
  | 'completed'
  | 'hung'
  | 'interrupted'
  | 'pending_attention'
  | 'viewed';

export interface AgentTeamMember {
  agentId: string;
  role: MemberRole;
  modelOverride?: string;
  order: number;
  displayState?: MemberDisplayState;
}

export interface AgentTeam {
  id: string;
  name: string;
  icon: string;
  description: string;
  members: AgentTeamMember[];
  /** Editable DAG edges (from -> to), kept next to the members so the canvas can rewire them. */
  edges: Array<[string, string]>;
  strategy: AgentTeamStrategy;
  shareContext: boolean;
}

/**
 * Mock agents used by the recovered team gallery/editor (builtin-only).
 * Frontend mock data for the gallery; R-WF-17 (DAG orchestration) replaces
 * this with the persisted team backend, at which point this seed can be
 * removed.
 */
export const MOCK_AGENT_TEAMS: AgentTeam[] = [
  {
    id: 'agent-team-coding',
    name: 'Coding Team',
    icon: 'code',
    description: 'Code review, refactoring and quality assurance',
    members: [
      { agentId: 'agentic', role: 'leader', order: 0, displayState: 'processing' },
      { agentId: 'CodeReview', role: 'member', order: 1, displayState: 'completed' },
      { agentId: 'Debug', role: 'member', order: 2, displayState: 'standby' },
      { agentId: 'GeneralPurpose', role: 'reviewer', order: 3, displayState: 'viewed' },
    ],
    edges: [
      ['agentic', 'CodeReview'],
      ['agentic', 'Debug'],
      ['CodeReview', 'GeneralPurpose'],
      ['Debug', 'GeneralPurpose'],
    ],
    strategy: 'collaborative',
    shareContext: true,
  },
  {
    id: 'agent-team-research',
    name: 'Research Team',
    icon: 'chart',
    description: 'Information gathering, data analysis and report writing',
    members: [
      { agentId: 'DeepResearch', role: 'leader', order: 0, displayState: 'completed' },
      { agentId: 'Explore', role: 'member', order: 1, displayState: 'hung' },
      { agentId: 'FileFinder', role: 'reviewer', order: 2, displayState: 'pending_attention' },
    ],
    edges: [
      ['DeepResearch', 'Explore'],
      ['DeepResearch', 'FileFinder'],
    ],
    strategy: 'sequential',
    shareContext: true,
  },
  {
    id: 'agent-team-ppt',
    name: 'PPT Production',
    icon: 'layout',
    description: 'Content planning, visual design and copy polishing',
    members: [
      { agentId: 'Cowork', role: 'leader', order: 0, displayState: 'interrupted' },
    ],
    edges: [],
    strategy: 'collaborative',
    shareContext: false,
  },
];

export const AGENT_TEAM_TEMPLATES: Array<{
  id: string;
  name: string;
  icon: string;
  description: string;
  memberIds: string[];
}> = [
  {
    id: 'tpl-coding',
    name: 'Coding Team',
    icon: 'code',
    description: 'Code review, refactoring and quality assurance',
    memberIds: ['agentic', 'CodeReview', 'Debug', 'GeneralPurpose'],
  },
  {
    id: 'tpl-research',
    name: 'Research Team',
    icon: 'chart',
    description: 'Information gathering, data analysis and report writing',
    memberIds: ['DeepResearch', 'Explore', 'FileFinder'],
  },
  {
    id: 'tpl-ppt',
    name: 'PPT Production',
    icon: 'layout',
    description: 'Content planning, copy and visual planning',
    memberIds: ['Cowork'],
  },
  {
    id: 'tpl-fullstack',
    name: 'Fullstack Team',
    icon: 'rocket',
    description: 'End-to-end development, testing and documentation',
    memberIds: ['agentic', 'Debug', 'GeneralPurpose', 'CodeReview'],
  },
];

/** Compute the max capability level a team covers, keyed by capability category. */
export function computeAgentTeamCapabilities(
  team: AgentTeam,
  allAgents: AgentWithCapabilities[],
): Record<CapabilityCategory, number> {
  const result: Record<CapabilityCategory, number> = {
    coding: 0,
    docs: 0,
    analysis: 0,
    testing: 0,
    creative: 0,
    ops: 0,
  };
  for (const member of team.members) {
    const agent = allAgents.find((a) => a.id === member.agentId);
    if (!agent) continue;
    for (const cap of agent.capabilities) {
      result[cap.category] = Math.max(result[cap.category], cap.level);
    }
  }
  return result;
}

export type AgentsScenePage = 'home' | 'createAgent' | 'createLegion' | 'reviewTeam' | 'agentTeamEditor';
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
  openCreateLegion: () => void;
  openEditAgent: (agentId: string) => void;
  openReviewTeam: () => void;
  openAgentTeamEditor: (teamId: string) => void;

  // Agent team editor state (recovered from afc8c0aa1~1)
  agentTeams: AgentTeam[];
  activeAgentTeamId: string | null;
  viewMode: AgentTeamViewMode;
  /** Shared agent data for the team gallery/editor, synced from useAgentsList. */
  teamComposerAgents: AgentWithCapabilities[];
  setTeamComposerAgents: (agents: AgentWithCapabilities[]) => void;
  setActiveAgentTeam: (id: string | null) => void;
  setViewMode: (mode: AgentTeamViewMode) => void;
  addAgentTeam: (team: Omit<AgentTeam, 'members' | 'edges'>) => void;
  updateAgentTeam: (id: string, patch: Partial<Pick<AgentTeam, 'name' | 'icon' | 'description' | 'strategy' | 'shareContext'>>) => void;
  deleteAgentTeam: (id: string) => void;
  addMember: (teamId: string, agentId: string, role?: MemberRole) => void;
  removeMember: (teamId: string, agentId: string) => void;
  updateMemberRole: (teamId: string, agentId: string, role: MemberRole) => void;
  /** DAG edge editing: create/rewire/remove edges between member nodes (R-WF-17). */
  addTeamEdge: (teamId: string, from: string, to: string) => void;
  removeTeamEdge: (teamId: string, from: string, to: string) => void;
  updateMemberDisplayState: (teamId: string, agentId: string, state: MemberDisplayState) => void;
  setMemberDisplayStates: (teamId: string, states: Record<string, MemberDisplayState>) => void;
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
  openCreateLegion: () => set({ page: 'createLegion' }),
  openEditAgent: (agentId: string) => set({
    page: 'createAgent',
    agentEditorMode: 'edit',
    editingAgentId: agentId,
  }),
  openReviewTeam: () => set({ page: 'reviewTeam' }),
  openAgentTeamEditor: (teamId) => set({ page: 'agentTeamEditor', activeAgentTeamId: teamId }),

  agentTeams: MOCK_AGENT_TEAMS,
  activeAgentTeamId: MOCK_AGENT_TEAMS[0].id,
  viewMode: 'formation',
  teamComposerAgents: [],
  setTeamComposerAgents: (agents) => set({ teamComposerAgents: agents }),
  setActiveAgentTeam: (id) => set({ activeAgentTeamId: id }),
  setViewMode: (mode) => set({ viewMode: mode }),
  addAgentTeam: (team) => {
    const newAgentTeam: AgentTeam = { ...team, members: [], edges: [] };
    set((s) => ({ agentTeams: [...s.agentTeams, newAgentTeam], activeAgentTeamId: newAgentTeam.id }));
  },
  updateAgentTeam: (id, patch) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) => (t.id === id ? { ...t, ...patch } : t)),
    })),
  deleteAgentTeam: (id) =>
    set((s) => {
      const next = s.agentTeams.filter((t) => t.id !== id);
      const activeId = s.activeAgentTeamId === id ? (next[0]?.id ?? null) : s.activeAgentTeamId;
      return { agentTeams: next, activeAgentTeamId: activeId };
    }),
  addMember: (teamId, agentId, role = 'member') =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) => {
        if (t.id !== teamId) return t;
        if (t.members.some((m) => m.agentId === agentId)) return t;
        const newMember: AgentTeamMember = { agentId, role, order: t.members.length };
        return { ...t, members: [...t.members, newMember] };
      }),
    })),
  removeMember: (teamId, agentId) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) =>
        t.id === teamId
          ? { ...t, members: t.members.filter((m) => m.agentId !== agentId) }
          : t,
      ),
    })),
  updateMemberRole: (teamId, agentId, role) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) =>
        t.id === teamId
          ? { ...t, members: t.members.map((m) => (m.agentId === agentId ? { ...m, role } : m)) }
          : t,
      ),
    })),
  addTeamEdge: (teamId, from, to) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) => {
        if (t.id !== teamId) return t;
        const memberIds = new Set(t.members.map((m) => m.agentId));
        if (!memberIds.has(from) || !memberIds.has(to) || from === to) return t;
        if (t.edges.some(([a, b]) => a === from && b === to)) return t;
        return { ...t, edges: [...t.edges, [from, to]] };
      }),
    })),
  removeTeamEdge: (teamId, from, to) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) =>
        t.id === teamId
          ? { ...t, edges: t.edges.filter(([a, b]) => !(a === from && b === to)) }
          : t,
      ),
    })),
  updateMemberDisplayState: (teamId, agentId, state) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) =>
        t.id === teamId
          ? {
              ...t,
              members: t.members.map((m) => (m.agentId === agentId ? { ...m, displayState: state } : m)),
            }
          : t,
      ),
    })),
  setMemberDisplayStates: (teamId, states) =>
    set((s) => ({
      agentTeams: s.agentTeams.map((t) =>
        t.id === teamId
          ? {
              ...t,
              members: t.members.map((m) =>
                states[m.agentId] ? { ...m, displayState: states[m.agentId] } : m,
              ),
            }
          : t,
      ),
    })),
}));
