/**
 * GroupLogView — R-WF-14 read-only group timeline.
 *
 * Group sessions open as a read-only bubble timeline: the same history
 * source (get_group_history) and the same FlowChat bubble pipeline as
 * GroupChatView, but with NO composer, NO member table and NO interactive
 * actions (TypeContract §R-WF-14: no input, no member table, no interaction).
 *
 * Reuse rules:
 * - History projection = groupMessageToDialogTurn (shared module, single
 *   source of truth for the group message wire; GroupChatView uses the same
 *   projection so the two views cannot drift).
 * - Bubble list = existing ModernFlowChatContainer (same as GroupChatView).
 * - Retry affordance reuses the existing `nav.sessions.loading` /
 *   `nav.groupChats.historyLoadFailed` / `actions.retry` i18n keys.
 * - Styles = existing appearance tokens only (GroupLogView.scss mirrors the
 *   GroupChatView body scaffold minus the input bar).
 */

import React, { useCallback, useEffect, useState } from 'react';
import { ModernFlowChatContainer as FlowChatContainer } from '../../../flow_chat/components/modern/ModernFlowChatContainer';
import { flowChatStore } from '../../../flow_chat/store/FlowChatStore';
import { useI18n } from '@/infrastructure/i18n';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { createLogger } from '@/shared/utils/logger';
import { groupMessageToDialogTurn } from './groupMessageProjection';
import './GroupLogView.scss';

const log = createLogger('GroupLogView');

const HISTORY_LIMIT = 200;

interface GroupLogViewProps {
  /** Group session id (== session id). */
  groupId: string;
  /** Group session workspace rootPath. */
  workspacePath: string;
  /** Whether the view is the active scene (passed to FlowChatContainer for virtualization/scroll). */
  isSceneActive?: boolean;
}

/**
 * Read-only group log: injects the persisted group history into the shared
 * FlowChat bubble pipeline (flowChatStore.addDialogTurn) and renders it with
 * the existing ModernFlowChatContainer. No ChatInput, no member management,
 * no fork/invite — the view is a pure timeline.
 */
export const GroupLogView: React.FC<GroupLogViewProps> = ({
  groupId,
  workspacePath,
  isSceneActive = true,
}) => {
  const { t } = useI18n('common');
  const [isLoadingHistory, setIsLoadingHistory] = useState(false);
  const [historyFailed, setHistoryFailed] = useState(false);
  const [, forceRender] = useState(0);

  const loadHistory = useCallback(async () => {
    if (!groupId) return;
    setIsLoadingHistory(true);
    setHistoryFailed(false);
    try {
      const response = await toolAPI.executeTool({
        toolName: 'get_group_history',
        parameters: { action: 'history', group_id: groupId, limit: HISTORY_LIMIT },
        workspacePath,
      });
      const messages = response?.result?.messages;
      if (response?.success === true && Array.isArray(messages)) {
        // Inject in chronological order (backend returns time-ordered); turns
        // already in the local store are skipped (addDialogTurn dedups by id).
        for (const message of messages as Array<Parameters<typeof groupMessageToDialogTurn>[0]>) {
          if (!message || typeof message.content !== 'string') continue;
          flowChatStore.addDialogTurn(groupId, groupMessageToDialogTurn(message, groupId));
        }
      } else {
        log.warn('get_group_history returned an unexpected response', {
          success: response?.success,
          error: response?.error || response?.validation_error,
        });
        setHistoryFailed(true);
      }
    } catch (error) {
      log.warn('Failed to load group history', { groupId, error });
      setHistoryFailed(true);
    } finally {
      setIsLoadingHistory(false);
      forceRender(v => v + 1);
    }
  }, [groupId, workspacePath]);

  useEffect(() => {
    void loadHistory();
  }, [loadHistory]);

  const emptyState = React.useMemo(
    () => (
      <div className="group-log-view__empty" data-bf-component="group-log-view" data-bf-part="emptyState">
        {t('nav.groupChats.viewHint')}
      </div>
    ),
    [t],
  );

  return (
    <div
      className="group-log-view"
      data-bf-component="group-log-view"
      data-bf-part="root"
      data-testid="group-log-view"
      data-group-id={groupId}
    >
      <div className="group-log-view__body" data-bf-component="group-log-view" data-bf-part="body">
        {isLoadingHistory && !flowChatStore.getState().sessions.get(groupId)?.dialogTurns.length ? (
          <div className="group-log-view__state">{t('nav.sessions.loading')}</div>
        ) : historyFailed && !flowChatStore.getState().sessions.get(groupId)?.dialogTurns.length ? (
          <div className="group-log-view__state">
            {t('nav.groupChats.historyLoadFailed')}
            <button
              type="button"
              className="group-log-view__retry"
              onClick={() => { void loadHistory(); }}
            >
              {t('actions.retry')}
            </button>
          </div>
        ) : (
          <FlowChatContainer
            className="group-log-view__chat-container"
            isViewportActive={isSceneActive}
            emptyState={emptyState}
            onOpenVisualization={() => {}}
            onFileViewRequest={() => {}}
            onTabOpen={() => {}}
            onSwitchToChatPanel={() => {}}
            config={{ enableMarkdown: true, autoScroll: true, showTimestamps: false }}
          />
        )}
      </div>
    </div>
  );
};

export default GroupLogView;
