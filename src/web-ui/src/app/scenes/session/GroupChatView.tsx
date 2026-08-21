/**
 * GroupChatView — group chat session view (R-GC-14 view + R-GC-15 member
 * management & fork).
 *
 * Reuse rules (type-contract section 4, top red line):
 * - Layout = the original session pane (zero hand-rolled bars, R-GC-24):
 *   - Top bar = the existing FlowChatHeader inside ModernFlowChatContainer
 *     (flow_chat/components/modern/ModernFlowChatContainer.tsx:2462-2488).
 *     The group-chat menu (members / invite / fork) is injected into the
 *     existing left action group via `headerLeftActionsContent`
 *     (FlowChatHeader.tsx:490-497), which already hosts SessionFilesBadge.
 *   - Bubble list = existing ModernFlowChatContainer
 *     (flow_chat/components/modern/ModernFlowChatContainer.tsx). History
 *     turns are injected via flowChatStore.addDialogTurn (FlowChatStore.ts:5084)
 *     and rendered by the existing UserMessageItem (senderBadge reads
 *     metadata.senderName/senderSessionId automatically, UserMessageItem.tsx:219).
 *   - Input = existing ChatInput + ChatInputRegistration.onSubmit host contract
 *     (chatInputRegistration.ts:34-60; ChatInput.tsx:5266-5282 explicitly names
 *     the registered-host send button). onSubmit calls
 *     toolAPI.executeTool({ toolName: 'send_group_message', ... })
 *     (ToolAPI.ts:49-61 — the single camelCase execute_tool wrapper).
 * - Member picker (invite/fork) reuses the component-library Select with
 *   multiple + searchable + showSelectAll (Select.tsx:87, exported from
 *   component-library index.ts:21) inside the existing Modal (Modal.tsx:65)
 *   with Button (Button.tsx:15) / Input (Input.tsx:20) actions — no custom
 *   list is built (R-GC-22 / R-GC-30).
 * - R-GC-30 (owner directive, direction corrected 2026-08-14): the owner
 *   picks invite/fork members themselves from a real optional session list —
 *   NO member-count input (R-GC-28 had wrongly added it). Member source =
 *   runtime-fetched real sessions (sessionAPI.listSessions per root;
 *   R-GC-R6 2026-08-15: agentType no longer filtered — every real session
 *   including agentic is a selectable member). The backend invite/fork
 *   validates each picked id exists and registers it in the group's
 *   groupChats (group_room_tools.rs invite_member / fork_group); no fresh
 *   member sessions are created.
 * - Jump to a forked child group reuses the R-GC-13 handleGroupChatCreated
 *   registration shape: flowChatStore.createSession (FlowChatStore.ts:3744) +
 *   markSessionAsGroupChat (FlowChatStore.ts:7075) + openMainSession
 *   (sessionActivation.ts:7).
 * - Every action goes through execute_tool; bare invoke('*_group_*') is forbidden.
 * - Styles = existing appearance tokens only.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Button, IconButton, Input, Modal, Select, type SelectOption } from '@/component-library';
import { UserPlus, GitBranch, Users } from 'lucide-react';
import { ModernFlowChatContainer as FlowChatContainer } from '../../../flow_chat/components/modern/ModernFlowChatContainer';
import { ChatInput } from '../../../flow_chat/components/ChatInput';
import type { ChatInputRegistration, ChatInputSubmission } from '../../../flow_chat/components/chatInputRegistration';
import { flowChatStore } from '../../../flow_chat/store/FlowChatStore';
import { openMainSession } from '../../../flow_chat/services/sessionActivation';
import type { DialogTurn } from '../../../flow_chat/types/flow-chat';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionMetadata } from '@/shared/types/session-history';
import type { WorkspaceInfo } from '@/shared/types';
import { useI18n } from '@/infrastructure/i18n';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { groupMessageToDialogTurn } from './groupMessageProjection';

const log = createLogger('GroupChatView');

const HISTORY_LIMIT = 200;

interface GroupChatViewProps {
  /** Group session id (== session id). */
  groupId: string;
  /** Group session workspace rootPath. */
  workspacePath: string;
  /** Current session name (header display). */
  groupName?: string;
  /** Whether the view is the active scene (passed to FlowChatContainer for virtualization/scroll). */
  isSceneActive?: boolean;
  /**
   * R-GC-32/33 (2026-08-14, owner-verified P0): assistant workspaces — each
   * workspace's rootPath is queried with sessionAPI.listSessions so invite/fork
   * show the REAL persisted sessions living there (opened or not), exactly
   * matching the create-group member source. R-GC-33 removes R-GC-19's
   * fabricated preset rows (fake SessionMetadata with hardcoded 'Claw').
   * Same shape as CreateGroupChatDialog's assistantWorkspaces (MainNav
   * assistantWorkspacesList).
   */
  assistantWorkspaces?: WorkspaceInfo[];
}

/**
 * Resolve the last persisted turn id of a group session. fork_group_chat
 * requires a source_turn_id that matches a persisted turn (branch_session
 * errors with NotFound otherwise), and the backend send flow uses the same id
 * as messageId and turn_id. Falls back to the newest locally injected turn.
 */
function lastTurnIdOf(session: { dialogTurns?: DialogTurn[] } | undefined): string | undefined {
  const turns = session?.dialogTurns;
  if (!turns || turns.length === 0) return undefined;
  const last = turns[turns.length - 1];
  return last?.id || last?.userMessage?.id || undefined;
}

export const GroupChatView: React.FC<GroupChatViewProps> = ({
  groupId,
  workspacePath,
  groupName,
  isSceneActive = true,
  assistantWorkspaces = [],
}) => {
  const { t } = useI18n('common');
  const [isLoadingHistory, setIsLoadingHistory] = useState(false);
  const [historyFailed, setHistoryFailed] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const [, forceRender] = useState(0);

  // R-GC-15: member management state (dialogs only; no custom bars, R-GC-24).
  const [isMembersOpen, setIsMembersOpen] = useState(false);
  const [memberIds, setMemberIds] = useState<string[]>([]);
  const [memberMetaById, setMemberMetaById] = useState<Map<string, SessionMetadata>>(new Map());
  const [isLoadingMembers, setIsLoadingMembers] = useState(false);
  const [membersLoadFailed, setMembersLoadFailed] = useState(false);
  const [isInviteOpen, setIsInviteOpen] = useState(false);
  const [isForkOpen, setIsForkOpen] = useState(false);
  const [isMutatingMember, setIsMutatingMember] = useState(false);
  const membersInitRef = React.useRef(false);

  const lastTurnId = useMemo(
    () => lastTurnIdOf(flowChatStore.getState().sessions.get(groupId)),
    // re-read on render; flowChatStore updates are surfaced via forceRender.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [groupId, isSending],
  );

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

  // R-GC-37 (2026-08-15): member name resolution walks EVERY workspace root —
  // a group may contain members persisted in other assistant workspaces, and a
  // single listSessions(workspacePath) lookup would resolve those ids to
  // nothing (members would render as raw UUIDs). Same shape as
  // CreateGroupChatDialog.loadMembers (roots = workspacePath + assistant
  // workspace rootPaths, sessionId deduped, per-root catch -> []).
  // Reuses sessionAPI.loadSessionMetadata + sessionAPI.listSessions (the same
  // data source the R-GC-13 member picker uses); no new storage is built.
  // Same stable-reference pattern as CreateGroupChatDialog (R-GC-19): the
  // workspaces array reference may change every render; a ref holds the latest
  // value so loadMembers keeps a stable identity.
  const assistantWorkspacesRef = React.useRef(assistantWorkspaces);
  assistantWorkspacesRef.current = assistantWorkspaces;

  const loadMembers = useCallback(async () => {
    if (!groupId || !workspacePath) return;
    setIsLoadingMembers(true);
    setMembersLoadFailed(false);
    try {
      const metadata = await sessionAPI.loadSessionMetadata(groupId, workspacePath);
      const raw = metadata?.customMetadata?.groupChats;
      const ids: string[] = Array.isArray(raw)
        ? raw.filter((v): v is string => typeof v === 'string')
        : [];
      setMemberIds(ids);

      // Resolve display names across ALL workspace roots (R-GC-37), same as
      // CreateGroupChatDialog:90-111: workspacePath + every assistant
      // workspace rootPath, dedupe by sessionId (first root wins), per-root
      // listSessions failures degrade to []. Missing sessions fall back to
      // their raw session id in memberRows (defensive, never crashes).
      const roots = [
        workspacePath,
        ...assistantWorkspacesRef.current.map(workspace => workspace.rootPath).filter(Boolean),
      ].filter((root, index, array) => root && array.indexOf(root) === index);
      const seen = new Set<string>();
      const byId = new Map<string, SessionMetadata>();
      const lists = await Promise.all(
        roots.map(root =>
          Promise.resolve(sessionAPI.listSessions(root)).catch((error) => {
            log.warn('Failed to load sessions for group member resolution', { error, workspacePath: root });
            return [];
          }),
        ),
      );
      for (const list of lists) {
        // Defensive: a root returning undefined (or a malformed payload) must
        // not crash member resolution — same Array.isArray guard as the old
        // single-root lookup.
        if (!Array.isArray(list)) continue;
        for (const meta of list) {
          if (seen.has(meta.sessionId)) continue;
          seen.add(meta.sessionId);
          byId.set(meta.sessionId, meta);
        }
      }
      setMemberMetaById(new Map(ids.map(id => [id, byId.get(id)]).filter(
        (entry): entry is [string, SessionMetadata] => entry[1] !== undefined,
      )));
    } catch (error) {
      log.warn('Failed to load group members', { groupId, error });
      setMembersLoadFailed(true);
    } finally {
      setIsLoadingMembers(false);
    }
  }, [groupId, workspacePath]);

  useEffect(() => {
    if (membersInitRef.current) return;
    membersInitRef.current = true;
    void loadMembers();
  }, [loadMembers]);

  // R-GC-15: invite — invite_group_member (contract section 1.4, camelCase
  // execute_tool wrapper). workspace passed to the backend = current
  // workspacePath (contract section 2a / group_room_tools.rs invite path).
  const handleInvite = useCallback(async (selectedIds: string[]) => {
    if (selectedIds.length === 0 || isMutatingMember) return;
    setIsMutatingMember(true);
    try {
      let successCount = 0;
      for (const memberSessionId of selectedIds) {
        const response = await toolAPI.executeTool({
          toolName: 'invite_group_member',
          parameters: {
            action: 'invite',
            group_id: groupId,
            member_session_id: memberSessionId,
            workspace: workspacePath,
          },
          workspacePath,
        });
        if (response?.success !== true) {
          const message =
            response?.error ||
            response?.validation_error ||
            t('nav.groupChats.inviteFailed');
          notificationService.error(message, { duration: 4000 });
          continue;
        }
        successCount += 1;
      }
      if (successCount > 0) {
        notificationService.success(
          t('nav.groupChats.invited', { count: successCount }),
          { duration: 3000 },
        );
        await loadMembers();
      }
    } catch (error) {
      log.error('Failed to invite group members', { groupId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.inviteFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsMutatingMember(false);
    }
  }, [groupId, isMutatingMember, loadMembers, t, workspacePath]);

  // R-GC-15: remove — remove_group_member.
  const handleRemove = useCallback(async (memberSessionId: string) => {
    if (isMutatingMember) return;
    setIsMutatingMember(true);
    try {
      const response = await toolAPI.executeTool({
        toolName: 'remove_group_member',
        parameters: { action: 'remove', group_id: groupId, member_session_id: memberSessionId },
        workspacePath,
      });
      if (response?.success !== true) {
        const message =
          response?.error ||
          response?.validation_error ||
          t('nav.groupChats.removeFailed');
        notificationService.error(message, { duration: 4000 });
        return;
      }
      notificationService.success(t('nav.groupChats.removed'), { duration: 3000 });
      await loadMembers();
    } catch (error) {
      log.error('Failed to remove group member', { groupId, memberSessionId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.removeFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsMutatingMember(false);
    }
  }, [groupId, isMutatingMember, loadMembers, t, workspacePath]);

  // R-GC-15: fork — fork_group_chat then jump to the child group view.
  // Reuses the R-GC-13 handleGroupChatCreated registration shape
  // (createSession + markSessionAsGroupChat + openMainSession).
  const handleFork = useCallback(async (name: string, memberIds: string[]) => {
    if (isMutatingMember) return;
    const turnId = lastTurnId;
    if (!turnId) {
      notificationService.error(t('nav.groupChats.forkNeedsMessage'), { duration: 4000 });
      return;
    }
    setIsMutatingMember(true);
    try {
      const response = await toolAPI.executeTool({
        toolName: 'fork_group_chat',
        parameters: {
          action: 'fork',
          group_id: groupId,
          name,
          turn_id: turnId,
          members: memberIds,
        },
        workspacePath,
      });
      const childGroupId = response?.result?.childGroupId;
      if (response?.success !== true || typeof childGroupId !== 'string' || !childGroupId) {
        const message =
          response?.error ||
          response?.validation_error ||
          t('nav.groupChats.forkFailed');
        notificationService.error(message, { duration: 4000 });
        return;
      }
      notificationService.success(t('nav.groupChats.forked', { name }), { duration: 3000 });
      // Jump to the child group view (R-GC-15 acceptance: fork -> child view).
      // Child group = agent_type="group" session, same as the parent
      // (branch_session forks the group session; backend agent type is
      // default_group_agent_type, group_room_tools.rs — R-WF-02 first-class
      // agent type).
      flowChatStore.createSession(
        childGroupId,
        {
          workspacePath,
          projectWorkspacePath: workspacePath,
          agentType: 'group',
        },
        undefined,
        name,
        1048576,
        'group',
        workspacePath,
      );
      flowChatStore.markSessionAsGroupChat(childGroupId);
      await openMainSession(childGroupId, {});
    } catch (error) {
      log.error('Failed to fork group chat', { groupId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.forkFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsMutatingMember(false);
    }
  }, [groupId, isMutatingMember, lastTurnId, t, workspacePath]);

  const handleSubmit = useCallback(async (submission: ChatInputSubmission) => {
    const content = submission.text?.trim();
    if (!content || isSending || !groupId) return;
    setIsSending(true);
    try {
      // Contract section 1.4: go through execute_tool (camelCase). Bare
      // invoke('send_group_message') is forbidden (R-GC-05 removed the command).
      // R-GC-34 (owner identity P0 fix, plan B): the group chat owner is the
      // master actor, not the group session itself. sender_session_id uses the
      // GROUP_MASTER_ACTOR reserved word ("__master__", local_customizations.rs:
      // 96). The backend resolves it to Commander role + L0 depth + localized
      // owner name, so the bubble renders "[Commander L0] Owner" instead of
      // "[Agent L0] <group name>" (sender badge reads metadata.senderRole/
      // senderName, UserMessageItem.tsx:219).
      const response = await toolAPI.executeTool({
        toolName: 'send_group_message',
        parameters: {
          action: 'send',
          group_id: groupId,
          content,
          sender_session_id: '__master__',
        },
        workspacePath,
      });
      if (response?.success !== true) {
        const message = response?.error || response?.validation_error || t('nav.groupChats.sendFailed');
        notificationService.error(message, { duration: 4000 });
        return;
      }
      // R-GC-26: the backend routes the message into the group session's real
      // dialog turn (coordinator.start_dialog_turn), which emits
      // DialogTurnStarted + streaming events. The event handler creates the
      // turn and renders the group master response; no local optimistic
      // injection is needed (a local turn would duplicate the backend turn).
    } catch (error) {
      log.error('Failed to send group message', { groupId, error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.sendFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsSending(false);
    }
  }, [groupId, isSending, t, workspacePath]);

  const registration = useMemo<ChatInputRegistration>(
    () => ({
      registrationId: `group-chat:${groupId}`,
      placeholder: t('nav.groupChats.messagePlaceholder'),
      workspacePath,
      onSubmit: handleSubmit,
    }),
    [groupId, handleSubmit, t, workspacePath],
  );

  // R-GC-15: member rows — name from listSessions metadata, fallback raw id.
  const memberRows = useMemo(
    () => memberIds.map(id => ({ id, name: memberMetaById.get(id)?.sessionName || id })),
    [memberIds, memberMetaById],
  );

  // R-GC-24: group chat menu rendered inside the original FlowChatHeader left
  // action group (reuses IconButton + Modal + Select; no custom top bar).
  const headerLeftActionsContent = useMemo(() => (
    <div
      className="group-chat-view__header-actions"
      data-bf-component="group-chat-view"
      data-bf-part="headerActions"
    >
      <IconButton
        variant="ghost"
        size="xs"
        aria-label={t('nav.groupChats.membersLabel', { count: memberRows.length })}
        tooltip={t('nav.groupChats.membersLabel', { count: memberRows.length })}
        data-testid="group-chat-members-toggle"
        onClick={() => setIsMembersOpen(true)}
      >
        <Users size={14} aria-hidden="true" />
      </IconButton>
      <IconButton
        variant="ghost"
        size="xs"
        aria-label={t('nav.groupChats.invite')}
        tooltip={t('nav.groupChats.invite')}
        onClick={() => setIsInviteOpen(true)}
      >
        <UserPlus size={14} aria-hidden="true" />
      </IconButton>
      <IconButton
        variant="ghost"
        size="xs"
        aria-label={t('nav.groupChats.fork')}
        tooltip={t('nav.groupChats.fork')}
        onClick={() => setIsForkOpen(true)}
      >
        <GitBranch size={14} aria-hidden="true" />
      </IconButton>
    </div>
  ), [memberRows.length, t]);

  const emptyState = useMemo(
    () => (
      <div className="group-chat-view__empty" data-bf-component="group-chat-view" data-bf-part="emptyState">
        {t('nav.groupChats.viewHint')}
      </div>
    ),
    [t],
  );

  return (
    <div
      className="group-chat-view"
      data-bf-component="group-chat-view"
      data-bf-part="root"
      data-testid="group-chat-view"
      data-group-id={groupId}
    >
      <div className="group-chat-view__body" data-bf-component="group-chat-view" data-bf-part="body">
        {isLoadingHistory && !flowChatStore.getState().sessions.get(groupId)?.dialogTurns.length ? (
          <div className="group-chat-view__state">{t('nav.sessions.loading')}</div>
        ) : historyFailed && !flowChatStore.getState().sessions.get(groupId)?.dialogTurns.length ? (
          <div className="group-chat-view__state">
            {t('nav.groupChats.historyLoadFailed')}
            <button
              type="button"
              className="group-chat-view__retry"
              onClick={() => { void loadHistory(); }}
            >
              {t('actions.retry')}
            </button>
          </div>
        ) : (
          <FlowChatContainer
            className="group-chat-view__chat-container"
            isViewportActive={isSceneActive}
            emptyState={emptyState}
            headerLeftActionsContent={headerLeftActionsContent}
            onOpenVisualization={() => {}}
            onFileViewRequest={() => {}}
            onTabOpen={() => {}}
            onSwitchToChatPanel={() => {}}
            config={{ enableMarkdown: true, autoScroll: true, showTimestamps: false }}
          />
        )}
      </div>

      <div className="group-chat-view__input" data-bf-component="group-chat-view" data-bf-part="input">
        <ChatInput
          isSceneActive={isSceneActive}
          onSendMessage={(_message: string) => {}}
          registration={registration}
        />
      </div>

      {isMembersOpen ? (
        <GroupMembersDialog
          groupName={groupName}
          memberRows={memberRows}
          isLoading={isLoadingMembers}
          loadFailed={membersLoadFailed}
          busy={isMutatingMember}
          onRetry={() => { void loadMembers(); }}
          onClose={() => setIsMembersOpen(false)}
          onRemove={handleRemove}
        />
      ) : null}

      {isInviteOpen ? (
        <GroupMemberPickerDialog
          title={t('nav.groupChats.inviteTitle')}
          workspacePath={workspacePath}
          assistantWorkspaces={assistantWorkspaces}
          isOpen={isInviteOpen}
          busy={isMutatingMember}
          onClose={() => setIsInviteOpen(false)}
          onConfirm={handleInvite}
        />
      ) : null}

      {isForkOpen ? (
        <GroupForkDialog
          groupName={groupName}
          workspacePath={workspacePath}
          assistantWorkspaces={assistantWorkspaces}
          isOpen={isForkOpen}
          busy={isMutatingMember}
          onClose={() => setIsForkOpen(false)}
          onConfirm={handleFork}
        />
      ) : null}
    </div>
  );
};

/**
 * R-GC-24: member list dialog. Reuses Modal + Button (component-library);
 * rows render the existing member list shape. No custom top bar.
 */
interface GroupMembersDialogProps {
  groupName?: string;
  memberRows: Array<{ id: string; name: string }>;
  isLoading: boolean;
  loadFailed: boolean;
  busy: boolean;
  onRetry: () => void;
  onClose: () => void;
  onRemove: (memberSessionId: string) => void | Promise<void>;
}

function GroupMembersDialog({
  groupName,
  memberRows,
  isLoading,
  loadFailed,
  busy,
  onRetry,
  onClose,
  onRemove,
}: GroupMembersDialogProps) {
  const { t } = useI18n('common');
  return (
    <Modal
      isOpen
      onClose={busy ? () => {} : onClose}
      title={groupName || t('nav.groupChats.untitled')}
      size="small"
      closeOnOverlayClick={!busy}
    >
      <div data-bf-component="group-member-list-dialog" data-bf-part="root" className="group-chat-dialog">
        {isLoading ? (
          <div className="group-chat-dialog__state">{t('nav.sessions.loading')}</div>
        ) : loadFailed ? (
          <div className="group-chat-dialog__state">
            {t('nav.groupChats.membersLoadFailed')}
            <Button type="button" variant="secondary" size="small" onClick={onRetry}>
              {t('actions.retry')}
            </Button>
          </div>
        ) : memberRows.length === 0 ? (
          <div className="group-chat-dialog__state">{t('nav.groupChats.noMembers')}</div>
        ) : (
          <div className="group-chat-dialog__member-list" data-testid="group-chat-member-list">
            {memberRows.map(member => (
              <div
                key={member.id}
                className="group-chat-dialog__member-row"
                data-bf-component="group-member-list-dialog"
                data-bf-part="memberRow"
                data-member-id={member.id}
              >
                <span className="group-chat-dialog__member-name">{member.name}</span>
                <Button
                  type="button"
                  variant="ghost"
                  size="small"
                  disabled={busy}
                  onClick={() => { void onRemove(member.id); }}
                >
                  {t('nav.groupChats.remove')}
                </Button>
              </div>
            ))}
          </div>
        )}

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t('actions.close')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/**
 * R-GC-22/30/32/33: member picker dialog (invite). R-GC-30 (owner directive): the
 * owner picks invite members themselves from a real optional session list — NO
 * member-count input (R-GC-28 had wrongly added it), NO hardcoded presets.
 * R-GC-33 (2026-08-14, owner-verified P0, CEO ruling): member source = REAL
 * sessions across ALL assistant workspaces — for each assistant workspace
 * rootPath, sessionAPI.listSessions(thatRoot) returns the persisted sessions
 * that actually live on disk (opened or not, because the backend
 * `assistant_workspace_base_dir` + `ensure_assistant_workspaces` discover and
 * open every `~/.bitfun/personal_assistant/workspace-*`). NO fabricated
 * SessionMetadata presets (R-GC-19's `inactive` fake rows are removed — they
 * invented sessionId=workspace.id / hardcoded agentType='Claw' / fake values).
 * R-GC-R6 (2026-08-15, owner decision): agentType NOT filtered — every real
 * session including agentic is a selectable member. agentType comes from real
 * session metadata, zero hardcoded strings.
 * Reuses the component-library Select (Select.tsx:87) with multiple +
 * searchable + showSelectAll inside the existing Modal.
 */
interface GroupMemberPickerDialogProps {
  title: string;
  workspacePath: string;
  assistantWorkspaces?: WorkspaceInfo[];
  isOpen: boolean;
  busy: boolean;
  onClose: () => void;
  onConfirm: (selectedIds: string[]) => void | Promise<void>;
}

function GroupMemberPickerDialog({
  title,
  workspacePath,
  assistantWorkspaces = [],
  isOpen,
  busy,
  onClose,
  onConfirm,
}: GroupMemberPickerDialogProps) {
  const { t } = useI18n('common');
  const [sessions, setSessions] = useState<SessionMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // R-GC-19 same stable-reference pattern: the workspaces array reference may
  // change every render; putting it directly in deps would rebuild loadSessions
  // repeatedly -> useEffect infinite reload.
  const assistantWorkspacesRef = React.useRef(assistantWorkspaces);
  assistantWorkspacesRef.current = assistantWorkspaces;

  // R-GC-33 / R-GC-R6 (owner decision 2026-08-15): member source = ALL real
  // sessions across every assistant workspace root (including the group
  // workspace itself), agentType NOT filtered — every real session (Claw or
  // agentic) is a selectable member. listSessions per root reads real
  // persisted metadata from disk; no fake preset rows.
  const loadSessions = useCallback(() => {
    setIsLoading(true);
    setLoadFailed(false);
    const roots = [
      workspacePath,
      ...assistantWorkspacesRef.current.map(workspace => workspace.rootPath).filter(Boolean),
    ].filter((root, index, array) => root && array.indexOf(root) === index);
    const seen = new Set<string>();
    return Promise.all(
      roots.map(root =>
        sessionAPI.listSessions(root).catch((error) => {
          log.warn('Failed to load sessions for member picker', { error, workspacePath: root });
          return [];
        }),
      ),
    )
      .then(lists => {
        const byId = new Map<string, SessionMetadata>();
        for (const list of lists) {
          for (const meta of list) {
            if (seen.has(meta.sessionId)) continue;
            seen.add(meta.sessionId);
            byId.set(meta.sessionId, meta);
          }
        }
        setSessions(Array.from(byId.values()));
      })
      .catch(error => {
        log.warn('Failed to load sessions for member picker', { error, workspacePath });
        setLoadFailed(true);
      })
      .finally(() => setIsLoading(false));
  }, [workspacePath]);

  useEffect(() => {
    if (!isOpen) {
      setSelectedIds(new Set());
      setLoadFailed(false);
      return;
    }
    void loadSessions();
  }, [isOpen, loadSessions]);

  const options = useMemo<SelectOption[]>(
    () => sessions.map(meta => ({
      value: meta.sessionId,
      label: meta.sessionName || t('nav.sessions.untitled'),
    })),
    [sessions, t],
  );

  const selectedValue = useMemo(
    () => options.filter(option => selectedIds.has(String(option.value))).map(option => option.value),
    [options, selectedIds],
  );

  return (
    <Modal
      isOpen={isOpen}
      onClose={busy ? () => {} : onClose}
      title={title}
      size="medium"
      closeOnOverlayClick={!busy}
    >
      <div data-bf-component="group-member-picker-dialog" data-bf-part="root" className="group-chat-dialog">
        <div className="group-chat-dialog__field">
          {isLoading ? (
            <div className="group-chat-dialog__state">{t('nav.sessions.loading')}</div>
          ) : loadFailed ? (
            <div className="group-chat-dialog__state">
              {t('nav.groupChats.membersLoadFailed')}
              <Button type="button" variant="secondary" size="small" onClick={() => { void loadSessions(); }}>
                {t('actions.retry')}
              </Button>
            </div>
          ) : (
            <Select
              multiple
              searchable
              showSelectAll
              loading={isLoading}
              options={options}
              value={selectedValue}
              placeholder={t('nav.groupChats.members')}
              emptyText={t('nav.groupChats.noClawSessions')}
              searchPlaceholder={t('nav.groupChats.membersSearch')}
              onChange={(value) => {
                const next = Array.isArray(value) ? value : [value];
                setSelectedIds(new Set(next.map(String)));
              }}
              data-testid="group-member-picker-select"
              triggerTestId="group-member-picker-trigger"
              dropdownTestId="group-member-picker-dropdown"
            />
          )}
        </div>

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => {
              void onConfirm(Array.from(selectedIds));
            }}
            disabled={busy || selectedIds.size === 0}
            isLoading={busy}
          >
            {t('nav.groupChats.confirmInvite')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/**
 * R-GC-15/30/32/33: fork dialog — child group name + member multi-select.
 * R-GC-30 (owner directive): the owner picks fork members themselves from a
 * real optional session list — NO member-count input (R-GC-28 had wrongly added
 * it). R-GC-33 (2026-08-14, owner-verified P0, CEO ruling): member source = REAL
 * sessions across ALL assistant workspace roots (same loader as invite/create),
 * no fabricated presets. R-GC-R6 (2026-08-15, owner decision): agentType NOT
 * filtered — every real session including agentic is a selectable member.
 * Reuses the component-library Select (Select.tsx:87) with multiple +
 * searchable + showSelectAll inside the existing Modal.
 */
interface GroupForkDialogProps {
  groupName?: string;
  workspacePath: string;
  assistantWorkspaces?: WorkspaceInfo[];
  isOpen: boolean;
  busy: boolean;
  onClose: () => void;
  onConfirm: (name: string, memberIds: string[]) => void | Promise<void>;
}

function GroupForkDialog({
  groupName,
  workspacePath,
  assistantWorkspaces = [],
  isOpen,
  busy,
  onClose,
  onConfirm,
}: GroupForkDialogProps) {
  const { t } = useI18n('common');
  const [name, setName] = useState('');
  const [sessions, setSessions] = useState<SessionMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  // R-GC-19 same stable-reference pattern (same as GroupMemberPickerDialog).
  const assistantWorkspacesRef = React.useRef(assistantWorkspaces);
  assistantWorkspacesRef.current = assistantWorkspaces;

  // R-GC-33 / R-GC-R6: same source as create/invite — walk every assistant
  // workspace root to load REAL sessions (agentType not filtered), zero
  // fabricated presets.
  const loadSessions = useCallback(() => {
    setIsLoading(true);
    setLoadFailed(false);
    const roots = [
      workspacePath,
      ...assistantWorkspacesRef.current.map(workspace => workspace.rootPath).filter(Boolean),
    ].filter((root, index, array) => root && array.indexOf(root) === index);
    const seen = new Set<string>();
    return Promise.all(
      roots.map(root =>
        sessionAPI.listSessions(root).catch((error) => {
          log.warn('Failed to load sessions for fork member picker', { error, workspacePath: root });
          return [];
        }),
      ),
    )
      .then(lists => {
        const byId = new Map<string, SessionMetadata>();
        for (const list of lists) {
          for (const meta of list) {
            if (seen.has(meta.sessionId)) continue;
            seen.add(meta.sessionId);
            byId.set(meta.sessionId, meta);
          }
        }
        setSessions(Array.from(byId.values()));
      })
      .catch(error => {
        log.warn('Failed to load sessions for fork member picker', { error, workspacePath });
        setLoadFailed(true);
      })
      .finally(() => setIsLoading(false));
  }, [workspacePath]);

  // Only seed the default child-group name when the dialog opens; never reset
  // the user's typed name on unrelated re-renders (t/groupName excluded from
  // deps for that reason).
  const forkOpenedRef = React.useRef(false);
  useEffect(() => {
    if (!isOpen) {
      forkOpenedRef.current = false;
      setName('');
      setSelectedIds(new Set());
      setLoadFailed(false);
      return;
    }
    if (!forkOpenedRef.current) {
      forkOpenedRef.current = true;
      setName(`${groupName || t('nav.groupChats.untitled')} ${t('nav.groupChats.forkSuffix')}`);
      void loadSessions();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen]);

  const options = useMemo<SelectOption[]>(
    () => sessions.map(meta => ({
      value: meta.sessionId,
      label: meta.sessionName || t('nav.sessions.untitled'),
    })),
    [sessions, t],
  );

  const selectedValue = useMemo(
    () => options.filter(option => selectedIds.has(String(option.value))).map(option => option.value),
    [options, selectedIds],
  );

  const trimmedName = name.trim();

  return (
    <Modal
      isOpen={isOpen}
      onClose={busy ? () => {} : onClose}
      title={t('nav.groupChats.forkTitle')}
      size="medium"
      closeOnOverlayClick={!busy}
    >
      <div data-bf-component="group-fork-dialog" data-bf-part="root" className="group-chat-dialog">
        <div className="group-chat-dialog__field">
          <Input
            label={t('nav.groupChats.groupName')}
            value={name}
            onChange={e => setName(e.target.value)}
            placeholder={t('nav.groupChats.groupNamePlaceholder')}
            inputSize="medium"
            autoFocus
          />
        </div>

        <div className="group-chat-dialog__field">
          {isLoading ? (
            <div className="group-chat-dialog__state">{t('nav.sessions.loading')}</div>
          ) : loadFailed ? (
            <div className="group-chat-dialog__state">
              {t('nav.groupChats.membersLoadFailed')}
              <Button type="button" variant="secondary" size="small" onClick={() => { void loadSessions(); }}>
                {t('actions.retry')}
              </Button>
            </div>
          ) : (
            <Select
              multiple
              searchable
              showSelectAll
              loading={isLoading}
              options={options}
              value={selectedValue}
              placeholder={t('nav.groupChats.members')}
              emptyText={t('nav.groupChats.noClawSessions')}
              searchPlaceholder={t('nav.groupChats.membersSearch')}
              onChange={(value) => {
                const next = Array.isArray(value) ? value : [value];
                setSelectedIds(new Set(next.map(String)));
              }}
              data-testid="group-fork-picker-select"
              triggerTestId="group-fork-picker-trigger"
              dropdownTestId="group-fork-picker-dropdown"
            />
          )}
        </div>

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={busy}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => {
              void onConfirm(trimmedName, Array.from(selectedIds));
            }}
            disabled={busy || !trimmedName}
            isLoading={busy}
          >
            {t('nav.groupChats.confirmFork')}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

export default GroupChatView;
