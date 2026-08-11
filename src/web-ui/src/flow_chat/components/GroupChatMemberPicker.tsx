/**
 * GroupChatMemberPicker — member management panel (R-GC-19, P1-4 定标).
 *
 * 职责 = 成员管理面板（拉人/踢人/角色），非 @ 选择器。当前用户为 Owner 或
 * 主人时可踢人；非 Owner 踢人入口隐藏/禁用。
 *
 * Contract: type-contract v1.3 §2.3 (GroupChatMemberPickerProps).
 */

import React, { useCallback, useMemo, useState } from 'react';
import { UserPlus, UserMinus, Crown } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import type { GroupChatActor, GroupChatMember } from '../types/flow-chat';
import './GroupChatMemberPicker.scss';

export interface GroupChatMemberPickerProps {
  roomId: string;
  members: GroupChatMember[];
  currentActor: GroupChatActor;
  availableAssistants: { sessionId: string; name: string }[];  // 可加入的 Claw 助理
  onJoin: (sessionId: string) => void;
  onLeave: (sessionId: string) => void;
}

export const GroupChatMemberPicker: React.FC<GroupChatMemberPickerProps> = ({
  members,
  currentActor,
  availableAssistants,
  onJoin,
  onLeave,
}) => {
  const { t } = useI18n('common');
  const [showAddList, setShowAddList] = useState(false);

  /** 主人例外（P0-2/P1-4）：matches 枚举，禁字符串比较。 */
  const isMaster = currentActor.kind === 'master';
  /** 当前 Claw 是否为 Owner。 */
  const isOwnerClaw =
    currentActor.kind === 'claw' &&
    members.some(
      (member) => member.sessionId === currentActor.sessionId && member.role === 'owner',
    );
  const canManage = isMaster || isOwnerClaw;

  const addable = useMemo(
    () =>
      availableAssistants.filter(
        (assistant) => !members.some((member) => member.sessionId === assistant.sessionId),
      ),
    [availableAssistants, members],
  );

  const handleJoin = useCallback(
    (sessionId: string) => {
      onJoin(sessionId);
      setShowAddList(false);
    },
    [onJoin],
  );

  return (
    <div data-bf-component="group-chat-member-picker" className="group-chat-member-picker">
      <div data-bf-part="members" className="group-chat-member-picker__members">
        {members.length === 0 ? (
          <div className="group-chat-member-picker__empty">{t('groupChat.noMembers')}</div>
        ) : (
          members.map((member) => (
            <div
              key={member.sessionId}
              data-bf-component="group-chat-member-picker"
              data-bf-part="member"
              data-bf-state={member.role === 'owner' ? 'owner' : 'member'}
              className="group-chat-member-picker__member"
            >
              <span data-bf-part="memberName" className="group-chat-member-picker__member-name">
                {member.displayName ?? member.sessionId}
              </span>
              <span data-bf-part="memberRole" className={`group-chat-member-picker__role group-chat-member-picker__role--${member.role}`}>
                {member.role === 'owner' ? <Crown size={11} aria-hidden="true" /> : null}
                {member.role === 'owner' ? t('groupChat.roleOwner') : t('groupChat.roleMember')}
              </span>
              {canManage && member.role !== 'owner' ? (
                <button
                  data-bf-component="group-chat-member-picker"
                  data-bf-part="leaveButton"
                  className="group-chat-member-picker__leave"
                  aria-label={t('groupChat.leaveTooltip')}
                  onClick={() => onLeave(member.sessionId)}
                >
                  <UserMinus size={12} />
                </button>
              ) : null}
            </div>
          ))
        )}
      </div>

      {canManage ? (
        <div className="group-chat-member-picker__add">
          <button
            data-bf-component="group-chat-member-picker"
            data-bf-part="addToggle"
            className="group-chat-member-picker__add-toggle"
            onClick={() => setShowAddList((open) => !open)}
          >
            <UserPlus size={12} aria-hidden="true" />
            {t('groupChat.addMember')}
          </button>
          {showAddList && (
            <div data-bf-part="addList" className="group-chat-member-picker__add-list">
              {addable.length === 0 ? (
                <div className="group-chat-member-picker__empty">{t('groupChat.noAddable')}</div>
              ) : (
                addable.map((assistant) => (
                  <button
                    key={assistant.sessionId}
                    data-bf-component="group-chat-member-picker"
                    data-bf-part="addItem"
                    className="group-chat-member-picker__add-item"
                    onClick={() => handleJoin(assistant.sessionId)}
                  >
                    {assistant.name}
                  </button>
                ))
              )}
            </div>
          )}
        </div>
      ) : null}
    </div>
  );
};

export default GroupChatMemberPicker;
