import React from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { IconButton, Tooltip } from '@bitfun/ui';
import { List, ListTree } from 'lucide-react';
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
  const ViewIcon = isAll ? List : ListTree;

  return (
    <Tooltip
      content={actionTooltip}
      placement="right"
      followCursor
    >
      <IconButton
        className="bitfun-nav-panel__section-action bitfun-nav-panel__session-view-toggle"
        aria-label={actionTooltip}
        aria-pressed={isAll}
        icon={(
          <ViewIcon
            size={16}
            aria-hidden="true"
            data-session-view-icon={grouping}
          />
        )}
        size="xs"
        variant="quiet"
        data-bf-action="toggle-session-view"
        data-bf-component="session-navigation"
        data-bf-part="viewToggle"
        data-bf-state={grouping}
        data-testid="nav-workspace-session-view-toggle"
        data-view-mode={grouping}
        onClick={() => setGrouping(getNextWorkspaceSessionGrouping(grouping))}
      />
    </Tooltip>
  );
};

export default WorkspaceSessionGroupingToggle;
