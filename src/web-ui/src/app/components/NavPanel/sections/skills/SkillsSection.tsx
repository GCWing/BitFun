/**
 * SkillsSection — inline sub-list under the "Skills" nav item.
 * Fetches real skill data via configAPI and displays them.
 * Clicking a skill opens the Capabilities scene → skills view.
 */

import React, { useState, useEffect, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Puzzle } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useCapabilitiesSceneStore } from '../../../../scenes/capabilities/capabilitiesSceneStore';
import { configAPI } from '@/infrastructure/api';
import type { SkillInfo } from '@/infrastructure/config/types';

const SkillsSection: React.FC = () => {
  const { t } = useI18n('common');
  const activeTabId = useSceneStore((s) => s.activeTabId);
  const openScene = useSceneStore((s) => s.openScene);
  const activeView = useCapabilitiesSceneStore((s) => s.activeView);
  const setActiveView = useCapabilitiesSceneStore((s) => s.setActiveView);

  const [skills, setSkills] = useState<SkillInfo[]>([]);

  const load = useCallback(async () => {
    try {
      const list = await configAPI.getSkillConfigs();
      setSkills(list);
    } catch {
      // silent
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleClick = useCallback(() => {
    openScene('capabilities');
    setActiveView('skills');
  }, [openScene, setActiveView]);

  if (skills.length === 0) {
    return (
      <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--skills">
        <span className="bitfun-nav-panel__inline-empty">{t('empty.noData')}</span>
      </div>
    );
  }

  return (
    <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--skills">
      {skills.map((skill) => {
        const isActive = activeTabId === 'capabilities' && activeView === 'skills';
        const tooltipText = `${skill.name} (${skill.level})${skill.enabled ? '' : ` — ${t('status.disabled')}`}`;
        return (
          <Tooltip key={skill.name} content={tooltipText} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__inline-item',
                isActive && 'is-active',
                !skill.enabled && 'is-disabled',
              ].filter(Boolean).join(' ')}
              onClick={handleClick}
            >
              <Puzzle size={12} className="bitfun-nav-panel__inline-item-icon" aria-hidden />
              <span className="bitfun-nav-panel__inline-item-label">{skill.name}</span>
              {!skill.enabled && (
                <span className="bitfun-nav-panel__inline-item-badge bitfun-nav-panel__inline-item-badge--dim">off</span>
              )}
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
};

export default SkillsSection;
