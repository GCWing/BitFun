import React, { useEffect, useRef, useState } from 'react';
import { Button, Checkbox, Modal, Textarea } from '@/component-library';
import { useTranslation } from 'react-i18next';
import type { FlowChatHeaderCommandSummary } from '../modern/FlowChatHeader';
import './BackgroundCommandInputDialog.scss';

interface BackgroundCommandInputDialogProps {
  command: FlowChatHeaderCommandSummary | null;
  isSending: boolean;
  onClose: () => void;
  onSend: (request: { chars: string; appendEnter: boolean }) => Promise<void>;
}

export const BackgroundCommandInputDialog: React.FC<BackgroundCommandInputDialogProps> = ({
  command,
  isSending,
  onClose,
  onSend,
}) => {
  const { t } = useTranslation('flow-chat');
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const [chars, setChars] = useState('');
  const [appendEnter, setAppendEnter] = useState(true);
  const [maskInput, setMaskInput] = useState(false);

  useEffect(() => {
    if (!command) {
      setChars('');
      setAppendEnter(true);
      setMaskInput(false);
      return;
    }

    const frameId = window.requestAnimationFrame(() => {
      inputRef.current?.focus();
    });

    return () => {
      window.cancelAnimationFrame(frameId);
    };
  }, [command]);

  if (!command) {
    return null;
  }

  const canSend = chars.length > 0 || appendEnter;

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!canSend || isSending) {
      return;
    }
    await onSend({ chars, appendEnter });
  };

  return (
    <Modal
      isOpen={true}
      onClose={isSending ? () => {} : onClose}
      title={t('backgroundCommandInput.title')}
      size="medium"
      closeOnOverlayClick={!isSending}
      contentClassName="background-command-input-dialog__modal"
    >
      <form className="background-command-input-dialog" onSubmit={handleSubmit}>
        <div className="background-command-input-dialog__summary">
          <span className="background-command-input-dialog__summary-label">
            {t('backgroundCommandInput.commandLabel')}
          </span>
          <code>{command.command || command.title}</code>
        </div>

        <Textarea
          ref={inputRef}
          className={maskInput ? 'background-command-input-dialog__textarea background-command-input-dialog__textarea--masked' : 'background-command-input-dialog__textarea'}
          label={t('backgroundCommandInput.inputLabel')}
          value={chars}
          onChange={(event) => setChars(event.target.value)}
          placeholder={t('backgroundCommandInput.inputPlaceholder')}
          rows={4}
          disabled={isSending}
          autoComplete="off"
          spellCheck={false}
        />

        <div className="background-command-input-dialog__options">
          <Checkbox
            checked={appendEnter}
            onChange={(event) => setAppendEnter(event.target.checked)}
            disabled={isSending}
            label={t('backgroundCommandInput.appendEnter')}
          />
          <Checkbox
            checked={maskInput}
            onChange={(event) => setMaskInput(event.target.checked)}
            disabled={isSending}
            label={t('backgroundCommandInput.maskInput')}
          />
        </div>

        <p className="background-command-input-dialog__note">
          {t('backgroundCommandInput.privacyNote')}
        </p>

        <div className="background-command-input-dialog__actions">
          <Button
            type="button"
            variant="secondary"
            size="small"
            onClick={onClose}
            disabled={isSending}
          >
            {t('backgroundCommandInput.cancel')}
          </Button>
          <Button
            type="submit"
            variant="primary"
            size="small"
            isLoading={isSending}
            disabled={!canSend}
          >
            {t('backgroundCommandInput.send')}
          </Button>
        </div>
      </form>
    </Modal>
  );
};

export default BackgroundCommandInputDialog;
