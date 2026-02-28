/**
 * Team scene state management + mock data
 */
import { create } from 'zustand';
import type { SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';

// ─── Types ────────────────────────────────────────────────────────────────────

export type MemberRole = 'leader' | 'member' | 'reviewer';
export type TeamStrategy = 'sequential' | 'collaborative' | 'free';
export type TeamViewMode = 'formation' | 'list';

export const CAPABILITY_CATEGORIES = ['编码', '文档', '分析', '测试', '创意', '运维'] as const;
export type CapabilityCategory = (typeof CAPABILITY_CATEGORIES)[number];

/** 'mode' = 主 Agent 模式（如 Agentic/Plan/Debug），'subagent' = 子 Agent */
export type AgentKind = 'mode' | 'subagent';

export interface AgentCapability {
  category: CapabilityCategory;
  level: number; // 1-5
}

export interface AgentWithCapabilities extends SubagentInfo {
  capabilities: AgentCapability[];
  iconKey?: string;
  /** 区分 Agent 模式与 Sub-Agent */
  agentKind?: AgentKind;
}

export interface TeamMember {
  agentId: string;
  role: MemberRole;
  modelOverride?: string;
  order: number;
}

export interface Team {
  id: string;
  name: string;
  icon: string;
  description: string;
  members: TeamMember[];
  strategy: TeamStrategy;
  shareContext: boolean;
}

// ─── Capability colors ────────────────────────────────────────────────────────

export const CAPABILITY_COLORS: Record<CapabilityCategory, string> = {
  编码: '#60a5fa',
  文档: '#6eb88c',
  分析: '#8b5cf6',
  测试: '#c9944d',
  创意: '#e879a0',
  运维: '#5ea3a3',
};

// ─── Mock agents ──────────────────────────────────────────────────────────────

export const MOCK_AGENTS: AgentWithCapabilities[] = [
  {
    id: 'mock-code-architect',
    name: 'CodeArchitect',
    description: '负责系统架构设计、代码结构规划与技术选型，擅长识别设计模式和架构反模式',
    isReadonly: true,
    toolCount: 14,
    defaultTools: ['read_file', 'write_file', 'search_code', 'run_command'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'primary',
    iconKey: 'code2',
    capabilities: [
      { category: '编码', level: 5 },
      { category: '分析', level: 4 },
      { category: '文档', level: 3 },
    ],
  },
  {
    id: 'mock-code-reviewer',
    name: 'CodeReviewer',
    description: '专注代码审查与质量评估，能发现潜在 Bug、安全漏洞及性能瓶颈',
    isReadonly: true,
    toolCount: 8,
    defaultTools: ['read_file', 'search_code', 'list_files'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'primary',
    iconKey: 'eye',
    capabilities: [
      { category: '编码', level: 4 },
      { category: '测试', level: 3 },
      { category: '分析', level: 4 },
    ],
  },
  {
    id: 'mock-test-gen',
    name: 'TestGenerator',
    description: '自动生成单元测试、集成测试用例，提升代码覆盖率，支持多种测试框架',
    isReadonly: true,
    toolCount: 10,
    defaultTools: ['read_file', 'write_file', 'run_command', 'search_code'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'fast',
    iconKey: 'flask',
    capabilities: [
      { category: '测试', level: 5 },
      { category: '编码', level: 3 },
      { category: '分析', level: 2 },
    ],
  },
  {
    id: 'mock-debugger',
    name: 'Debugger',
    description: '精准定位程序错误，分析堆栈跟踪，给出修复建议，支持多种运行时环境',
    isReadonly: true,
    toolCount: 12,
    defaultTools: ['read_file', 'run_command', 'search_code', 'search_files'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'primary',
    iconKey: 'bug',
    capabilities: [
      { category: '编码', level: 5 },
      { category: '测试', level: 4 },
      { category: '分析', level: 3 },
    ],
  },
  {
    id: 'mock-documentor',
    name: 'Documentor',
    description: '自动生成项目文档、API 文档与注释，保持文档与代码同步更新',
    isReadonly: true,
    toolCount: 6,
    defaultTools: ['read_file', 'write_file', 'list_files'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'fast',
    iconKey: 'filetext',
    capabilities: [
      { category: '文档', level: 5 },
      { category: '分析', level: 3 },
      { category: '创意', level: 2 },
    ],
  },
  {
    id: 'mock-researcher',
    name: 'Researcher',
    description: '深度信息检索与知识整合，擅长从海量资料中提炼关键洞察与数据支撑',
    isReadonly: true,
    toolCount: 9,
    defaultTools: ['web_search', 'read_file', 'write_file'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'primary',
    iconKey: 'globe',
    capabilities: [
      { category: '分析', level: 5 },
      { category: '文档', level: 4 },
      { category: '创意', level: 2 },
    ],
  },
  {
    id: 'mock-data-analyst',
    name: 'DataAnalyst',
    description: '数据清洗、统计分析与可视化，擅长发现数据规律并生成分析报告',
    isReadonly: true,
    toolCount: 11,
    defaultTools: ['read_file', 'run_command', 'write_file', 'web_search'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'primary',
    iconKey: 'barchart',
    capabilities: [
      { category: '分析', level: 5 },
      { category: '文档', level: 4 },
      { category: '编码', level: 2 },
    ],
  },
  {
    id: 'mock-content-planner',
    name: 'ContentPlanner',
    description: '内容结构规划与大纲设计，擅长将复杂主题转化为清晰易懂的内容框架',
    isReadonly: true,
    toolCount: 5,
    defaultTools: ['read_file', 'write_file', 'web_search'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'fast',
    iconKey: 'layers',
    capabilities: [
      { category: '创意', level: 5 },
      { category: '文档', level: 4 },
      { category: '分析', level: 3 },
    ],
  },
  {
    id: 'mock-copywriter',
    name: 'Copywriter',
    description: '专业文案撰写与优化，擅长多种文体风格，能够精准传递品牌价值',
    isReadonly: true,
    toolCount: 4,
    defaultTools: ['read_file', 'write_file'],
    enabled: true,
    subagentSource: 'builtin',
    model: 'fast',
    iconKey: 'penline',
    capabilities: [
      { category: '创意', level: 4 },
      { category: '文档', level: 5 },
      { category: '分析', level: 2 },
    ],
  },
  {
    id: 'mock-ops-agent',
    name: 'OpsAgent',
    description: '自动化运维任务执行，监控系统状态，处理部署流程和环境配置',
    isReadonly: true,
    toolCount: 16,
    defaultTools: ['run_command', 'read_file', 'write_file', 'search_files'],
    enabled: false,
    subagentSource: 'builtin',
    model: 'fast',
    iconKey: 'server',
    capabilities: [
      { category: '运维', level: 5 },
      { category: '编码', level: 3 },
      { category: '分析', level: 2 },
    ],
  },
];

// ─── Mock teams with pre-seeded members ───────────────────────────────────────

export const MOCK_TEAMS: Team[] = [
  {
    id: 'team-coding',
    name: '编码团队',
    icon: 'code',
    description: '代码审查、重构与质量保障',
    members: [
      { agentId: 'mock-code-architect', role: 'leader', order: 0 },
      { agentId: 'mock-code-reviewer', role: 'member', order: 1 },
      { agentId: 'mock-debugger', role: 'member', order: 2 },
      { agentId: 'mock-test-gen', role: 'reviewer', order: 3 },
    ],
    strategy: 'collaborative',
    shareContext: true,
  },
  {
    id: 'team-research',
    name: '调研团队',
    icon: 'chart',
    description: '信息搜集、数据分析与报告撰写',
    members: [
      { agentId: 'mock-researcher', role: 'leader', order: 0 },
      { agentId: 'mock-data-analyst', role: 'member', order: 1 },
      { agentId: 'mock-documentor', role: 'reviewer', order: 2 },
    ],
    strategy: 'sequential',
    shareContext: true,
  },
  {
    id: 'team-ppt',
    name: 'PPT 制作',
    icon: 'layout',
    description: '内容策划、视觉设计与文案润色',
    members: [
      { agentId: 'mock-content-planner', role: 'leader', order: 0 },
      { agentId: 'mock-copywriter', role: 'member', order: 1 },
    ],
    strategy: 'collaborative',
    shareContext: false,
  },
];

// ─── Team templates (for "use template" quick start) ─────────────────────────

export const TEAM_TEMPLATES: Array<{
  id: string;
  name: string;
  icon: string;
  description: string;
  memberIds: string[];
}> = [
  {
    id: 'tpl-coding',
    name: '编码团队',
    icon: 'code',
    description: '代码审查、重构与质量保障',
    memberIds: ['mock-code-architect', 'mock-code-reviewer', 'mock-debugger', 'mock-test-gen'],
  },
  {
    id: 'tpl-research',
    name: '调研团队',
    icon: 'chart',
    description: '信息搜集、数据分析与报告撰写',
    memberIds: ['mock-researcher', 'mock-data-analyst', 'mock-documentor'],
  },
  {
    id: 'tpl-ppt',
    name: 'PPT 制作',
    icon: 'layout',
    description: '内容策划、文案与视觉规划',
    memberIds: ['mock-content-planner', 'mock-copywriter'],
  },
  {
    id: 'tpl-fullstack',
    name: '全栈团队',
    icon: 'rocket',
    description: '全流程开发、测试与文档',
    memberIds: ['mock-code-architect', 'mock-debugger', 'mock-test-gen', 'mock-documentor'],
  },
];

// ─── Helper: compute team capability coverage ─────────────────────────────────

export function computeTeamCapabilities(
  team: Team,
  allAgents: AgentWithCapabilities[],
): Record<CapabilityCategory, number> {
  const result: Record<CapabilityCategory, number> = {
    编码: 0, 文档: 0, 分析: 0, 测试: 0, 创意: 0, 运维: 0,
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

// ─── Scene page ───────────────────────────────────────────────────────────────

export type TeamScenePage = 'agentsOverview' | 'expertTeamsOverview' | 'editor';
export type HomeFilter = 'all' | 'agent' | 'team';

// ─── Store ────────────────────────────────────────────────────────────────────

interface TeamStoreState {
  // Scene navigation
  page: TeamScenePage;
  homeFilter: HomeFilter;
  setPage: (page: TeamScenePage) => void;
  setHomeFilter: (filter: HomeFilter) => void;
  openAgentsOverview: () => void;
  openExpertTeamsOverview: () => void;
  openTeamEditor: (teamId: string) => void;
  agentSoloEnabled: Record<string, boolean>;
  setAgentSoloEnabled: (agentId: string, enabled: boolean) => void;

  teams: Team[];
  activeTeamId: string | null;
  viewMode: TeamViewMode;

  setActiveTeam: (id: string | null) => void;
  setViewMode: (mode: TeamViewMode) => void;
  addTeam: (team: Omit<Team, 'members'>) => void;
  updateTeam: (id: string, patch: Partial<Pick<Team, 'name' | 'icon' | 'description' | 'strategy' | 'shareContext'>>) => void;
  deleteTeam: (id: string) => void;
  addMember: (teamId: string, agentId: string, role?: MemberRole) => void;
  removeMember: (teamId: string, agentId: string) => void;
  updateMemberRole: (teamId: string, agentId: string, role: MemberRole) => void;
}

export const useTeamStore = create<TeamStoreState>((set) => ({
  page: 'agentsOverview',
  homeFilter: 'all',
  setPage: (page) => set({ page }),
  setHomeFilter: (filter) => set({ homeFilter: filter }),
  openAgentsOverview: () => set({ page: 'agentsOverview' }),
  openExpertTeamsOverview: () => set({ page: 'expertTeamsOverview' }),
  openTeamEditor: (teamId) => set({ page: 'editor', activeTeamId: teamId }),
  agentSoloEnabled: {},
  setAgentSoloEnabled: (agentId, enabled) =>
    set((s) => ({
      agentSoloEnabled: {
        ...s.agentSoloEnabled,
        [agentId]: enabled,
      },
    })),

  teams: MOCK_TEAMS,
  activeTeamId: MOCK_TEAMS[0].id,
  viewMode: 'formation',

  setActiveTeam: (id) => set({ activeTeamId: id }),
  setViewMode: (mode) => set({ viewMode: mode }),

  addTeam: (team) => {
    const newTeam: Team = { ...team, members: [] };
    set((s) => ({ teams: [...s.teams, newTeam], activeTeamId: newTeam.id }));
  },

  updateTeam: (id, patch) =>
    set((s) => ({
      teams: s.teams.map((t) => (t.id === id ? { ...t, ...patch } : t)),
    })),

  deleteTeam: (id) =>
    set((s) => {
      const next = s.teams.filter((t) => t.id !== id);
      const activeId = s.activeTeamId === id ? (next[0]?.id ?? null) : s.activeTeamId;
      return { teams: next, activeTeamId: activeId };
    }),

  addMember: (teamId, agentId, role = 'member') =>
    set((s) => ({
      teams: s.teams.map((t) => {
        if (t.id !== teamId) return t;
        if (t.members.some((m) => m.agentId === agentId)) return t;
        const newMember: TeamMember = { agentId, role, order: t.members.length };
        return { ...t, members: [...t.members, newMember] };
      }),
    })),

  removeMember: (teamId, agentId) =>
    set((s) => ({
      teams: s.teams.map((t) =>
        t.id === teamId
          ? { ...t, members: t.members.filter((m) => m.agentId !== agentId) }
          : t,
      ),
    })),

  updateMemberRole: (teamId, agentId, role) =>
    set((s) => ({
      teams: s.teams.map((t) =>
        t.id === teamId
          ? { ...t, members: t.members.map((m) => (m.agentId === agentId ? { ...m, role } : m)) }
          : t,
      ),
    })),
}));
