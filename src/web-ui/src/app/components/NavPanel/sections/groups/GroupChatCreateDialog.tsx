/**
 * GroupChatCreateDialog — create-group-chat modal (R-GC-20).
 *
 * 新建群聊：群名 + 选成员（Claw 助理多选）+ 模式默认 Free → createRoom。
 * 成员候选来自 assistantWorkspacesList（Claw 助理工作区）。
 */

import React, { useMemo, useState } from 'react';
import { X, Users } from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { useGroupChatStore } from '../../../../../flow_chat/store/groupChatStore';
import './GroupChatCreateDialog.scss';

export interface GroupChatCreateDialogProps {
  workspacePath: string;
  /** 可加入的 Claw 助理（sessionId + 名称）。 */
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
  // loadMembers 遗留处理（Wave 5 登记）：桌面单工作区场景下 store 内部 workspace
  // 固定 ''；多工作区支持时按契约 workspace_path 传递（R-GC-14/17/18 接线）。
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
        'free', // 模式默认 Free（契约 §1.3 P2-9）
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
    <div data-bf-component="group-chat-create-dialog" className="group-chat-create-dialog__backdrop" onClick={onClose}>
      <div className="group-chat-create-dialog" onClick={(event) => event.stopPropagation()}>
        <header className="group-chat-create-dialog__header">
          <span data-bf-part="title">
            <Users size={14} aria-hidden="true" />
            {t('groupChat.createTitle')}
          </span>
          <button data-bf-part="close" className="group-chat-create-dialog__close" onClick={onClose} aria-label="close">
            <X size={14} />
          </button>
        </header>

        <div className="group-chat-create-dialog__body">
          <label className="group-chat-create-dialog__field">
            <span className="group-chat-create-dialog__label">{t('groupChat.nameLabel')}</span>
            <input
              data-bf-part="nameInput"
              className="group-chat-create-dialog__input"
              value={name}
              placeholder={t('groupChat.namePlaceholder')}
              onChange={(event) => setName(event.target.value)}
              autoFocus
            />
          </label>

          <div className="group-chat-create-dialog__field">
            <span className="group-chat-create-dialog__label">{t('groupChat.membersLabel')}</span>
            <div data-bf-part="memberList" className="group-chat-create-dialog__members">
              {sortedAssistants.length === 0 ? (
                <div className="group-chat-create-dialog__empty">{t('groupChat.noAddable')}</div>
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
          <button data-bf-part="cancel" className="group-chat-create-dialog__cancel" onClick={onClose}>
            {t('groupChat.cancel')}
          </button>
          <button
            data-bf-part="create"
            className="group-chat-create-dialog__create"
            disabled={!canSubmit}
            onClick={handleCreate}
          >
            {t('groupChat.create')}
          </button>
        </footer>
      </div>
    </div>
  );
};

export default GroupChatCreateDialog;
