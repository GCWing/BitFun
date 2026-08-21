import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useUserToolGroups } from '@/app/scenes/agents/components/useUserToolGroups';
import ToolSuiteView from '@/app/scenes/agents/components/ToolSuiteView';
import type { AgentProfileConfigItem } from '@/infrastructure/config/types';
import { configAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import './ToolsScene.scss';

const log = createLogger('ToolsScene');

interface ModeConfigView {
  enabled_tools: string[];
  default_tools: string[];
}

const ToolsScene: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const { groups: userToolGroups, saveGroups: saveUserToolGroups } = useUserToolGroups();
  const [tools, setTools] = useState<Array<{
    name: string;
    description: string;
    is_readonly: boolean;
  }>>([]);
  const [modeConfigs, setModeConfigs] = useState<Record<string, ModeConfigView>>({});
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    const load = async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const [allTools, configs] = await Promise.all([
          invoke<Array<{ name: string; description: string; is_readonly: boolean }>>('get_all_tools_info'),
          configAPI.getAgentProfileConfigs().catch(() => ({})),
        ]);
        if (!active) return;
        setTools(allTools);
        const view: Record<string, ModeConfigView> = {};
        const configsRecord = configs as Record<string, AgentProfileConfigItem>;
        for (const [profileId, config] of Object.entries(configsRecord)) {
          view[profileId] = {
            enabled_tools: config.enabled_tools ?? [],
            default_tools: config.default_tools ?? [],
          };
        }
        setModeConfigs(view);
      } catch (error) {
        log.error('Failed to load tool suite data', { error });
      } finally {
        if (active) setLoading(false);
      }
    };
    void load();
    return () => { active = false; };
  }, []);

  const getModeConfig = useMemo(() => {
    return (modeId: string): ModeConfigView | null => {
      const profileId = modeId;
      const config = modeConfigs[profileId];
      if (!config) {
        return { enabled_tools: [], default_tools: [] };
      }
      return config;
    };
  }, [modeConfigs]);

  if (loading) {
    return <div className="bitfun-tools-scene" data-bf-scene="tools" data-bf-part="root"><span>{t('suite.loading')}</span></div>;
  }

  return (
    <div className="bitfun-tools-scene" data-testid="agent-skill-panel" data-bf-scene="tools" data-bf-part="root">
      <ToolSuiteView
        tools={tools}
        getModeConfig={getModeConfig}
        userGroups={userToolGroups}
        onSaveUserGroups={saveUserToolGroups}
      />
    </div>
  );
};

export default ToolsScene;
