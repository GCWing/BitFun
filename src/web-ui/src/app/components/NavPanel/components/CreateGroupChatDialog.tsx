/**
 * CreateGroupChatDialog - group chat create dialog (R-GC-13 / R-GC-28 / R-GC-30
 * / R-GC-33).
 *
 * Reuse rules:
 * - Modal / Button / Input / Checkbox all from component-library (existing components).
 * - R-GC-30 (owner directive, direction corrected 2026-08-14): the owner picks
 *   group members themselves from a real optional session list — NO member-count
 *   input (R-GC-28 had wrongly added it), NO hardcoded presets. Member source =
 *   runtime-fetched real sessions across ALL assistant workspace roots
 *   (sessionAPI.listSessions per root; R-GC-R6 2026-08-15: agentType no longer
 *   filtered — every real session including agentic is selectable).
 * - R-GC-33 (2026-08-14, owner-verified P0, CEO ruling): R-GC-19's preset fabrication
 *   is REMOVED — previously assistantWorkspaces were faked into SessionMetadata
 *   rows (sessionId = workspace.id, hardcoded agentType 'Claw', fake values).
 *   Now each assistant workspace rootPath is queried with listSessions and the
 *   real persisted sessions (opened or not) are shown; agentType comes from
 *   real session metadata, zero hardcoded strings.
 * - Create goes through toolAPI.executeTool (camelCase - the only existing
 *   execute_tool wrapper, ToolAPI.ts:49-61); direct invoke('create_group_chat')
 *   is forbidden (the backend command was removed in R-GC-05).
 * - Members = the real session ids the caller passes in. The backend
 *   create validates each id exists and registers it in groupChats
 *   (group_room_tools.rs create_group, R-GC-28 rebuilt contract: no fresh
 *   anonymous member sessions are created anymore). The selected ids here
 *   are the members, used 1:1.
 */

import React, { useCallback, useEffect, useState } from 'react';
import { Button, Checkbox, Input, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { toolAPI } from '@/infrastructure/api/service-api/ToolAPI';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionMetadata } from '@/shared/types/session-history';
import type { WorkspaceInfo } from '@/shared/types';
import { createLogger } from '@/shared/utils/logger';
import { notificationService } from '@/shared/notification-system';
import './CreateGroupChatDialog.scss';

const log = createLogger('CreateGroupChatDialog');

interface CreateGroupChatDialogProps {
  isOpen: boolean;
  onClose: () => void;
  /** Group workspace rootPath (R-GC-26: Claw default assistant workspace). */
  workspacePath: string;
  /**
   * R-GC-19/30/33: assistant workspaces (Claw presets) — each workspace's
   * rootPath is queried with sessionAPI.listSessions to collect the REAL
   * persisted Claw sessions living there (opened or not). R-GC-33 removes the
   * R-GC-19 fake-preset fabrication: no SessionMetadata rows are invented.
   */
  assistantWorkspaces?: WorkspaceInfo[];
  onCreated: (groupId: string, name: string) => void | Promise<void>;
}

export const CreateGroupChatDialog: React.FC<CreateGroupChatDialogProps> = ({
  isOpen,
  onClose,
  workspacePath,
  assistantWorkspaces = [],
  onCreated,
}) => {
  const { t } = useI18n('common');
  const [name, setName] = useState('');
  const [members, setMembers] = useState<SessionMetadata[]>([]);
  const [selectedMemberIds, setSelectedMemberIds] = useState<Set<string>>(new Set());
  const [isLoadingMembers, setIsLoadingMembers] = useState(false);
  const [loadFailed, setLoadFailed] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // R-GC-19: keep a stable reference to assistantWorkspaces (the array reference
  // passed by the parent may change every render; using it directly in useCallback
  // deps would rebuild loadMembers repeatedly -> useEffect would loop loadMembers
  // forever). A ref holds the latest value; deps keep only workspacePath and
  // isOpen to drive the one-shot load.
  const assistantWorkspacesRef = React.useRef(assistantWorkspaces);
  assistantWorkspacesRef.current = assistantWorkspaces;

  // R-GC-33 / R-GC-R6 (owner decision 2026-08-15): member source = ALL real
  // sessions across every assistant workspace root (including the current
  // workspace), agentType NOT filtered — every real session (Claw or agentic)
  // is selectable as a group member. listSessions per root reads real
  // persisted metadata from disk; R-GC-19's fabricated preset rows (inactive
  // fake SessionMetadata) are removed. agentType is read from real session
  // metadata — zero hardcoded strings.
  const loadMembers = useCallback(async () => {
    setIsLoadingMembers(true);
    setLoadFailed(false);
    try {
      const roots = [
        workspacePath,
        ...assistantWorkspacesRef.current.map(workspace => workspace.rootPath).filter(Boolean),
      ].filter((root, index, array) => root && array.indexOf(root) === index);
      const seen = new Set<string>();
      const byId = new Map<string, SessionMetadata>();
      const lists = await Promise.all(
        roots.map(root =>
          sessionAPI.listSessions(root).catch((error) => {
            log.warn('Failed to load sessions for group member picker', { error, workspacePath: root });
            return [];
          }),
        ),
      );
      for (const list of lists) {
        for (const meta of list) {
          if (seen.has(meta.sessionId)) continue;
          seen.add(meta.sessionId);
          byId.set(meta.sessionId, meta);
        }
      }
      setMembers(Array.from(byId.values()));
    } catch (error) {
      log.warn('Failed to load sessions for group member picker', { error, workspacePath });
      setLoadFailed(true);
    } finally {
      setIsLoadingMembers(false);
    }
  }, [workspacePath]);

  useEffect(() => {
    if (!isOpen) {
      setName('');
      setSelectedMemberIds(new Set());
      setLoadFailed(false);
      return;
    }
    void loadMembers();
  }, [isOpen, loadMembers]);

  const toggleMember = useCallback((sessionId: string) => {
    setSelectedMemberIds(prev => {
      const next = new Set(prev);
      if (next.has(sessionId)) {
        next.delete(sessionId);
      } else {
        next.add(sessionId);
      }
      return next;
    });
  }, []);

  const allMemberIds = members.map(meta => meta.sessionId);
  const allSelected = allMemberIds.length > 0 && selectedMemberIds.size === allMemberIds.length;

  const toggleSelectAll = useCallback(() => {
    setSelectedMemberIds(prev => (
      prev.size === allMemberIds.length ? new Set() : new Set(allMemberIds)
    ));
  }, [allMemberIds]);

  const handleCreate = useCallback(async () => {
    const trimmedName = name.trim();
    if (!trimmedName || isSubmitting) return;
    setIsSubmitting(true);
    try {
      // R-GC-30 / R-GC-R6: members = the owner's own picks from the real
      // session list (every real session including agentic, not filtered).
      // The backend create validates each picked id exists and registers it
      // in the group's groupChats (group_room_tools.rs create_group); it does
      // not create fresh member sessions.
      const memberIds = Array.from(selectedMemberIds);
      // Contract section 1.4: go through execute_tool (ToolAPI camelCase
      // wrapper); direct invoke('create_group_chat') is forbidden.
      const response = await toolAPI.executeTool({
        toolName: 'create_group_chat',
        parameters: { action: 'create', name: trimmedName, members: memberIds, workspace: workspacePath || undefined },
        workspacePath,
      });
      const groupId = response?.result?.groupId;
      if (response?.success !== true || typeof groupId !== 'string' || !groupId) {
        const message =
          response?.error ||
          response?.validation_error ||
          t('nav.groupChats.createFailed');
        notificationService.error(message, { duration: 4000 });
        return;
      }
      // R-GC-31 (2026-08-14, owner-verified P0): the frontend create toast was
      // REMOVED — the single creation notice is the backend welcome turn bubble
      // (group_room_tools.rs:382 "group chat created" message; R-GC-25
      // group-owner session structure dependency, the group session must open with
      // a real host turn). Before, the frontend toast and the welcome turn showed
      // identical text = real duplication (R-GC-29 only slimmed the backend text
      // without owner testing; acceptance assertions must be verified at runtime,
      // not self-declared after editing).
      await onCreated(groupId, trimmedName);
      onClose();
    } catch (error) {
      log.error('Failed to create group chat', { error });
      notificationService.error(
        error instanceof Error ? error.message : t('nav.groupChats.createFailed'),
        { duration: 4000 },
      );
    } finally {
      setIsSubmitting(false);
    }
  }, [isSubmitting, name, onClose, onCreated, selectedMemberIds, t, workspacePath]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={isSubmitting ? () => {} : onClose}
      title={t('nav.groupChats.newGroupChat')}
      size="medium"
      closeOnOverlayClick={!isSubmitting}
    >
      <div data-bf-component="create-group-chat-dialog" data-bf-part="root" className="group-chat-dialog">
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

        {/* R-GC-30 / R-GC-R6: real-session member multi-select (owner picks;
            runtime-fetched list, zero hardcoded). R-GC-28's member-count input
            is removed. */}
        <div className="group-chat-dialog__members">
          <div className="group-chat-dialog__members-header">
            <span className="group-chat-dialog__members-label">{t('nav.groupChats.members')}</span>
            {members.length > 0 ? (
              <Checkbox
                checked={allSelected}
                onChange={toggleSelectAll}
                label={allSelected ? t('actions.deselectAll') : t('actions.selectAll')}
                size="small"
              />
            ) : null}
          </div>

          {isLoadingMembers ? (
            <div className="group-chat-dialog__state">{t('nav.sessions.loading')}</div>
          ) : loadFailed ? (
            <div className="group-chat-dialog__state">
              {t('nav.groupChats.membersLoadFailed')}
              <Button type="button" variant="secondary" size="small" onClick={() => { void loadMembers(); }}>
                {t('actions.retry')}
              </Button>
            </div>
          ) : members.length === 0 ? (
            <div className="group-chat-dialog__state">{t('nav.groupChats.noClawSessions')}</div>
          ) : (
            <div className="group-chat-dialog__member-list">
              {members.map(meta => {
                const isSelected = selectedMemberIds.has(meta.sessionId);
                return (
                  <label
                    key={meta.sessionId}
                    className={`group-chat-dialog__member-row${isSelected ? ' is-selected' : ''}`}
                  >
                    <Checkbox
                      checked={isSelected}
                      onChange={() => toggleMember(meta.sessionId)}
                      disabled={isSubmitting}
                    />
                    <span className="group-chat-dialog__member-name">
                      {meta.sessionName || t('nav.sessions.untitled')}
                    </span>
                  </label>
                );
              })}
            </div>
          )}
        </div>

        <div className="group-chat-dialog__actions">
          <Button type="button" variant="ghost" onClick={onClose} disabled={isSubmitting}>
            {t('actions.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            onClick={() => { void handleCreate(); }}
            disabled={!name.trim() || isSubmitting}
            isLoading={isSubmitting}
          >
            {t('nav.groupChats.create')}
          </Button>
        </div>
      </div>
    </Modal>
  );
};

export default CreateGroupChatDialog;
