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
  // P1-2：直接订阅 members Map 引用（immer 下未变时引用稳定，避免派生数组无限循环）。
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

  /** P1-2 修复：真实成员数（members Map，P1-11 单源），非 memberLimit 上限。 */
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
    // P1-4 修复：loadRooms 同时同步 store 的 workspacePath（各 action 消费）。
    setWorkspacePath(effectiveWorkspacePath);
    loadRooms(effectiveWorkspacePath).catch(() => {
      // Best-effort: store 数据未就绪时保持空态（铁则 6 防呆）。
    });
  }, [isVisible, effectiveWorkspacePath, loadRooms, setWorkspacePath]);

  const handleRoomClick = useCallback(
    (roomId: string) => {
      setActiveRoom(roomId);
      loadMembers(roomId).catch(() => {
        // Best-effort: 成员数据加载失败不影响房间激活。
      });
    },
    [setActiveRoom, loadMembers],
  );

  const handleDelete = useCallback(
    async (roomId: string, roomName: string) => {
      const confirmed = await confirmWarning(
        t('groupChat.deleteTitle'),
        t('groupChat.deleteConfirm', { name: roomName }),
        { confirmText: t('groupChat.deleteConfirmText') },
      );
      if (!confirmed) return;
      await deleteRoom(roomId, { kind: 'master' });
    },
    [deleteRoom, t],
  );

  if (!isVisible) return null;

  return (
    <div data-bf-component="group-chats-section" className="group-chats-section">
      {sortedRooms.length === 0 ? (
        <div data-bf-component="group-chats-section" data-bf-part="empty" className="group-chats-section__empty">
          {t('groupChat.empty')}
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
              <span data-bf-part="itemName" className="group-chats-section__item-name">
                {room.name}
              </span>
              <span data-bf-part="itemMeta" className="group-chats-section__item-meta">
                <Users size={10} aria-hidden="true" />
                {memberCountByRoom.get(room.roomId) ?? 0}
              </span>
              <span data-bf-part="itemMode" className={`group-chats-section__item-mode group-chats-section__item-mode--${room.mode}`}>
                {room.mode === 'round_robin' ? t('groupChat.modeRoundRobin') : t('groupChat.modeFree')}
              </span>
              <Tooltip content={t('groupChat.deleteTooltip')} placement="right">
                <IconButton
                  size="small"
                  aria-label={t('groupChat.deleteTooltip')}
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
