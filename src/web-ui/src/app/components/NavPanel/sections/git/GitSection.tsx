/**
 * GitSection — inline sub-list under the "Git" nav item (like SessionsSection).
 * Items: Working copy / Branches / Graph; clicking one opens the Git scene and sets the active view.
 */

import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { GitBranch, Layers2 } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useGitSceneStore, type GitSceneView } from '../../../../scenes/git/gitSceneStore';
import { useApp } from '../../../../hooks/useApp';
import './GitSection.scss';

const GIT_VIEWS: { id: GitSceneView; icon: React.ElementType; labelKey: string }[] = [
  { id: 'working-copy', icon: GitBranch, labelKey: 'tabs.changes' },
  { id: 'branches', icon: Layers2, labelKey: 'tabs.branches' },
  { id: 'graph', icon: Layers2, labelKey: 'tabs.branchGraph' },
];

const GitSection: React.FC = () => {
  const { t } = useTranslation('panels/git');
  const activeTabId = useSceneStore(s => s.activeTabId);
  const openScene = useSceneStore(s => s.openScene);
  const activeView = useGitSceneStore(s => s.activeView);
  const setActiveView = useGitSceneStore(s => s.setActiveView);
  const { switchLeftPanelTab } = useApp();

  const handleSelect = useCallback(
    (view: GitSceneView) => {
      openScene('git');
      setActiveView(view);
      switchLeftPanelTab('git');
    },
    [openScene, setActiveView, switchLeftPanelTab]
  );

  return (
    <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--git">
      {GIT_VIEWS.map(({ id, icon: Icon, labelKey }) => {
        const label = t(labelKey);
        return (
          <Tooltip key={id} content={label} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__inline-item',
                activeTabId === 'git' && activeView === id && 'is-active',
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

export default GitSection;
