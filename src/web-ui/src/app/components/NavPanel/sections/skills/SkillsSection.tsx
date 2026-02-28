/**
 * SkillsSection — inline sub-list under the "Skills" nav item.
 * Shows two fixed entries: Market and Installed.
 * Clicking either opens the Skills scene and activates the corresponding view.
 */

import React, { useCallback } from 'react';
import { Store, Package } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useSkillsSceneStore, type SkillsView } from '../../../../scenes/skills/skillsSceneStore';
import { useI18n } from '@/infrastructure/i18n';
import './SkillsSection.scss';

const SKILLS_VIEWS: { id: SkillsView; Icon: React.ElementType; labelKey: string }[] = [
  { id: 'market',        Icon: Store,   labelKey: 'nav.items.market'          },
  { id: 'installed-all', Icon: Package, labelKey: 'nav.categories.installed'  },
];

const SkillsSection: React.FC = () => {
  const { t } = useI18n('scenes/skills');
  const activeTabId  = useSceneStore((s) => s.activeTabId);
  const openScene    = useSceneStore((s) => s.openScene);
  const activeView   = useSkillsSceneStore((s) => s.activeView);
  const setActiveView = useSkillsSceneStore((s) => s.setActiveView);

  const handleSelect = useCallback(
    (view: SkillsView) => {
      openScene('skills');
      setActiveView(view);
    },
    [openScene, setActiveView],
  );

  return (
    <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--skills">
      {SKILLS_VIEWS.map(({ id, Icon, labelKey }) => {
        const label    = t(labelKey);
        const isActive = activeTabId === 'skills' && activeView === id;
        return (
          <Tooltip key={id} content={label} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__inline-item',
                isActive && 'is-active',
              ].filter(Boolean).join(' ')}
              onClick={() => handleSelect(id)}
            >
              <Icon size={12} className="bitfun-nav-panel__inline-item-icon" aria-hidden />
              <span className="bitfun-nav-panel__inline-item-label">{label}</span>
            </button>
          </Tooltip>
        );
      })}
    </div>
  );
};

export default SkillsSection;
