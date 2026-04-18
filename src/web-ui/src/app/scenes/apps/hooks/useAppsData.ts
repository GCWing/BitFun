import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import type { ModeConfigItem, ModeSkillInfo } from '@/infrastructure/config/types';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { useNotification } from '@/shared/notification-system';
import { APP_REGISTRY, type AppEntity, isPrimaryAgentMode } from '../appRegistry';
import { enrichAgentCapabilities } from '../appsUtils';

export const CAPABILITY_CATEGORIES = ['编码', '文档', '分析', '测试', '创意', '运维'] as const;
export type CapabilityCategory = (typeof CAPABILITY_CATEGORIES)[number];

export interface AgentCapability {
  category: CapabilityCategory;
  level: number;
}

export interface AgentWithCapabilities {
  id: string;
  name: string;
  description: string;
  isReadonly: boolean;
  toolCount?: number;
  defaultTools?: string[];
  enabled: boolean;
  model?: string;
  capabilities: AgentCapability[];
  iconKey?: string;
}

export interface ToolInfo {
  name: string;
  description: string;
  is_readonly: boolean;
}

export type AppCardModel = AppEntity & {
  includedAgents: AgentWithCapabilities[];
};

export function useAppsData(searchQuery: string) {
  const notification = useNotification();
  const { workspacePath } = useCurrentWorkspace();
  const [allAgents, setAllAgents] = useState<AgentWithCapabilities[]>([]);
  const [loading, setLoading] = useState(true);
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [modeSkills, setModeSkills] = useState<Record<string, ModeSkillInfo[]>>({});
  const [modeConfigs, setModeConfigs] = useState<Record<string, ModeConfigItem>>({});
  const loadRequestIdRef = useRef(0);

  const loadAppsData = useCallback(async () => {
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
      const [modes, tools, configs] = await Promise.all([
        agentAPI.getAvailableModes().catch(() => []),
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

      if (requestId !== loadRequestIdRef.current) return;

      const primaryAgents = modes
        .map((mode) => enrichAgentCapabilities({
          id: mode.id,
          name: mode.name,
          description: mode.description,
          isReadonly: mode.isReadonly,
          toolCount: mode.toolCount,
          defaultTools: mode.defaultTools ?? [],
          enabled: mode.enabled,
          model: undefined,
          capabilities: [],
        }))
        .filter((agent) => isPrimaryAgentMode({ id: agent.id, agentKind: 'mode' }));

      setAllAgents(primaryAgents);
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
    void loadAppsData();
  }, [loadAppsData]);

  const appCards = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();

    return APP_REGISTRY
      .map((app) => {
        const includedAgents = app.kind === 'mode-app'
          ? app.agentIds.map((id) => allAgents.find((agent) => agent.id === id)).filter(Boolean) as AgentWithCapabilities[]
          : [allAgents.find((agent) => agent.id === app.agentId)].filter(Boolean) as AgentWithCapabilities[];

        return {
          ...app,
          includedAgents,
        } satisfies AppCardModel;
      })
      .filter((app) => app.includedAgents.length > 0)
      .filter((app) => {
        if (!q) return true;
        return app.id.toLowerCase().includes(q);
      });
  }, [allAgents, searchQuery]);

  const getAgentById = useCallback((agentId: string | null) => {
    if (!agentId) return null;
    return allAgents.find((agent) => agent.id === agentId) ?? null;
  }, [allAgents]);

  const getAppById = useCallback((appId: string | null) => {
    if (!appId) return null;
    const app = APP_REGISTRY.find((item) => item.id === appId);
    if (!app) return null;

    const includedAgents = app.kind === 'mode-app'
      ? app.agentIds.map((id) => allAgents.find((agent) => agent.id === id)).filter(Boolean) as AgentWithCapabilities[]
      : [allAgents.find((agent) => agent.id === app.agentId)].filter(Boolean) as AgentWithCapabilities[];

    if (includedAgents.length === 0) return null;

    return {
      ...app,
      includedAgents,
    } satisfies AppCardModel;
  }, [allAgents]);

  const getModeConfig = useCallback((agentId: string): ModeConfigItem | null => {
    const agent = allAgents.find((item) => item.id === agentId);
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
      await saveModeConfig(agentId, { enabled_tools: Array.from(new Set(toolNames)) });
    } catch {
      notification.error('工具切换失败');
    }
  }, [notification, saveModeConfig]);

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

      try {
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('mode:config:updated');
      } catch {
        // ignore
      }
    } catch {
      notification.error('重置工具失败');
    }
  }, [notification, workspacePath]);

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
      notification.error('Skill 切换失败');
    }
  }, [notification, workspacePath]);

  return {
    allAgents,
    appCards,
    availableTools,
    getAgentById,
    getAppById,
    getModeConfig,
    getModeSkills,
    handleResetTools,
    handleSetSkills,
    handleSetTools,
    loadAppsData,
    loading,
  };
}
