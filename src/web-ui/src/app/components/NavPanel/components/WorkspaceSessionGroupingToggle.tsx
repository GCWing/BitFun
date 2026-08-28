import React from 'react';
import {
  BITFUN_ICON_SIZE,
  NavigationSessionViewAllIcon,
  NavigationSessionViewGroupedIcon,
} from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { Tooltip } from '@bitfun/ui';
import {
  getNextWorkspaceSessionGrouping,
  useWorkspaceSessionViewStore,
} from '../workspaceSessionView';

const WorkspaceSessionGroupingToggle: React.FC = () => {
  const { t } = useI18n('common');
  const grouping = useWorkspaceSessionViewStore(state => state.grouping);
  const setGrouping = useWorkspaceSessionViewStore(state => state.setGrouping);
  const isAll = grouping === 'all';
  const actionTooltip = t(`nav.sessions.viewToggle.${isAll ? 'grouped' : 'all'}Tooltip`);
  const ViewIcon = isAll
    ? NavigationSessionViewAllIcon
    : NavigationSessionViewGroupedIcon;

  return (
    <Tooltip
      content={actionTooltip}
      placement="right"
      followCursor
    >
      <button
        type="button"
        className="bitfun-nav-panel__section-action bitfun-nav-panel__session-view-toggle"
        aria-label={t('nav.sessions.viewToggle.allTooltip')}
        aria-pressed={isAll}
        data-bf-action="toggle-session-view"
        data-bf-component="session-navigation"
        data-bf-part="viewToggle"
        data-bf-state={grouping}
        data-testid="nav-workspace-session-view-toggle"
        data-view-mode={grouping}
        onClick={() => setGrouping(getNextWorkspaceSessionGrouping(grouping))}
      >
        <ViewIcon size={BITFUN_ICON_SIZE.navigation} aria-hidden="true" />
      </button>
    </Tooltip>
  );
};

export default WorkspaceSessionGroupingToggle;
