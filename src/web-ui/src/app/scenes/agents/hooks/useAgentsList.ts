import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { SubagentAPI, type SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import type { ModeConfigItem, ModeSkillInfo } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import type { DynamicToolInfo } from '@/shared/types/agent-api';
import type { AgentWithCapabilities } from '../agentsStore';
import { enrichCapabilities } from '../utils';
import { STATIC_HIDDEN_AGENT_IDS, isAgentInOverviewZone } from '../agentVisibility';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { loadDefaultReviewTeamDefinition } from '@/shared/services/reviewTeamService';

export type FilterLevel = 'all' | 'builtin' | 'user' | 'project';
export type FilterType = 'all' | 'mode' | 'subagent';

export interface ToolInfo {
  name: string;
  description: string;
  is_readonly: boolean;
  dynamic_info?: DynamicToolInfo;
}

interface UseAgentsListOptions {
  searchQuery: string;
  filterLevel: FilterLevel;
  filterType: FilterType;
  t: TFunction<'scenes/agents'>;
}

export function useAgentsList({
  searchQuery,
  filterLevel,
  filterType,
  t,
}: UseAgentsListOptions) {
  const notification = useNotification();
  const { workspacePath } = useCurrentWorkspace();
  const [allAgents, setAllAgents] = useState<AgentWithCapabilities[]>([]);
  const [loading, setLoading] = useState(true);
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [modeSkills, setModeSkills] = useState<Record<string, ModeSkillInfo[]>>({});
  const [modeConfigs, setModeConfigs] = useState<Record<string, ModeConfigItem>>({});
  const [modeManageableSubagents, setModeManageableSubagents] = useState<Record<string, SubagentInfo[]>>({});
  const [hiddenAgentIds, setHiddenAgentIds] = useState<ReadonlySet<string>>(
    () => new Set(STATIC_HIDDEN_AGENT_IDS),
  );
  const loadRequestIdRef = useRef(0);

  const loadAgents = useCallback(async () => {
    const requestId = ++loadRequestIdRef.current;
    setLoading(true);

    const fetchTools = async (): Promise<ToolInfo[]> => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        return await invoke<ToolInfo[]>('get_all_tools_info');
      } catch {
        return [];
      }
    };

    try {
      const [modes, subagents, tools, configs, reviewTeamDefinition] = await Promise.all([
        agentAPI.getAvailableModes().catch(() => []),
        SubagentAPI.listSubagents({ workspacePath: workspacePath || undefined }).catch(() => []),
        fetchTools(),
        configAPI.getModeConfigs().catch(() => ({})),
        loadDefaultReviewTeamDefinition().catch(() => undefined),
      ]);
      const skillEntries = await Promise.all(
        modes.map(async (mode) => [
          mode.id,
          await configAPI.getModeSkillConfigs({
            modeId: mode.id,
            workspacePath: workspacePath || undefined,
          }).catch(() => []),
        ] as const),
      );
      const manageableSubagentEntries = await Promise.all(
        modes.map(async (mode) => [
          mode.id,
          await SubagentAPI.listManageableSubagents({
            parentAgentType: mode.id,
            workspacePath: workspacePath || undefined,
          }).catch(() => []),
        ] as const),
      );
      if (requestId !== loadRequestIdRef.current) {
        return;
      }

      const manageableSubagentsByMode = Object.fromEntries(manageableSubagentEntries);

      const modeAgents: AgentWithCapabilities[] = modes.map((mode) =>
        enrichCapabilities({
          key: `mode::${mode.id}`,
          id: mode.id,
          name: mode.name,
          description: mode.description,
          isReadonly: mode.isReadonly,
          isReview: false,
          toolCount: mode.toolCount,
          defaultTools: mode.defaultTools ?? [],
          defaultEnabled: true,
          effectiveEnabled: true,
          visibleSubagentCount: manageableSubagentsByMode[mode.id]
            ?.filter((subagent) => subagent.effectiveEnabled).length ?? 0,
          capabilities: [],
          agentKind: 'mode',
        }),
      );

      const subAgents: AgentWithCapabilities[] = subagents.map((subagent) =>
        enrichCapabilities({
          ...subagent,
          capabilities: [],
          agentKind: 'subagent',
        }),
      );

      setAllAgents([...modeAgents, ...subAgents]);
      setAvailableTools(tools);
      setModeSkills(Object.fromEntries(skillEntries));
      setModeConfigs(configs as Record<string, ModeConfigItem>);
      setModeManageableSubagents(manageableSubagentsByMode);
      setHiddenAgentIds(new Set([
        ...STATIC_HIDDEN_AGENT_IDS,
        ...(reviewTeamDefinition?.hiddenAgentIds ?? []),
      ]));
    } finally {
      if (requestId === loadRequestIdRef.current) {
        setLoading(false);
      }
    }
  }, [workspacePath]);

  useEffect(() => {
    void loadAgents();
  }, [loadAgents]);

  const getModeConfig = useCallback((agentId: string): ModeConfigItem | null => {
    const agent = allAgents.find((item) => item.id === agentId && item.agentKind === 'mode');
    if (!agent) return null;

    const userConfig = modeConfigs[agentId];
    const defaultTools = agent.defaultTools ?? [];

    if (!userConfig) {
      return {
        mode_id: agentId,
        enabled_tools: defaultTools,
        default_tools: defaultTools,
      };
    }

    return {
      ...userConfig,
      default_tools: userConfig.default_tools ?? defaultTools,
    };
  }, [allAgents, modeConfigs]);

  const getModeSkills = useCallback((agentId: string): ModeSkillInfo[] => {
    return modeSkills[agentId] ?? [];
  }, [modeSkills]);

  const getModeManageableSubagents = useCallback((agentId: string): SubagentInfo[] => {
    return modeManageableSubagents[agentId] ?? [];
  }, [modeManageableSubagents]);

  const saveModeConfig = useCallback(async (agentId: string, updates: Partial<ModeConfigItem>) => {
    const config = getModeConfig(agentId);
    if (!config) return;

    const updated = { ...config, ...updates };
    await configAPI.setModeConfig(agentId, updated);
    setModeConfigs((prev) => ({ ...prev, [agentId]: updated }));

    try {
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch {
      // ignore
    }
  }, [getModeConfig]);

  const handleSetTools = useCallback(async (agentId: string, toolNames: string[]) => {
    try {
      const nextTools = Array.from(new Set(toolNames));
      await saveModeConfig(agentId, { enabled_tools: nextTools });
    } catch {
      notification.error(t('agentsOverview.toolToggleFailed'));
    }
  }, [notification, saveModeConfig, t]);

  const handleResetTools = useCallback(async (agentId: string) => {
    try {
      await configAPI.resetModeConfig(agentId);
      const updated = await configAPI.getModeConfigs();
      const updatedSkills = await configAPI.getModeSkillConfigs({
        modeId: agentId,
        workspacePath: workspacePath || undefined,
      });
      setModeConfigs(updated as Record<string, ModeConfigItem>);
      setModeSkills((prev) => ({ ...prev, [agentId]: updatedSkills }));
      notification.success(t('agentsOverview.toolsResetSuccess'));

      try {
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('mode:config:updated');
      } catch {
        // ignore
      }
    } catch {
      notification.error(t('agentsOverview.toolsResetFailed'));
    }
  }, [notification, t, workspacePath]);

  const handleSetSkills = useCallback(async (agentId: string, enabledSkillKeys: string[]) => {
    try {
      await configAPI.replaceModeSkillSelection({
        modeId: agentId,
        enabledSkillKeys,
        workspacePath: workspacePath || undefined,
      });

      const updatedSkills = await configAPI.getModeSkillConfigs({
        modeId: agentId,
        workspacePath: workspacePath || undefined,
      });
      setModeSkills((prev) => ({ ...prev, [agentId]: updatedSkills }));

      try {
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('mode:config:updated');
      } catch {
        // ignore
      }
    } catch {
      notification.error(t('agentsOverview.skillToggleFailed'));
    }
  }, [notification, t, workspacePath]);

  const handleResetSkills = useCallback(async (agentId: string) => {
    try {
      await configAPI.resetModeSkillSelection({
        modeId: agentId,
        workspacePath: workspacePath || undefined,
      });

      const updatedSkills = await configAPI.getModeSkillConfigs({
        modeId: agentId,
        workspacePath: workspacePath || undefined,
      });
      setModeSkills((prev) => ({ ...prev, [agentId]: updatedSkills }));

      try {
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('mode:config:updated');
      } catch {
        // ignore
      }
    } catch {
      notification.error(t('agentsOverview.skillToggleFailed'));
    }
  }, [notification, t, workspacePath]);

  const handleSetSubagentEnabled = useCallback(async (
    agentId: string,
    subagentId: string,
    enabled: boolean,
  ) => {
    try {
      await SubagentAPI.updateSubagentConfig({
        subagentId,
        parentAgentType: agentId,
        enabled,
        workspacePath: workspacePath || undefined,
      });

      const updatedSubagents = await SubagentAPI.listManageableSubagents({
        parentAgentType: agentId,
        workspacePath: workspacePath || undefined,
      }).catch(() => []);

      setModeManageableSubagents((prev) => ({
        ...prev,
        [agentId]: updatedSubagents,
      }));
      setAllAgents((prev) => prev.map((agent) => (
        agent.agentKind === 'mode' && agent.id === agentId
          ? {
              ...agent,
              visibleSubagentCount: updatedSubagents.filter((subagent) => subagent.effectiveEnabled).length,
            }
          : agent
      )));

      try {
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('mode:config:updated');
      } catch {
        // ignore
      }
    } catch {
      notification.error(t('agentsOverview.subagentToggleFailed'));
    }
  }, [notification, t, workspacePath]);

  const filteredAgents = useMemo(() => allAgents.filter((agent) => {
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      if (!agent.name.toLowerCase().includes(query) && !agent.description.toLowerCase().includes(query)) {
        return false;
      }
    }

    if (filterType !== 'all') {
      if (filterType === 'mode' && agent.agentKind !== 'mode') return false;
      if (filterType === 'subagent' && agent.agentKind !== 'subagent') return false;
    }

    if (filterLevel !== 'all') {
      const level = agent.agentKind === 'mode' ? 'builtin' : (agent.subagentSource ?? 'builtin');
      if (level !== filterLevel) return false;
    }

    return true;
  }), [allAgents, filterLevel, filterType, searchQuery]);

  const overviewAgents = useMemo(
    () => allAgents.filter((agent) => isAgentInOverviewZone(agent, hiddenAgentIds)),
    [allAgents, hiddenAgentIds],
  );

  const counts = useMemo(() => ({
    all: overviewAgents.length,
    builtin: overviewAgents.filter((agent) => (agent.agentKind === 'mode' ? 'builtin' : (agent.subagentSource ?? 'builtin')) === 'builtin').length,
    user: overviewAgents.filter((agent) => agent.subagentSource === 'user').length,
    project: overviewAgents.filter((agent) => agent.subagentSource === 'project').length,
    mode: overviewAgents.filter((agent) => agent.agentKind === 'mode').length,
    subagent: overviewAgents.filter((agent) => agent.agentKind === 'subagent').length,
  }), [overviewAgents]);

  return {
    allAgents,
    filteredAgents,
    loading,
    availableTools,
    getModeSkills,
    getModeManageableSubagents,
    counts,
    hiddenAgentIds,
    loadAgents,
    getModeConfig,
    handleSetTools,
    handleResetTools,
    handleSetSkills,
    handleResetSkills,
    handleSetSubagentEnabled,
  };
}

export { enrichCapabilities };
