/**
 * GroupChatPane — group chat room chat panel (R-GC-18).
 *
 * Layout mirrors ChatPane: header (room name + member count + mode toggle),
 * message list (simple list rendering GroupChatMessage by author), and the
 * shared full ChatInput. Submissions route through
 * ChatInputRegistration.onSubmit → sendMessage; `@@` member mentions
 * (R-GC-15/16) route through registration.groupChatMention and are carried as
 * group-member session-reference contexts into mentionTargets.
 *
 * Contract: type-contract v1.3 §2.4 (ChatInputRegistration) + R-GC-15/16/18.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { MessageSquare, Repeat, Settings2, Users } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { useOptionalWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useGroupChatStore } from '../store/groupChatStore';
import type { GroupChatActor, GroupChatMember, GroupChatMessage } from '../types/flow-chat';
import type { ChatInputRegistration, ChatInputSubmission } from './chatInputRegistration';
import type { SessionReferenceContext } from '@/shared/types/context';
import { ChatInput } from './ChatInput';
import { GroupChatMemberPicker } from './GroupChatMemberPicker';
import './GroupChatPane.scss';

const EMPTY_MEMBERS: GroupChatMember[] = [];
const EMPTY_MESSAGES: GroupChatMessage[] = [];

// P2-12: configurable timeout scan values (S-90 — no magic numbers in the UI).
// Backend default is group_chat.reply_timeout_secs = 300; kept in one place so
// the UI never hardcodes the value in the component body.
const GROUP_CHAT_REPLY_TIMEOUT_SECS = 300;
const GROUP_CHAT_TIMEOUT_SCAN_INTERVAL_MS = 60_000;

export interface GroupChatPaneProps {
  roomId: string;
  isViewportActive?: boolean;
  /** P2-10: addable Claw assistants (real data source wired by the host). */
  availableAssistants?: { sessionId: string; name: string }[];
}

export const GroupChatPane: React.FC<GroupChatPaneProps> = ({
  roomId,
  isViewportActive = true,
  availableAssistants = [],
}) => {
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
  const setWorkspacePath = useGroupChatStore((state) => state.setWorkspacePath);
  const { workspacePath } = useOptionalWorkspaceContext() ?? { workspacePath: '' };
  const [memberPickerOpen, setMemberPickerOpen] = useState(false);
  const [timeoutReminders, setTimeoutReminders] = useState<
    Array<{ roomId: string; messageId: string; content: string }>
  >([]);
  // P2-8: user-visible error surface (load/send/join/leave failures).
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!roomId || !isViewportActive) return;
    // Task B: the store concentrates the real workspace path; keep it in sync
    // even when the pane mounts directly (without GroupChatsSection).
    if (workspacePath) {
      setWorkspacePath(workspacePath);
    }
    loadMembers(roomId)
      .catch(() => setErrorMessage(t('nav.groupChat.loadFailed')));
    loadMessages(roomId)
      .catch(() => setErrorMessage(t('nav.groupChat.loadFailed')));
  }, [roomId, isViewportActive, loadMembers, loadMessages, setWorkspacePath, workspacePath, t]);

  // P1-1/P2-4/P2-12 fix: timeout-reminder consumer — periodic scan scoped to
  // THIS room (roomId passed so N panes never scan the whole table) using the
  // configurable reply timeout (R-GC-26 reply_timeout_secs). Timed-out
  // messages surface as reminders (system-level notification semantics).
  useEffect(() => {
    if (!isViewportActive) return;
    const scan = () => {
      scanTimeouts(GROUP_CHAT_REPLY_TIMEOUT_SECS, roomId).then((reminders) => {
        if (reminders.length > 0) {
          setTimeoutReminders((prev) => mergeReminders(prev, reminders));
        }
      }).catch(() => {
        // Best-effort: a failed scan must not interrupt the panel.
      });
    };
    scan();
    const timer = window.setInterval(scan, GROUP_CHAT_TIMEOUT_SCAN_INTERVAL_MS); // scan once per minute.
    return () => window.clearInterval(timer);
  }, [isViewportActive, scanTimeouts, roomId]);

  const handleSubmit = useCallback(
    (text: string, author: GroupChatActor, mentionTargets: GroupChatActor[], urgent?: boolean) => {
      if (!text.trim()) return;
      sendMessage(roomId, author, text, mentionTargets, urgent).catch((error) => {
        setErrorMessage(error instanceof Error ? error.message : String(error));
      });
    },
    [roomId, sendMessage],
  );

  // Group chat mentions collected from the shared ChatInput (@@ member mode).
  const [mentionTargets, setMentionTargets] = useState<GroupChatActor[]>([]);

  const handleJoin = useCallback(
    (sessionId: string) => {
      joinRoom(roomId, sessionId, { kind: 'master' }).catch((error) => {
        setErrorMessage(error instanceof Error ? error.message : String(error));
      });
    },
    [roomId, joinRoom],
  );

  const handleLeave = useCallback(
    (sessionId: string) => {
      leaveRoom(roomId, sessionId, { kind: 'master' }).catch((error) => {
        setErrorMessage(error instanceof Error ? error.message : String(error));
      });
    },
    [roomId, leaveRoom],
  );

  const handleSetMode = useCallback(() => {
    setMode(roomId, mode === 'free' ? 'round_robin' : 'free', { kind: 'master' }).catch((error) => {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    });
  }, [roomId, mode, setMode]);

  const registration: ChatInputRegistration = useMemo(
    () => ({
      groupChatMention: {
        members,
        onMentionSelect: (target: GroupChatActor) => {
          setMentionTargets((prev) => {
            const next = prev.filter((existing) =>
              existing.kind === 'claw' && target.kind === 'claw'
                ? existing.sessionId !== target.sessionId
                : true,
            );
            // @all replaces any explicit members; explicit members remove @all.
            if (target.kind === 'all') return [target];
            return [...next.filter((existing) => existing.kind !== 'all'), target];
          });
        },
      },
      onSubmit: (submission: ChatInputSubmission) => {
        const { text, mentionTargets: targets } = buildGroupChatSubmission(submission, mentionTargets);
        setMentionTargets([]);
        handleSubmit(text, { kind: 'master' }, targets, false);
      },
    }),
    [members, mentionTargets, handleSubmit],
  );

  const messageRows = useMemo(
    () => renderMessages(
      messages,
      members,
      t('nav.groupChat.masterLabel'),
      t('nav.groupChat.allLabel'),
    ),
    [messages, members, t],
  );

  if (!room) {
    return (
      <div data-bf-component="group-chat-pane" data-bf-part="empty" className="group-chat-pane group-chat-pane--empty">
        {t('nav.groupChat.paneEmpty')}
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
        <span data-bf-component="group-chat-pane" data-bf-part="memberCount" className="group-chat-pane__meta">
          <Users size={11} aria-hidden="true" />
          {members.length}
        </span>
        <button
          data-bf-component="group-chat-pane"
          data-bf-part="modeToggle"
          className="group-chat-pane__mode-toggle"
          onClick={handleSetMode}
        >
          <Repeat size={12} aria-hidden="true" />
          {mode === 'round_robin' ? t('nav.groupChat.modeRoundRobin') : t('nav.groupChat.modeFree')}
        </button>
        <button
          data-bf-component="group-chat-pane"
          data-bf-part="memberToggle"
          className="group-chat-pane__member-toggle"
          onClick={() => setMemberPickerOpen((open) => !open)}
          aria-label={t('nav.groupChat.manageMembers')}
        >
          <Settings2 size={12} aria-hidden="true" />
        </button>
      </header>
      {errorMessage ? (
        <div data-bf-component="group-chat-pane" data-bf-part="error" className="group-chat-pane__error" role="alert">
          <span>{errorMessage}</span>
          <button
            type="button"
            className="group-chat-pane__error-dismiss"
            aria-label={t('nav.groupChat.dismissError')}
            onClick={() => setErrorMessage(null)}
          >
            ×
          </button>
        </div>
      ) : null}
      {memberPickerOpen ? (
        <GroupChatMemberPicker
          roomId={roomId}
          members={members}
          currentActor={{ kind: 'master' }}
          availableAssistants={availableAssistants}
          onJoin={handleJoin}
          onLeave={handleLeave}
        />
      ) : null}
      {timeoutReminders.length > 0 ? (
        <div data-bf-component="group-chat-pane" data-bf-part="timeoutReminders" className="group-chat-pane__timeout-reminders">
          {timeoutReminders.map((reminder) => (
            <div key={reminder.messageId} data-bf-component="group-chat-pane" data-bf-part="timeoutReminder" className="group-chat-pane__timeout-reminder">
              {t('nav.groupChat.timeoutReminder')}: {reminder.content}
            </div>
          ))}
        </div>
      ) : null}
      <div data-bf-component="group-chat-pane" data-bf-part="messages" className="group-chat-pane__messages">
        {messageRows.length === 0 ? (
          <div className="group-chat-pane__no-messages">{t('nav.groupChat.noMessages')}</div>
        ) : (
          messageRows
        )}
      </div>
      <footer data-bf-component="group-chat-pane" data-bf-part="input" className="group-chat-pane__input">
        <ChatInput isSceneActive={isViewportActive} registration={registration} />
      </footer>
    </div>
  );
};

/** P1-1 fix: merge timeout reminders (dedupe by messageId). */
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

/** Renders one message row with the sender label (master / member displayName). */
function renderMessages(
  messages: GroupChatMessage[],
  members: Array<{ sessionId: string; displayName?: string }>,
  masterLabel: string,
  allLabel: string,
) {
  return messages.map((message) => {
    const senderLabel = authorLabel(message.author, members, masterLabel, allLabel);
    return (
      <div key={message.messageId} data-bf-component="group-chat-pane" data-bf-part="message" className="group-chat-pane__message">
        <span data-bf-component="group-chat-pane" data-bf-part="messageAuthor" className="group-chat-pane__message-author">{senderLabel}</span>
        <span data-bf-component="group-chat-pane" data-bf-part="messageContent" className="group-chat-pane__message-content">{message.content}</span>
      </div>
    );
  });
}

function authorLabel(
  author: GroupChatActor,
  members: Array<{ sessionId: string; displayName?: string }>,
  masterLabel: string,
  allLabel: string,
): string {
  switch (author.kind) {
    case 'master':
      return masterLabel;
    case 'all':
      return allLabel;
    case 'claw': {
      const member = members.find((m) => m.sessionId === author.sessionId);
      return member?.displayName ?? author.sessionId;
    }
  }
}

/**
 * Task A: normalize a ChatInputSubmission into the group chat message body and
 * mention targets. Group-member mentions arrive as session-reference contexts
 * carrying `metadata.groupChatMention` (R-GC-15 `@@`); their display capsules
 * become readable `@name` mentions in the body.
 */
export function buildGroupChatSubmission(
  submission: ChatInputSubmission,
  pendingTargets: GroupChatActor[] = [],
): { text: string; mentionTargets: GroupChatActor[] } {
  const displayText = (submission.displayText ?? submission.text).trim();
  const text = displayText.replace(/\[Session reference:\s*(.+?)\]/g, '@$1');
  const membersFromContexts = (submission.contexts ?? [])
    .filter((context): context is SessionReferenceContext =>
      context.type === 'session-reference' && context.metadata?.groupChatMention !== undefined)
    .map((context) => context.metadata?.groupChatMention as GroupChatActor)
    .filter((target): target is GroupChatActor => target !== undefined);
  // Dedupe by identity: @all is a single fixed target; members key on sessionId.
  const byKey = new Map<string, GroupChatActor>();
  for (const target of [...pendingTargets, ...membersFromContexts]) {
    byKey.set(target.kind === 'claw' ? `claw:${target.sessionId}` : target.kind, target);
  }
  return { text, mentionTargets: [...byKey.values()] };
}

export default GroupChatPane;
