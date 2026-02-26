/**
 * CapabilitiesSection — inline sub-list under the "Capabilities" nav item.
 * Items: Sub-agents / Skills / MCP; clicking one opens the Capabilities scene and sets the active view.
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Bot, Puzzle, Plug } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useCapabilitiesSceneStore, type CapabilitiesView } from '../../../../scenes/capabilities/capabilitiesSceneStore';
import { useApp } from '../../../../hooks/useApp';
import './CapabilitiesSection.scss';

const CAP_VIEWS: { id: CapabilitiesView; icon: React.ElementType; labelKey: string }[] = [
  { id: 'sub-agents', icon: Bot, labelKey: 'subagents' },
  { id: 'skills', icon: Puzzle, labelKey: 'skills' },
  { id: 'mcp', icon: Plug, labelKey: 'mcp' },
];

const CapabilitiesSection: React.FC = () => {
  const { t } = useTranslation('scenes/capabilities');
  const activeTabId = useSceneStore((s) => s.activeTabId);
  const openScene = useSceneStore((s) => s.openScene);
  const activeView = useCapabilitiesSceneStore((s) => s.activeView);
  const setActiveView = useCapabilitiesSceneStore((s) => s.setActiveView);
  const { switchLeftPanelTab } = useApp();

  const handleSelect = useCallback(
    (view: CapabilitiesView) => {
      openScene('capabilities');
      setActiveView(view);
      switchLeftPanelTab('capabilities');
    },
    [openScene, setActiveView, switchLeftPanelTab]
  );

  return (
    <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--capabilities">
      {CAP_VIEWS.map(({ id, icon: Icon, labelKey }) => {
        const label = t(labelKey);
        return (
          <Tooltip key={id} content={label} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__inline-item',
                activeTabId === 'capabilities' && activeView === id && 'is-active',
              ]
                .filter(Boolean)
                .join(' ')}
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

export default CapabilitiesSection;
