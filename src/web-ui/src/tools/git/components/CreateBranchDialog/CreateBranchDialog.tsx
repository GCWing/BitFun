/**
 * Create-branch dialog for Git.
 * Provides a consistent UI for creating a new branch from an existing base branch.
 */

import React, { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { GitBranch } from 'lucide-react';
import { Button, Modal, Input } from '@/component-library';
import './CreateBranchDialog.scss';

export interface CreateBranchDialogProps {
  /** Whether the dialog is open */
  isOpen: boolean;
  /** Base branch name */
  baseBranch: string;
  /** Confirm callback */
  onConfirm: (branchName: string) => void | Promise<void>;
  /** Cancel callback */
  onCancel: () => void;
  /** Whether branch creation is in progress */
  isCreating?: boolean;
  /** Existing branches (used to prevent duplicates) */
  existingBranches?: string[];
}

export const CreateBranchDialog: React.FC<CreateBranchDialogProps> = ({
  isOpen,
  baseBranch,
  onConfirm,
  onCancel,
  isCreating = false,
  existingBranches = []
}) => {
  const { t } = useTranslation('panels/git');
  const [branchName, setBranchName] = useState('');
  const [error, setError] = useState('');

  useEffect(() => {
    if (!isOpen) {
      setBranchName('');
      setError('');
    }
  }, [isOpen]);

  const validateBranchName = useCallback((name: string): string => {
    if (!name.trim()) {
      return t('validation.branchNameEmpty');
    }

    if (name.includes('..')) {
      return t('validation.branchNameContainsDoubleDot');
    }
    if (name.includes('//')) {
      return t('validation.branchNameContainsDoubleSlash');
    }
    if (name.startsWith('.') || name.endsWith('.')) {
      return t('validation.branchNameDotEdge');
    }
    if (name.startsWith('/') || name.endsWith('/')) {
      return t('validation.branchNameSlashEdge');
    }
    if (name.includes(' ')) {
      return t('validation.branchNameSpaces');
    }
    if (['^', '~', ':', '?', '*', '[', '\\'].some((char) => name.includes(char))) {
      return t('validation.branchNameSpecialChars');
    }
    if (name.includes('@{')) {
      return t('validation.branchNameAtBrace');
    }


    if (existingBranches.includes(name)) {
      return t('validation.branchNameExistsWithName', { name });
    }

    return '';
  }, [existingBranches, t]);

  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setBranchName(value);
    

    const validationError = validateBranchName(value);
    setError(validationError);
  }, [validateBranchName]);

  const handleConfirm = useCallback(async () => {
    const trimmedName = branchName.trim();
    

    const validationError = validateBranchName(trimmedName);
    if (validationError) {
      setError(validationError);
      return;
    }
    
    await onConfirm(trimmedName);
  }, [branchName, validateBranchName, onConfirm]);

  const handleCancel = useCallback(() => {
    setBranchName('');
    setError('');
    onCancel();
  }, [onCancel]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      handleCancel();
    } else if (e.key === 'Enter' && !error && branchName.trim()) {
      e.preventDefault();
      handleConfirm();
    }
  }, [handleCancel, handleConfirm, error, branchName]);

  const canSubmit = branchName.trim().length > 0 && !error && !isCreating;

  return (
    <Modal isOpen={isOpen} onClose={handleCancel} title={t('dialog.createNewBranch.title')} size="small">
      <div className="bitfun-create-branch-dialog" onKeyDown={handleKeyDown}>
        <div className="bitfun-create-branch-dialog__base-info">
          <div className="bitfun-create-branch-dialog__base-label">
            <GitBranch size={14} />
            <span>{t('dialog.createNewBranch.baseBranch')}</span>
          </div>
          <div className="bitfun-create-branch-dialog__base-value">
            {baseBranch}
          </div>
        </div>

        <div className="bitfun-create-branch-dialog__form">
          <Input
            label={t('dialog.createNewBranch.nameLabel')}
            value={branchName}
            onChange={handleInputChange}
            placeholder={t('dialog.createNewBranch.namePlaceholder')}
            disabled={isCreating}
            autoFocus
            error={!!error}
            errorMessage={error}
          />
          <div className="bitfun-create-branch-dialog__hint">
            <div>{t('dialog.createNewBranch.namingHintTitle')}</div>
            <ul>
              <li>
                {t('dialog.createNewBranch.namingHints.featureLabel')} <code>{t('dialog.createNewBranch.namingHints.featureExample')}</code>
              </li>
              <li>
                {t('dialog.createNewBranch.namingHints.bugfixLabel')} <code>{t('dialog.createNewBranch.namingHints.bugfixExample')}</code>
              </li>
              <li>
                {t('dialog.createNewBranch.namingHints.hotfixLabel')} <code>{t('dialog.createNewBranch.namingHints.hotfixExample')}</code>
              </li>
              <li>
                {t('dialog.createNewBranch.namingHints.releaseLabel')} <code>{t('dialog.createNewBranch.namingHints.releaseExample')}</code>
              </li>
            </ul>
          </div>
        </div>

        <div className="bitfun-create-branch-dialog__actions">
          <Button 
            variant="secondary"
            size="small"
            onClick={handleCancel}
            disabled={isCreating}
          >
            {t('dialog.createNewBranch.cancel')}
          </Button>
          <Button 
            variant="primary"
            size="small"
            onClick={handleConfirm}
            disabled={!canSubmit}
            isLoading={isCreating}
          >
            <GitBranch size={14} />
            {t('dialog.createNewBranch.confirm')}
          </Button>
        </div>

        <div className="bitfun-create-branch-dialog__shortcuts">
          <span>Esc</span> {t('dialog.createNewBranch.cancel')} · <span>Enter</span> {t('dialog.createNewBranch.confirm')}
        </div>
      </div>
    </Modal>
  );
};

export default CreateBranchDialog;
