/**
 * TeamSection — inline sub-list under the "Team" nav item (like GitSection).
 * Items: Agents / Expert teams; clicking one opens the Team scene and sets the active view.
 */

import React, { useCallback } from 'react';
import { Bot, Users } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { useSceneStore } from '../../../../stores/sceneStore';
import { useTeamStore, type TeamScenePage } from '../../../../scenes/team/teamStore';
import { useApp } from '../../../../hooks/useApp';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';

const TEAM_VIEWS: { id: TeamScenePage; icon: React.ElementType; labelKey: string }[] = [
  { id: 'agentsOverview', icon: Bot, labelKey: 'nav.team.agentsOverview' },
  { id: 'expertTeamsOverview', icon: Users, labelKey: 'nav.team.expertTeams' },
];

const TeamSection: React.FC = () => {
  const { t } = useI18n('common');
  const activeTabId = useSceneStore((s) => s.activeTabId);
  const openScene = useSceneStore((s) => s.openScene);
  const page = useTeamStore((s) => s.page);
  const openAgentsOverview = useTeamStore((s) => s.openAgentsOverview);
  const openExpertTeamsOverview = useTeamStore((s) => s.openExpertTeamsOverview);
  const { switchLeftPanelTab } = useApp();

  const handleSelect = useCallback(
    (view: TeamScenePage) => {
      if (view === 'editor') return;
      openScene('team');
      if (view === 'agentsOverview') {
        openAgentsOverview();
      } else {
        openExpertTeamsOverview();
      }
      switchLeftPanelTab('team');
    },
    [openScene, openAgentsOverview, openExpertTeamsOverview, switchLeftPanelTab]
  );

  return (
    <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--team">
      {TEAM_VIEWS.map(({ id, icon: Icon, labelKey }) => {
        const label = t(labelKey);
        const isActive = activeTabId === 'team' && page === id;
        return (
          <Tooltip key={id} content={label} placement="right" followCursor>
            <button
              type="button"
              className={[
                'bitfun-nav-panel__inline-item',
                isActive && 'is-active',
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

export default TeamSection;
