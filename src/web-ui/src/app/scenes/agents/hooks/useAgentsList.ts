import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { agentAPI, type ModeInfo } from '@/infrastructure/api/service-api/AgentAPI';
import type { AgentSource } from '@/infrastructure/api/service-api/CustomAgentAPI';
import { SubagentAPI, type SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import type {
  AgentModelDefaultsConfig,
  AgentProfileConfigItem,
  AIModelConfig,
  DefaultModelsConfig,
  ModeSkillInfo,
  SubagentModelSelection,
} from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';
import type { DynamicToolInfo } from '@/shared/types/agent-api';
import type { AgentWithCapabilities } from '../agentsStore';
import { enrichCapabilities } from '../utils';
import { HIDDEN_AGENT_IDS, isAgentInOverviewZone } from '../agentVisibility';
import { useCurrentWorkspace } from '@/infrastructure/contexts/WorkspaceContext';
import { loadDefaultReviewTeamDefinition } from '@/shared/services/reviewTeamService';
import { globalEventBus } from '@/infrastructure/event-bus';
import { isRemoteWorkspace } from '@/shared/types';

export type FilterLevel = 'all' | 'builtin' | 'user' | 'project' | 'external';
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

interface ModeProfileEntry {
  profileId: string;
  profileLabel?: string;
  memberModeIds: string[];
  representativeModeId: string;
}

function modeProfileIdFor(mode: Pick<ModeInfo, 'id' | 'configProfileId'>): string {
  return mode.configProfileId || mode.id;
}

function buildProfileMap(modes: ModeInfo[]): Record<string, ModeProfileEntry> {
  const profiles = new Map<string, ModeProfileEntry>();

  for (const mode of modes) {
    const profileId = modeProfileIdFor(mode);
    const existing = profiles.get(profileId);
    const memberModeIds = mode.configProfileMemberModeIds?.length
      ? mode.configProfileMemberModeIds
      : [mode.id];

    if (existing) {
      existing.memberModeIds = Array.from(new Set([...existing.memberModeIds, ...memberModeIds]));
      continue;
    }

    profiles.set(profileId, {
      profileId,
      profileLabel: mode.configProfileLabel,
      memberModeIds: [...memberModeIds],
      representativeModeId: mode.id,
    });
  }

  return Object.fromEntries(profiles.entries());
}

function buildModeConfigsByProfile(
  modes: ModeInfo[],
  configs: Record<string, AgentProfileConfigItem>,
): Record<string, AgentProfileConfigItem> {
  const byProfile: Record<string, AgentProfileConfigItem> = {};

  for (const mode of modes) {
    const config = configs[mode.id];
    if (!config) {
      continue;
    }
    byProfile[modeProfileIdFor(mode)] = config;
  }

  return byProfile;
}

function resolveAgentSource(
  agent: Pick<AgentWithCapabilities, 'source' | 'subagentSource'>,
): AgentSource {
  return agent.source ?? agent.subagentSource ?? 'builtin';
}

function configuredModelName(
  models: AIModelConfig[],
  modelId: string | null | undefined,
): string | undefined {
  if (!modelId?.trim()) {
    return undefined;
  }

  const model = models.find((candidate) => candidate.id === modelId);
  return model?.model_name?.trim() || model?.name?.trim() || model?.id;
}

function subagentModelOverride(
  subagent: SubagentInfo,
  builtinOverrides: Record<string, SubagentModelSelection>,
): SubagentModelSelection | undefined {
  const source = subagent.subagentSource ?? subagent.source;
  if (source === 'builtin') {
    return builtinOverrides[subagent.id];
  }

  if (!subagent.modelIsExplicit || !subagent.model?.trim()) {
    return undefined;
  }

  return subagent.model.trim() === 'inherit'
    ? { kind: 'inherit' }
    : { kind: 'fixed', model_id: subagent.model.trim() };
}

function subagentModelDisplayName(
  selection: SubagentModelSelection | undefined,
  models: AIModelConfig[],
  defaultModels: DefaultModelsConfig,
): string | undefined {
  if (!selection || selection.kind === 'inherit') {
    return undefined;
  }

  const modelId = selection.model_id.trim();
  if (!modelId) {
    return undefined;
  }

  if (modelId === 'primary') {
    return configuredModelName(models, defaultModels.primary) ?? modelId;
  }

  if (modelId === 'fast') {
    return configuredModelName(models, defaultModels.fast)
      ?? configuredModelName(models, defaultModels.primary)
      ?? modelId;
  }

  return configuredModelName(models, modelId) ?? modelId;
}

export function useAgentsList({
  searchQuery,
  filterLevel,
  filterType,
  t,
}: UseAgentsListOptions) {
  const notification = useNotification();
  const { workspace, workspacePath } = useCurrentWorkspace();
  const [allAgents, setAllAgents] = useState<AgentWithCapabilities[]>([]);
  const [loading, setLoading] = useState(true);
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [configuredModels, setConfiguredModels] = useState<AIModelConfig[]>([]);
  const [modeProfiles, setModeProfiles] = useState<Record<string, ModeProfileEntry>>({});
  const [agentSkills, setAgentSkills] = useState<Record<string, ModeSkillInfo[]>>({});
  const [modeConfigs, setModeConfigs] = useState<Record<string, AgentProfileConfigItem>>({});
  const [modeManageableSubagents, setModeManageableSubagents] = useState<Record<string, SubagentInfo[]>>({});
  const [hiddenAgentIds, setHiddenAgentIds] = useState<ReadonlySet<string>>(
    () => new Set(HIDDEN_AGENT_IDS),
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
      const [modes, subagents, tools, configs, reviewTeamDefinition, modelConfigs] = await Promise.all([
        agentAPI.getAvailableModes().catch(() => []),
        SubagentAPI.listSubagents({ workspacePath: workspacePath || undefined }).catch(() => []),
        fetchTools(),
        configAPI.getAgentProfileConfigs().catch(() => ({})),
        loadDefaultReviewTeamDefinition().catch(() => undefined),
        configAPI.getConfigs([
          'ai.models',
          'ai.default_models',
          'ai.agent_model_defaults',
        ]).catch((): Record<string, unknown> => ({})),
      ]);

      const profileMap = buildProfileMap(modes);
      const profileEntries = Object.values(profileMap);

      const skillTargets = [
        ...profileEntries.map((profile) => ({
          cacheKey: profile.profileId,
          agentId: profile.representativeModeId,
        })),
        ...subagents
          .filter((subagent) => subagent.defaultTools.includes('Skill'))
          .map((subagent) => ({
            cacheKey: subagent.id,
            agentId: subagent.id,
          })),
      ];
      const skillEntries = await Promise.all(
        skillTargets.map(async ({ cacheKey, agentId }) => [
          cacheKey,
          await configAPI.getModeSkillConfigs({
            modeId: agentId,
            workspacePath: workspacePath || undefined,
          }).catch(() => []),
        ] as const),
      );
      const manageableSubagentEntries = await Promise.all(
        profileEntries.map(async (profile) => [
          profile.profileId,
          await SubagentAPI.listManageableSubagents({
            parentAgentType: profile.representativeModeId,
            workspacePath: workspacePath || undefined,
          }).catch(() => []),
        ] as const),
      );

      if (requestId !== loadRequestIdRef.current) {
        return;
      }

      const manageableSubagentsByProfile = Object.fromEntries(manageableSubagentEntries);
      const models = (modelConfigs['ai.models'] as AIModelConfig[] | undefined) ?? [];
      const defaultModels = (
        modelConfigs['ai.default_models'] as DefaultModelsConfig | undefined
      ) ?? {};
      const builtinOverrides = (
        modelConfigs['ai.agent_model_defaults'] as AgentModelDefaultsConfig | undefined
      )?.subagents?.builtin ?? {};

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
          source: mode.source,
          path: mode.path,
          model: mode.model,
          configProfileId: mode.configProfileId,
          configProfileLabel: mode.configProfileLabel,
          configProfileMemberModeIds: mode.configProfileMemberModeIds,
          visibleSubagentCount: manageableSubagentsByProfile[mode.configProfileId]
            ?.filter((subagent) => subagent.effectiveEnabled).length ?? 0,
          capabilities: [],
          agentKind: 'mode',
        }),
      );

      const subAgents: AgentWithCapabilities[] = subagents.map((subagent) => {
        const modelOverride = subagentModelOverride(subagent, builtinOverrides);

        return enrichCapabilities({
          ...subagent,
          capabilities: [],
          agentKind: 'subagent',
          subagentModelOverride: modelOverride,
          subagentModelDisplayName: subagentModelDisplayName(modelOverride, models, defaultModels),
        });
      });

      setAllAgents([...modeAgents, ...subAgents]);
      setAvailableTools(tools);
      setConfiguredModels(models);
      setModeProfiles(profileMap);
      setAgentSkills(Object.fromEntries(skillEntries));
      setModeConfigs(buildModeConfigsByProfile(modes, configs as Record<string, AgentProfileConfigItem>));
      setModeManageableSubagents(manageableSubagentsByProfile);
      setHiddenAgentIds(new Set([
        ...HIDDEN_AGENT_IDS,
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

  useEffect(() => {
    const handleCustomAgentUpdated = () => {
      void loadAgents();
    };

    globalEventBus.on('custom-agent:updated', handleCustomAgentUpdated);
    return () => {
      globalEventBus.off('custom-agent:updated', handleCustomAgentUpdated);
    };
  }, [loadAgents]);

  const getModeProfile = useCallback((agentId: string): ModeProfileEntry | null => {
    const agent = allAgents.find((item) => item.id === agentId && item.agentKind === 'mode');
    if (!agent) {
      return null;
    }

    const profileId = agent.configProfileId ?? agentId;
    return modeProfiles[profileId] ?? {
      profileId,
      profileLabel: agent.configProfileLabel,
      memberModeIds: agent.configProfileMemberModeIds ?? [agentId],
      representativeModeId: agentId,
    };
  }, [allAgents, modeProfiles]);

  const getModeConfig = useCallback((agentId: string): AgentProfileConfigItem | null => {
    const agent = allAgents.find((item) => item.id === agentId && item.agentKind === 'mode');
    if (!agent) return null;

    const profileId = agent.configProfileId ?? agentId;
    const userConfig = modeConfigs[profileId];
    const defaultTools = agent.defaultTools ?? [];

    if (!userConfig) {
      return {
        profile_id: agent.configProfileId ?? agentId,
        enabled_tools: defaultTools,
        default_tools: defaultTools,
      };
    }

    return {
      ...userConfig,
      profile_id: profileId,
      default_tools: userConfig.default_tools ?? defaultTools,
    };
  }, [allAgents, modeConfigs]);

  const getAgentSkills = useCallback((agentId: string): ModeSkillInfo[] => {
    const profile = getModeProfile(agentId);
    return agentSkills[profile?.profileId ?? agentId] ?? [];
  }, [agentSkills, getModeProfile]);

  const getModeManageableSubagents = useCallback((agentId: string): SubagentInfo[] => {
    const profile = getModeProfile(agentId);
    return profile ? (modeManageableSubagents[profile.profileId] ?? []) : [];
  }, [getModeProfile, modeManageableSubagents]);

  const saveModeConfig = useCallback(async (agentId: string, updates: Partial<AgentProfileConfigItem>) => {
    const config = getModeConfig(agentId);
    const profile = getModeProfile(agentId);
    if (!config || !profile) return;

    const updated = { ...config, ...updates };
    await configAPI.setAgentProfileConfig(profile.representativeModeId, updated);
    setModeConfigs((prev) => ({ ...prev, [profile.profileId]: updated }));

    try {
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch {
      // ignore
    }
  }, [getModeConfig, getModeProfile]);

  const handleSetTools = useCallback(async (agentId: string, toolNames: string[]) => {
    try {
      const nextTools = Array.from(new Set(toolNames));
      await saveModeConfig(agentId, { enabled_tools: nextTools });
    } catch {
      notification.error(t('agentsOverview.toolToggleFailed'));
    }
  }, [notification, saveModeConfig, t]);

  const handleResetTools = useCallback(async (agentId: string) => {
    const profile = getModeProfile(agentId);
    if (!profile) return;

    try {
      await configAPI.resetAgentProfileConfig(profile.representativeModeId);
      const updated = await configAPI.getAgentProfileConfigs();
      const updatedSkills = await configAPI.getModeSkillConfigs({
        modeId: profile.representativeModeId,
        workspacePath: workspacePath || undefined,
      });
      const modes = await agentAPI.getAvailableModes().catch(() => []);
      setModeConfigs(buildModeConfigsByProfile(modes, updated as Record<string, AgentProfileConfigItem>));
      setAgentSkills((prev) => ({ ...prev, [profile.profileId]: updatedSkills }));
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
  }, [getModeProfile, notification, t, workspacePath]);

  const handleSetSkills = useCallback(async (agentId: string, enabledSkillKeys: string[]) => {
    const profile = getModeProfile(agentId);
    const cacheKey = profile?.profileId ?? agentId;
    const targetAgentId = profile?.representativeModeId ?? agentId;

    try {
      await configAPI.replaceModeSkillSelection({
        modeId: targetAgentId,
        enabledSkillKeys,
        workspacePath: workspacePath || undefined,
      });

      const updatedSkills = await configAPI.getModeSkillConfigs({
        modeId: targetAgentId,
        workspacePath: workspacePath || undefined,
      });
      setAgentSkills((prev) => ({ ...prev, [cacheKey]: updatedSkills }));

      try {
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('mode:config:updated');
      } catch {
        // ignore
      }
    } catch {
      notification.error(t('agentsOverview.skillToggleFailed'));
    }
  }, [getModeProfile, notification, t, workspacePath]);

  const handleResetSkills = useCallback(async (agentId: string) => {
    const profile = getModeProfile(agentId);
    const cacheKey = profile?.profileId ?? agentId;
    const targetAgentId = profile?.representativeModeId ?? agentId;

    try {
      await configAPI.resetModeSkillSelection({
        modeId: targetAgentId,
        workspacePath: workspacePath || undefined,
      });

      const updatedSkills = await configAPI.getModeSkillConfigs({
        modeId: targetAgentId,
        workspacePath: workspacePath || undefined,
      });
      setAgentSkills((prev) => ({ ...prev, [cacheKey]: updatedSkills }));

      try {
        const { globalEventBus } = await import('@/infrastructure/event-bus');
        globalEventBus.emit('mode:config:updated');
      } catch {
        // ignore
      }
    } catch {
      notification.error(t('agentsOverview.skillToggleFailed'));
    }
  }, [getModeProfile, notification, t, workspacePath]);

  const handleSetSubagentEnabled = useCallback(async (
    agentId: string,
    subagentId: string,
    enabled: boolean,
  ) => {
    const profile = getModeProfile(agentId);
    if (!profile) return;

    try {
      await SubagentAPI.updateSubagentConfig({
        subagentId,
        parentAgentType: agentId,
        enabled,
        workspacePath: workspacePath || undefined,
      });

      const updatedSubagents = await SubagentAPI.listManageableSubagents({
        parentAgentType: profile.representativeModeId,
        workspacePath: workspacePath || undefined,
      }).catch(() => []);

      setModeManageableSubagents((prev) => ({
        ...prev,
        [profile.profileId]: updatedSubagents,
      }));
      setAllAgents((prev) => prev.map((agent) => (
        agent.agentKind === 'mode' && (agent.configProfileId ?? agent.id) === profile.profileId
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
  }, [getModeProfile, notification, t, workspacePath]);

  const handleSetSubagentModel = useCallback(async (
    subagentId: string,
    selection: SubagentModelSelection | undefined,
  ) => {
    try {
      await SubagentAPI.updateSubagentConfig({
        subagentId,
        model: selection
          ? (selection.kind === 'inherit' ? 'inherit' : selection.model_id)
          : undefined,
        clearModelOverride: !selection,
        workspacePath: workspacePath || undefined,
      });
      await loadAgents();
    } catch {
      notification.error(t('agentCard.modelSelector.updateFailed'));
    }
  }, [loadAgents, notification, t, workspacePath]);

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
      const level = resolveAgentSource(agent);
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
    builtin: overviewAgents.filter((agent) => resolveAgentSource(agent) === 'builtin').length,
    user: overviewAgents.filter((agent) => resolveAgentSource(agent) === 'user').length,
    project: overviewAgents.filter((agent) => resolveAgentSource(agent) === 'project').length,
    external: overviewAgents.filter((agent) => resolveAgentSource(agent) === 'external').length,
    mode: overviewAgents.filter((agent) => agent.agentKind === 'mode').length,
    subagent: overviewAgents.filter((agent) => agent.agentKind === 'subagent').length,
  }), [overviewAgents]);

  return {
    workspacePath,
    workspaceIsRemote: isRemoteWorkspace(workspace),
    allAgents,
    filteredAgents,
    loading,
    availableTools,
    configuredModels,
    getModeProfile,
    getAgentSkills,
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
    handleSetSubagentModel,
  };
}

export { enrichCapabilities };
