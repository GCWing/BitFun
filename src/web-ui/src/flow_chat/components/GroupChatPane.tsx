/**
 * GroupChatPane — group chat room chat panel (R-GC-18).
 *
 * Layout mirrors ChatPane: header (room name + member count + mode toggle),
 * message list (simple list rendering GroupChatMessage by author), and a
 * ChatInput routed through GroupChatRegistration.onSubmit → sendMessage.
 *
 * Contract: type-contract v1.3 §2.4 (GroupChatRegistration) + R-GC-18.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { MessageSquare, Repeat, Settings2, Users } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { useGroupChatStore } from '../store/groupChatStore';
import type { GroupChatActor, GroupChatMember, GroupChatMessage } from '../types/flow-chat';
import type { GroupChatRegistration } from './chatInputRegistration';
import { GroupChatMemberPicker } from './GroupChatMemberPicker';
import './GroupChatPane.scss';

const EMPTY_MEMBERS: GroupChatMember[] = [];
const EMPTY_MESSAGES: GroupChatMessage[] = [];

export interface GroupChatPaneProps {
  roomId: string;
  isViewportActive?: boolean;
}

export const GroupChatPane: React.FC<GroupChatPaneProps> = ({ roomId, isViewportActive = true }) => {
  const { t } = useI18n('common');
  const room = useGroupChatStore((state) => state.rooms.get(roomId) ?? null);
  const members = useGroupChatStore(useShallow((state) => Array.from(state.members.get(roomId) ?? EMPTY_MEMBERS)));
  const messages = useGroupChatStore(useShallow((state) => Array.from(state.messages.get(roomId) ?? EMPTY_MESSAGES)));
  const mode = useGroupChatStore((state) => state.mode);
  const loadMembers = useGroupChatStore((state) => state.loadMembers);
  const loadMessages = useGroupChatStore((state) => state.loadMessages);
  const sendMessage = useGroupChatStore((state) => state.sendMessage);
  const setMode = useGroupChatStore((state) => state.setMode);
  const joinRoom = useGroupChatStore((state) => state.joinRoom);
  const leaveRoom = useGroupChatStore((state) => state.leaveRoom);
  const scanTimeouts = useGroupChatStore((state) => state.scanTimeouts);
  const [memberPickerOpen, setMemberPickerOpen] = useState(false);
  const [timeoutReminders, setTimeoutReminders] = useState<
    Array<{ roomId: string; messageId: string; content: string }>
  >([]);

  useEffect(() => {
    if (!roomId || !isViewportActive) return;
    loadMembers(roomId).catch(() => {});
    loadMessages(roomId).catch(() => {});
  }, [roomId, isViewportActive, loadMembers, loadMessages]);

  // P1-1 修复：超时提醒消费端——定时扫描（默认 300s，R-GC-26 reply_timeout_secs），
  // 超时消息作为提醒展示（系统级通知语义）。
  useEffect(() => {
    if (!isViewportActive) return;
    const scan = () => {
      scanTimeouts(300).then((reminders) => {
        if (reminders.length > 0) {
          setTimeoutReminders((prev) => mergeReminders(prev, reminders));
        }
      }).catch(() => {
        // Best-effort: 扫描失败不打断面板。
      });
    };
    scan();
    const timer = window.setInterval(scan, 60_000); // 每分钟扫描一次。
    return () => window.clearInterval(timer);
  }, [isViewportActive, scanTimeouts]);

  const handleSubmit = useCallback(
    (text: string, author: GroupChatActor, mentionTargets: GroupChatActor[], urgent?: boolean) => {
      if (!text.trim()) return;
      sendMessage(roomId, author, text, mentionTargets, urgent).catch(() => {});
    },
    [roomId, sendMessage],
  );

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const registration: GroupChatRegistration = useMemo(
    () => ({
      roomId,
      onSubmit: (text: string, author: GroupChatActor, mentionTargets: GroupChatActor[], urgent?: boolean) =>
        handleSubmit(text, author, mentionTargets, urgent),
    }),
    [roomId, handleSubmit],
  );

  const messageRows = useMemo(() => renderMessages(messages, members), [messages, members]);

  if (!room) {
    return (
      <div data-bf-component="group-chat-pane" data-bf-part="empty" className="group-chat-pane group-chat-pane--empty">
        {t('groupChat.paneEmpty')}
      </div>
    );
  }

  return (
    <div data-bf-component="group-chat-pane" data-bf-part="root" className="group-chat-pane">
      <header data-bf-component="group-chat-pane" data-bf-part="header" className="group-chat-pane__header">
        <span className="group-chat-pane__title">
          <MessageSquare size={14} aria-hidden="true" />
          {room.name}
        </span>
        <span data-bf-part="memberCount" className="group-chat-pane__meta">
          <Users size={11} aria-hidden="true" />
          {members.length}
        </span>
        <button
          data-bf-component="group-chat-pane"
          data-bf-part="modeToggle"
          className="group-chat-pane__mode-toggle"
          onClick={() => setMode(roomId, mode === 'free' ? 'round_robin' : 'free', { kind: 'master' })}
        >
          <Repeat size={12} aria-hidden="true" />
          {mode === 'round_robin' ? t('groupChat.modeRoundRobin') : t('groupChat.modeFree')}
        </button>
        <button
          data-bf-component="group-chat-pane"
          data-bf-part="memberToggle"
          className="group-chat-pane__member-toggle"
          onClick={() => setMemberPickerOpen((open) => !open)}
          aria-label={t('groupChat.manageMembers')}
        >
          <Settings2 size={12} aria-hidden="true" />
        </button>
      </header>
      {memberPickerOpen ? (
        <GroupChatMemberPicker
          roomId={roomId}
          members={members}
          currentActor={{ kind: 'master' }}
          availableAssistants={[]}
          onJoin={(sessionId) => joinRoom(roomId, sessionId, { kind: 'master' }).catch(() => {})}
          onLeave={(sessionId) => leaveRoom(roomId, sessionId, { kind: 'master' }).catch(() => {})}
        />
      ) : null}
      {timeoutReminders.length > 0 ? (
        <div data-bf-component="group-chat-pane" data-bf-part="timeoutReminders" className="group-chat-pane__timeout-reminders">
          {timeoutReminders.map((reminder) => (
            <div key={reminder.messageId} data-bf-part="timeoutReminder" className="group-chat-pane__timeout-reminder">
              {t('groupChat.timeoutReminder')}: {reminder.content}
            </div>
          ))}
        </div>
      ) : null}
      <div data-bf-component="group-chat-pane" data-bf-part="messages" className="group-chat-pane__messages">
        {messageRows.length === 0 ? (
          <div className="group-chat-pane__no-messages">{t('groupChat.noMessages')}</div>
        ) : (
          messageRows
        )}
      </div>
      <footer data-bf-component="group-chat-pane" data-bf-part="input" className="group-chat-pane__input">
        {renderChatInput(registration)}
      </footer>
    </div>
  );
};

/** P1-1 修复：合并超时提醒（按 messageId 去重）。 */
function mergeReminders(
  prev: Array<{ roomId: string; messageId: string; content: string }>,
  next: Array<{ roomId: string; messageId: string; content: string }>,
): Array<{ roomId: string; messageId: string; content: string }> {
  const byId = new Map(prev.map((reminder) => [reminder.messageId, reminder]));
  for (const reminder of next) {
    byId.set(reminder.messageId, reminder);
  }
  return Array.from(byId.values());
}

/** Renders one message row with the sender label (主人 / member displayName). */
function renderMessages(messages: GroupChatMessage[], members: Array<{ sessionId: string; displayName?: string }>) {
  return messages.map((message) => {
    const senderLabel = authorLabel(message.author, members);
    return (
      <div key={message.messageId} data-bf-component="group-chat-pane" data-bf-part="message" className="group-chat-pane__message">
        <span data-bf-part="messageAuthor" className="group-chat-pane__message-author">{senderLabel}</span>
        <span data-bf-part="messageContent" className="group-chat-pane__message-content">{message.content}</span>
      </div>
    );
  });
}

function authorLabel(
  author: GroupChatActor,
  members: Array<{ sessionId: string; displayName?: string }>,
): string {
  switch (author.kind) {
    case 'master':
      return '主人';
    case 'all':
      return '@全体';
    case 'claw': {
      const member = members.find((m) => m.sessionId === author.sessionId);
      return member?.displayName ?? author.sessionId;
    }
  }
}

/** Minimal ChatInput adapter: GroupChatPane keeps a simple text input routed to sendMessage. */
function renderChatInput(registration: GroupChatRegistration) {
  return (
    <GroupChatTextInput registration={registration} />
  );
}

function GroupChatTextInput({ registration }: { registration: GroupChatRegistration }) {
  const [text, setText] = React.useState('');
  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      const trimmed = text.trim();
      if (!trimmed) return;
      registration.onSubmit(trimmed, { kind: 'master' }, [], false);
      setText('');
    }
  };
  return (
    <input
      data-bf-component="group-chat-pane"
      data-bf-part="textInput"
      className="group-chat-pane__text-input"
      value={text}
      placeholder="..."
      onChange={(event) => setText(event.target.value)}
      onKeyDown={handleKeyDown}
    />
  );
}

export default GroupChatPane;
