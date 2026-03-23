/**
 * Terminal edit modal
 * Supports editing terminal name and startup command
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Modal, Input, Button } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
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
      if (e.key === 'Escape' && isOpen) {
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
    <Modal isOpen={isOpen} onClose={onClose} title={t('dialog.editTerminal.title')} size="small">
      <div className="terminal-edit-dialog__content">
        <Input
          ref={nameInputRef}
          label={t('dialog.editTerminal.nameLabel')}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={t('dialog.editTerminal.namePlaceholder')}
        />

        {showWorkingDirectory ? (
          <Input
            label={t('dialog.editTerminal.workingDirectoryLabel')}
            value={workingDirectory}
            onChange={(e) => setWorkingDirectory(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('dialog.editTerminal.workingDirectoryPlaceholder')}
            hint={t('dialog.editTerminal.workingDirectoryHint')}
          />
        ) : null}

        {showStartupCommand ? (
          <Input
            label={t('dialog.editTerminal.startupCommandLabel')}
            value={startupCommand}
            onChange={(e) => setStartupCommand(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('dialog.editTerminal.startupCommandPlaceholder')}
            hint={t('dialog.editTerminal.startupCommandHint')}
          />
        ) : null}
      </div>

      <div className="terminal-edit-dialog__footer">
        <Button variant="secondary" onClick={onClose}>
          {t('dialog.editTerminal.cancel')}
        </Button>
        <Button variant="primary" onClick={handleSave} disabled={!canSave}>
          {t('dialog.editTerminal.save')}
        </Button>
      </div>
    </Modal>
  );
};

export default TerminalEditModal;
