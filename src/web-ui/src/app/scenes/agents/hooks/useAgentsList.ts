import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { SubagentAPI } from '@/infrastructure/api/service-api/SubagentAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import type { ModeConfigItem, ModeSkillInfo } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import type { AgentWithCapabilities } from '../agentsStore';
import { enrichCapabilities } from '../utils';
import { isAgentInOverviewZone } from '../agentVisibility';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';

export type FilterLevel = 'all' | 'builtin' | 'user' | 'project';
export type FilterType = 'all' | 'mode' | 'subagent';

export interface ToolInfo {
  name: string;
  description: string;
  is_readonly: boolean;
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
      const [modes, subagents, tools, configs] = await Promise.all([
        agentAPI.getAvailableModes().catch(() => []),
        SubagentAPI.listSubagents({ workspacePath: workspacePath || undefined }).catch(() => []),
        fetchTools(),
        configAPI.getModeConfigs().catch(() => ({})),
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
      if (requestId !== loadRequestIdRef.current) {
        return;
      }

      const modeAgents: AgentWithCapabilities[] = modes.map((mode) =>
        enrichCapabilities({
          id: mode.id,
          name: mode.name,
          description: mode.description,
          isReadonly: mode.isReadonly,
          isReview: false,
          toolCount: mode.toolCount,
          defaultTools: mode.defaultTools ?? [],
          enabled: mode.enabled,
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
        enabled: true,
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
    () => allAgents.filter(isAgentInOverviewZone),
    [allAgents],
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
    counts,
    loadAgents,
    getModeConfig,
    handleSetTools,
    handleResetTools,
    handleSetSkills,
  };
}

export { enrichCapabilities };
