/**
 * GroupChatCreateDialog — create-group-chat modal (R-GC-20).
 *
 * Creates a group chat: name + member selection (multi-select Claw assistants)
 * + mode defaulting to Free -> createRoom. Member candidates come from
 * assistantWorkspacesList (Claw assistant workspaces).
 */

import React, { useMemo, useState } from 'react';
import { X, Users } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { useGroupChatStore } from '../../../../../flow_chat/store/groupChatStore';
import './GroupChatCreateDialog.scss';

export interface GroupChatCreateDialogProps {
  workspacePath: string;
  /** Addable Claw assistants (sessionId + name). */
  availableAssistants?: { sessionId: string; name: string }[];
  onClose: () => void;
}

export const GroupChatCreateDialog: React.FC<GroupChatCreateDialogProps> = ({
  workspacePath,
  availableAssistants = [],
  onClose,
}) => {
  const { t } = useI18n('common');
  const createRoom = useGroupChatStore((state) => state.createRoom);
  const [name, setName] = useState('');
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // loadMembers legacy handling (Wave 5 registration): in the desktop
  // single-workspace scenario the store's internal workspace stays '';
  // pass workspace_path per contract when multi-workspace lands (R-GC-14/17/18).
  void workspacePath;

  const toggleMember = (sessionId: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(sessionId)) {
        next.delete(sessionId);
      } else {
        next.add(sessionId);
      }
      return next;
    });
  };

  const canSubmit = name.trim().length > 0 && selected.size > 0 && !submitting;

  const handleCreate = async () => {
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      await createRoom(
        name.trim(),
        { kind: 'master' },
        Array.from(selected),
        'free', // mode defaults to Free (contract §1.3 P2-9)
      );
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setSubmitting(false);
    }
  };

  const sortedAssistants = useMemo(
    () => [...availableAssistants].sort((a, b) => a.name.localeCompare(b.name)),
    [availableAssistants],
  );

  return (
    <div data-bf-component="group-chat-create-dialog" data-bf-part="backdrop" className="group-chat-create-dialog__backdrop" onClick={onClose}>
      <div className="group-chat-create-dialog" onClick={(event) => event.stopPropagation()}>
        <header className="group-chat-create-dialog__header">
          <span data-bf-component="group-chat-create-dialog" data-bf-part="title">
            <Users size={14} aria-hidden="true" />
            {t('nav.groupChat.createTitle')}
          </span>
          <button data-bf-component="group-chat-create-dialog" data-bf-part="close" className="group-chat-create-dialog__close" onClick={onClose} aria-label="close">
            <X size={14} />
          </button>
        </header>

        <div className="group-chat-create-dialog__body">
          <label className="group-chat-create-dialog__field">
            <span className="group-chat-create-dialog__label">{t('nav.groupChat.nameLabel')}</span>
            <input
              data-bf-component="group-chat-create-dialog"
              data-bf-part="nameInput"
              className="group-chat-create-dialog__input"
              value={name}
              placeholder={t('nav.groupChat.namePlaceholder')}
              onChange={(event) => setName(event.target.value)}
              autoFocus
            />
          </label>

          <div className="group-chat-create-dialog__field">
            <span className="group-chat-create-dialog__label">{t('nav.groupChat.membersLabel')}</span>
            <div data-bf-component="group-chat-create-dialog" data-bf-part="memberList" className="group-chat-create-dialog__members">
              {sortedAssistants.length === 0 ? (
                <div className="group-chat-create-dialog__empty">{t('nav.groupChat.noAddable')}</div>
              ) : (
                sortedAssistants.map((assistant) => (
                  <label
                    key={assistant.sessionId}
                    data-bf-component="group-chat-create-dialog"
                    data-bf-part="memberOption"
                    data-bf-state={selected.has(assistant.sessionId) ? 'selected' : undefined}
                    className="group-chat-create-dialog__member-option"
                  >
                    <input
                      type="checkbox"
                      checked={selected.has(assistant.sessionId)}
                      onChange={() => toggleMember(assistant.sessionId)}
                    />
                    <span>{assistant.name}</span>
                  </label>
                ))
              )}
            </div>
          </div>

          {error ? <div className="group-chat-create-dialog__error">{error}</div> : null}
        </div>

        <footer className="group-chat-create-dialog__footer">
          <button data-bf-component="group-chat-create-dialog" data-bf-part="cancel" className="group-chat-create-dialog__cancel" onClick={onClose}>
            {t('nav.groupChat.cancel')}
          </button>
          <button
            data-bf-component="group-chat-create-dialog"
            data-bf-part="create"
            className="group-chat-create-dialog__create"
            disabled={!canSubmit}
            onClick={handleCreate}
          >
            {t('nav.groupChat.create')}
          </button>
        </footer>
      </div>
    </div>
  );
};

export default GroupChatCreateDialog;
