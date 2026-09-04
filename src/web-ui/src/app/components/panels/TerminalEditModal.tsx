/**
 * Terminal edit modal
 * Supports editing terminal name and startup command
 */

import {
  Button,
  Field,
  Input,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@openbitfun/ui';
import React, { useState, useEffect, useRef, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { isImeOwnedKeyboardEvent } from '@/shared/utils/ime';
import './TerminalEditModal.scss';

export interface TerminalEditModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (input: { name: string; workingDirectory?: string; startupCommand?: string }) => void;
  initialName: string;
  initialWorkingDirectory?: string;
  initialStartupCommand?: string;
  showWorkingDirectory?: boolean;
  showStartupCommand?: boolean;
}

export const TerminalEditModal: React.FC<TerminalEditModalProps> = ({
  isOpen,
  onClose,
  onSave,
  initialName,
  initialWorkingDirectory = '',
  initialStartupCommand = '',
  showWorkingDirectory = true,
  showStartupCommand = true,
}) => {
  const { t } = useI18n('panels/terminal');
  const [name, setName] = useState(initialName);
  const [workingDirectory, setWorkingDirectory] = useState(initialWorkingDirectory);
  const [startupCommand, setStartupCommand] = useState(initialStartupCommand);
  const nameInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setName(initialName);
      setWorkingDirectory(initialWorkingDirectory);
      setStartupCommand(initialStartupCommand);
      setTimeout(() => {
        nameInputRef.current?.focus();
        nameInputRef.current?.select();
      }, 100);
    }
  }, [initialName, initialStartupCommand, initialWorkingDirectory, isOpen]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen && !isImeOwnedKeyboardEvent(e)) {
        onClose();
      }
    };
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, onClose]);

  const handleSave = useCallback(() => {
    const trimmedName = name.trim();
    if (!trimmedName) return;

    const trimmedWorkingDirectory = workingDirectory.trim();
    const trimmedCommand = startupCommand.trim();
    onSave({
      name: trimmedName,
      workingDirectory: trimmedWorkingDirectory || undefined,
      startupCommand: trimmedCommand || undefined,
    });
    onClose();
  }, [name, onClose, onSave, startupCommand, workingDirectory]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSave();
    }
  }, [handleSave]);

  const canSave = name.trim().length > 0;

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) onClose(); }}
      size="sm"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{t('dialog.editTerminal.title')}</DialogTitle>
        </DialogHeading>
        <DialogClose />
      </DialogHeader>
      <DialogBody inset="none">
      <div data-openbitfun-component="terminal-edit-modal" data-openbitfun-part="content" className="terminal-edit-dialog__content">
        <Field label={t('dialog.editTerminal.nameLabel')}>
          <Input
            ref={nameInputRef}
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('dialog.editTerminal.namePlaceholder')}
          />
        </Field>

        {showWorkingDirectory ? (
          <Field
            description={t('dialog.editTerminal.workingDirectoryHint')}
            label={t('dialog.editTerminal.workingDirectoryLabel')}
          >
            <Input
              value={workingDirectory}
              onChange={(e) => setWorkingDirectory(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t('dialog.editTerminal.workingDirectoryPlaceholder')}
            />
          </Field>
        ) : null}

        {showStartupCommand ? (
          <Field
            description={t('dialog.editTerminal.startupCommandHint')}
            label={t('dialog.editTerminal.startupCommandLabel')}
          >
            <Input
              value={startupCommand}
              onChange={(e) => setStartupCommand(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t('dialog.editTerminal.startupCommandPlaceholder')}
            />
          </Field>
        ) : null}
      </div>

      <div data-openbitfun-component="terminal-edit-modal" data-openbitfun-part="footer" className="terminal-edit-dialog__footer">
        <Button variant="outline" onClick={onClose}>
          {t('dialog.editTerminal.cancel')}
        </Button>
        <Button variant="fill" onClick={handleSave} disabled={!canSave}>
          {t('dialog.editTerminal.save')}
        </Button>
      </div>
          </DialogBody>
    </Dialog>
  );
};

export default TerminalEditModal;
