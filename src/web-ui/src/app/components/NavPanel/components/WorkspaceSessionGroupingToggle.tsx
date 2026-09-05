import React from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { IconButton, Tooltip } from '@openbitfun/ui';
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
        className="openbitfun-nav-panel__section-action openbitfun-nav-panel__session-view-toggle"
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
        data-openbitfun-action="toggle-session-view"
        data-openbitfun-component="session-navigation"
        data-openbitfun-part="viewToggle"
        data-openbitfun-state={grouping}
        data-testid="nav-workspace-session-view-toggle"
        data-view-mode={grouping}
        onClick={() => setGrouping(getNextWorkspaceSessionGrouping(grouping))}
      />
    </Tooltip>
  );
};

export default WorkspaceSessionGroupingToggle;
