/**
 * Group chat mention picker (R-GC-16).
 *
 * Shown when the user types `@@` (memberMode, R-GC-15) to mention a group
 * member or @all. Selecting a member returns `{kind:'claw', sessionId}`;
 * selecting @all returns `{kind:'all'}` explicitly (P1-3).
 *
 * Contract: type-contract v1.3 §2.3 (GroupChatMentionPickerProps).
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Users } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import type { GroupChatActor, GroupChatMember } from '../types/flow-chat';
import './GroupChatMentionPicker.scss';

export interface GroupChatMentionPickerProps {
  isOpen: boolean;
  searchQuery: string;
  members: GroupChatMember[];
  onSelect: (target: GroupChatActor) => void;
  onClose: () => void;
}

/** The @all fixed item shown at the top of the picker (P1-4). */
export const GROUP_CHAT_ALL_ITEM = '@all';

export const GroupChatMentionPicker: React.FC<GroupChatMentionPickerProps> = ({
  isOpen,
  searchQuery,
  members,
  onSelect,
  onClose,
}) => {
  const { t } = useI18n('common');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const containerRef = useRef<HTMLDivElement | null>(null);

  const query = searchQuery.startsWith('@') ? searchQuery.slice(1) : searchQuery;

  const filteredMembers = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return members;
    return members.filter(
      (member) =>
        member.sessionId.toLowerCase().includes(q) ||
        (member.displayName ?? '').toLowerCase().includes(q),
    );
  }, [members, query]);

  const items = useMemo(() => {
    const list: Array<{ key: string; actor: GroupChatActor; label: string; isAll: boolean }> = [
      { key: GROUP_CHAT_ALL_ITEM, actor: { kind: 'all' }, label: GROUP_CHAT_ALL_ITEM, isAll: true },
    ];
    for (const member of filteredMembers) {
      list.push({
        key: member.sessionId,
        actor: { kind: 'claw', sessionId: member.sessionId, agentType: member.agentType },
        label: member.displayName ?? member.sessionId,
        isAll: false,
      });
    }
    return list;
  }, [filteredMembers]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [isOpen, searchQuery]);

  const handleSelect = useCallback(
    (actor: GroupChatActor) => {
      onSelect(actor);
      onClose();
    },
    [onSelect, onClose],
  );

  useEffect(() => {
    if (!isOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        setSelectedIndex((index) => Math.min(index + 1, items.length - 1));
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        setSelectedIndex((index) => Math.max(index - 1, 0));
      } else if (event.key === 'Enter') {
        event.preventDefault();
        const item = items[selectedIndex];
        if (item) handleSelect(item.actor);
      } else if (event.key === 'Escape') {
        event.preventDefault();
        onClose();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [isOpen, items, selectedIndex, handleSelect, onClose]);

  if (!isOpen) return null;

  return (
    <div
      ref={containerRef}
      data-bf-component="group-chat-mention-picker"
      data-bf-part="root"
      className="group-chat-mention-picker"
    >
      <div data-bf-component="group-chat-mention-picker" data-bf-part="items">
        {items.map((item, index) => (
          <div
            key={item.key}
            data-bf-component="group-chat-mention-picker"
            data-bf-part="item"
            data-bf-state={index === selectedIndex ? 'selected' : undefined}
            className={`group-chat-mention-picker__item ${index === selectedIndex ? 'group-chat-mention-picker__item--selected' : ''}`}
            onClick={() => handleSelect(item.actor)}
            onMouseEnter={() => setSelectedIndex(index)}
          >
            {item.isAll ? (
              <Users size={13} className="group-chat-mention-picker__icon group-chat-mention-picker__icon--all" />
            ) : (
              <span className="group-chat-mention-picker__avatar">{item.label.charAt(0).toUpperCase()}</span>
            )}
            <span data-bf-component="group-chat-mention-picker" data-bf-part="itemName" className="group-chat-mention-picker__item-name">
              {item.label}
            </span>
            {item.isAll && (
              <span data-bf-component="group-chat-mention-picker" data-bf-part="itemDetail" className="group-chat-mention-picker__item-detail">
                {t('nav.groupChat.allLabel')}
              </span>
            )}
          </div>
        ))}
        {items.length <= 1 && (
          <div className="group-chat-mention-picker__empty">{t('nav.groupChat.noMembers')}</div>
        )}
      </div>
    </div>
  );
};

export default GroupChatMentionPicker;
