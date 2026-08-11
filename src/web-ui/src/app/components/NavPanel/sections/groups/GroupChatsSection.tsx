/**
 * GroupChatsSection — inline accordion content for the "Group Chat" nav item.
 *
 * Lists group chat rooms (name / member count / mode badge). Clicking a room
 * activates it (setActiveRoom) and opens the GroupChatPane. Row menu offers
 * delete (confirm dialog → deleteRoom, P0-3).
 *
 * Contract: type-contract v1.3 §2.3 (R-GC-17).
 */

import React, { useCallback, useEffect, useMemo } from 'react';
import { Trash2, Users, MessageSquare } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { IconButton, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { confirmWarning } from '@/component-library/components/ConfirmDialog/confirmService';
import { useGroupChatStore } from '../../../../../flow_chat/store/groupChatStore';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';

export interface GroupChatsSectionProps {
  workspacePath?: string;
  isVisible?: boolean;
}

export const GroupChatsSection: React.FC<GroupChatsSectionProps> = ({
  workspacePath,
  isVisible = true,
}) => {
  const { t } = useI18n('common');
  const rooms = useGroupChatStore(useShallow((state) => Array.from(state.rooms.values())));
  // P1-2: subscribe to the members Map reference directly (stable under immer,
  // avoiding infinite loops from derived arrays).
  const membersByRoom = useGroupChatStore((state) => state.members);
  const activeRoomId = useGroupChatStore((state) => state.activeRoomId);
  const loadRooms = useGroupChatStore((state) => state.loadRooms);
  const setActiveRoom = useGroupChatStore((state) => state.setActiveRoom);
  const deleteRoom = useGroupChatStore((state) => state.deleteRoom);
  const loadMembers = useGroupChatStore((state) => state.loadMembers);
  const setWorkspacePath = useGroupChatStore((state) => state.setWorkspacePath);
  const { currentWorkspace } = useWorkspaceContext();

  const effectiveWorkspacePath = workspacePath ?? currentWorkspace?.rootPath ?? '';

  const sortedRooms = useMemo(
    () => [...rooms].sort((a, b) => b.lastActiveAt - a.lastActiveAt),
    [rooms],
  );

  /** P1-2 fix: real member count (members Map, P1-11 single source), not memberLimit. */
  const memberCountByRoom = useMemo(() => {
    const map = new Map<string, number>();
    for (const [roomId, members] of membersByRoom.entries()) {
      map.set(roomId, members?.length ?? 0);
    }
    return map;
  }, [membersByRoom]);

  useEffect(() => {
    if (!isVisible) return;
    if (!effectiveWorkspacePath) return;
    // P1-4 fix: loadRooms also syncs the store's workspacePath (consumed by actions).
    setWorkspacePath(effectiveWorkspacePath);
    loadRooms(effectiveWorkspacePath).then((_rooms) => {
      // P2-15: eagerly fetch member counts so the list never shows 0 while
      // members exist (member count is a separate read channel, P1-11).
      const roomIds = Array.from(useGroupChatStore.getState().rooms.keys());
      for (const roomId of roomIds) {
        loadMembers(roomId).catch(() => {
          // Best-effort: a member-count failure must not block the list.
        });
      }
    }).catch(() => {
      // Best-effort: keep the empty state until store data is ready (fail-safe).
    });
  }, [isVisible, effectiveWorkspacePath, loadRooms, loadMembers, setWorkspacePath]);

  const handleRoomClick = useCallback(
    (roomId: string) => {
      setActiveRoom(roomId);
      loadMembers(roomId).catch(() => {
        // Best-effort: member load failure must not block room activation.
      });
    },
    [setActiveRoom, loadMembers],
  );

  const handleDelete = useCallback(
    async (roomId: string, roomName: string) => {
      const confirmed = await confirmWarning(
        t('nav.groupChat.deleteTitle'),
        t('nav.groupChat.deleteConfirm', { name: roomName }),
        { confirmText: t('nav.groupChat.deleteConfirmText') },
      );
      if (!confirmed) return;
      await deleteRoom(roomId, { kind: 'master' });
    },
    [deleteRoom, t],
  );

  if (!isVisible) return null;

  return (
    <div data-bf-component="group-chats-section" data-bf-part="root" className="group-chats-section">
      {sortedRooms.length === 0 ? (
        <div data-bf-component="group-chats-section" data-bf-part="empty" className="group-chats-section__empty">
          {t('nav.groupChat.empty')}
        </div>
      ) : (
        <div data-bf-component="group-chats-section" data-bf-part="items" className="group-chats-section__items">
          {sortedRooms.map((room) => (
            <div
              key={room.roomId}
              data-bf-component="group-chats-section"
              data-bf-part="item"
              data-bf-state={room.roomId === activeRoomId ? 'active' : undefined}
              className={`group-chats-section__item${room.roomId === activeRoomId ? ' group-chats-section__item--active' : ''}`}
              onClick={() => handleRoomClick(room.roomId)}
            >
              <span className="group-chats-section__item-icon" aria-hidden="true">
                <MessageSquare size={13} />
              </span>
              <span data-bf-component="group-chats-section" data-bf-part="itemName" className="group-chats-section__item-name">
                {room.name}
              </span>
              <span data-bf-component="group-chats-section" data-bf-part="itemMeta" className="group-chats-section__item-meta">
                <Users size={10} aria-hidden="true" />
                {memberCountByRoom.get(room.roomId) ?? 0}
              </span>
              <span data-bf-component="group-chats-section" data-bf-part="itemMode" className={`group-chats-section__item-mode group-chats-section__item-mode--${room.mode}`}>
                {room.mode === 'round_robin' ? t('nav.groupChat.modeRoundRobin') : t('nav.groupChat.modeFree')}
              </span>
              <Tooltip content={t('nav.groupChat.deleteTooltip')} placement="right">
                <IconButton
                  size="small"
                  aria-label={t('nav.groupChat.deleteTooltip')}
                  data-bf-action="delete-room"
                  onClick={(event) => {
                    event.stopPropagation();
                    handleDelete(room.roomId, room.name);
                  }}
                >
                  <Trash2 size={11} />
                </IconButton>
              </Tooltip>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default GroupChatsSection;
