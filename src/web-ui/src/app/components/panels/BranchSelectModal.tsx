/**
 * Branch selection modal
 * Supports selecting existing branches or creating new branches
 */

import React, { useState, useEffect, useMemo, useCallback, useRef } from 'react';
import { createPortal } from 'react-dom';
import { GitBranch, Plus, X } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import { IconButton, Button, Input, Checkbox } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { gitAPI, type GitBranch as GitBranchType } from '../../../infrastructure/api/service-api/GitAPI';
import './BranchSelectModal.scss';

const log = createLogger('BranchSelectModal');

type SelectableBranch = GitBranchType & {
  isCurrent?: boolean;
  hasWorktree?: boolean;
};

export interface BranchSelectResult {
  branch: string;
  isNew: boolean;
  openAfterCreate: boolean;
}

export interface BranchSelectModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSelect: (result: BranchSelectResult) => void;
  repositoryPath: string;
  title?: string;
  currentBranch?: string;
  existingWorktreeBranches?: string[];
  showOpenAfterCreate?: boolean;
  defaultOpenAfterCreate?: boolean;
}

export const BranchSelectModal: React.FC<BranchSelectModalProps> = ({
  isOpen,
  onClose,
  onSelect,
  repositoryPath,
  title,
  currentBranch,
  existingWorktreeBranches = [],
  showOpenAfterCreate = false,
  defaultOpenAfterCreate = false,
}) => {
  const { t } = useI18n('panels/git');
  const { t: tCommon } = useI18n('common');
  const [branches, setBranches] = useState<GitBranchType[]>([]);
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedBranch, setSelectedBranch] = useState<string | null>(null);
  const [isNewBranch, setIsNewBranch] = useState(false);
  const [openAfterCreate, setOpenAfterCreate] = useState(defaultOpenAfterCreate);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const resolvedTitle = title ?? t('branchSelect.title');


  useEffect(() => {
    if (isOpen && repositoryPath) {
      loadBranches();
    }
  }, [isOpen, repositoryPath]);

  useEffect(() => {
    if (!isOpen) {
      setSearchTerm('');
      setSelectedBranch(null);
      setIsNewBranch(false);
      setOpenAfterCreate(defaultOpenAfterCreate);
      setError(null);
    } else {
      setTimeout(() => {
        inputRef.current?.focus();
      }, 100);
    }
  }, [defaultOpenAfterCreate, isOpen]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    };
    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, onClose]);

  const loadBranches = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const branchList = await gitAPI.getBranches(repositoryPath, false);
      setBranches(branchList);
    } catch (err) {
      log.error('Failed to load branches', err);
      setError(t('branchSelect.errors.loadFailed'));
    } finally {
      setIsLoading(false);
    }
  };

  const filteredBranches = useMemo<SelectableBranch[]>(() => {
    let result = branches;

    if (searchTerm.trim()) {
      const lowerSearch = searchTerm.toLowerCase();
      result = result.filter(branch =>
        branch.name.toLowerCase().includes(lowerSearch)
      );
    }

    const availableBranches: SelectableBranch[] = [];
    const unavailableBranches: SelectableBranch[] = [];
    const existingWorktreeSet = new Set(existingWorktreeBranches);

    result.forEach(branch => {
      const isCurrent = branch.name === currentBranch;
      const hasWorktree = existingWorktreeSet.has(branch.name);

      if (isCurrent || hasWorktree) {
        unavailableBranches.push({ ...branch, isCurrent, hasWorktree });
      } else {
        availableBranches.push(branch);
      }
    });

    return [...availableBranches, ...unavailableBranches];
  }, [branches, searchTerm, currentBranch, existingWorktreeBranches]);

  const canCreateNewBranch = useMemo(() => {
    if (!searchTerm.trim()) return false;
    const exists = branches.some(
      branch => branch.name.toLowerCase() === searchTerm.toLowerCase()
    );
    return !exists;
  }, [branches, searchTerm]);

  const handleSelectBranch = useCallback((branchName: string, isNew: boolean) => {
    setSelectedBranch(branchName);
    setIsNewBranch(isNew);
  }, []);

  const handleConfirm = useCallback(() => {
    if (selectedBranch) {
      onSelect({
        branch: selectedBranch,
        isNew: isNewBranch,
        openAfterCreate,
      });
      onClose();
    }
  }, [selectedBranch, isNewBranch, onClose, onSelect, openAfterCreate]);

  const handleDoubleClick = useCallback((branchName: string, isNew: boolean) => {
    onSelect({
      branch: branchName,
      isNew: isNew,
      openAfterCreate,
    });
    onClose();
  }, [onClose, onSelect, openAfterCreate]);

  if (!isOpen) return null;

  const modalContent = (
    <div className="branch-select-overlay" onClick={onClose}>
      <div className="branch-select-dialog" onClick={(e) => e.stopPropagation()}>
        <IconButton 
          className="branch-select-dialog__close"
          variant="ghost"
          size="xs"
          onClick={onClose}
          tooltip={tCommon('actions.close')}
        >
          <X size={14} />
        </IconButton>

        <div className="branch-select-dialog__header">
          <h2 className="branch-select-dialog__title">{resolvedTitle}</h2>
        </div>

        <div className="branch-select-dialog__content">
          <div className="branch-select-dialog__input-wrapper">
            <Input
              ref={inputRef}
              type="text"
              placeholder={t('branchSelect.inputPlaceholder')}
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="branch-select-dialog__input"
            />
          </div>

          {error && (
            <div className="branch-select-dialog__error">
              {error}
            </div>
          )}

          <div className="branch-select-dialog__list">
            {isLoading ? (
              <div className="branch-select-dialog__loading">
                <div className="branch-select-dialog__loading-dots">
                  <span></span>
                  <span></span>
                  <span></span>
                </div>
                  <span>{t('branchSelect.loading')}</span>
              </div>
            ) : (
              <>
                {canCreateNewBranch && (
                  <div
                    className={`branch-select-dialog__item branch-select-dialog__item--new ${
                      selectedBranch === searchTerm && isNewBranch ? 'selected' : ''
                    }`}
                    onClick={() => handleSelectBranch(searchTerm.trim(), true)}
                    onDoubleClick={() => handleDoubleClick(searchTerm.trim(), true)}
                  >
                    <Plus size={14} className="branch-select-dialog__item-icon branch-select-dialog__item-icon--new" />
                    <span className="branch-select-dialog__item-name">
                      {t('branchSelect.createNewLabel')} <strong>{searchTerm.trim()}</strong>
                    </span>
                  </div>
                )}

                {filteredBranches.map((branch) => {
                  const isDisabled = branch.isCurrent || branch.hasWorktree;
                  const hasWorktree = branch.hasWorktree;

                  return (
                    <div
                      key={branch.name}
                      className={`branch-select-dialog__item ${
                        selectedBranch === branch.name && !isNewBranch ? 'selected' : ''
                      } ${branch.current ? 'current' : ''} ${isDisabled ? 'disabled' : ''}`}
                      onClick={() => !isDisabled && handleSelectBranch(branch.name, false)}
                      onDoubleClick={() => !isDisabled && handleDoubleClick(branch.name, false)}
                    >
                      <GitBranch size={14} className="branch-select-dialog__item-icon" />
                      <span className="branch-select-dialog__item-name">
                        {branch.name}
                      </span>
                      {branch.current && (
                        <span className="branch-select-dialog__item-badge">{t('branch.current')}</span>
                      )}
                      {hasWorktree && !branch.current && (
                        <span className="branch-select-dialog__item-badge branch-select-dialog__item-badge--worktree">
                          {t('branchSelect.badges.inUse')}
                        </span>
                      )}
                    </div>
                  );
                })}

                {filteredBranches.length === 0 && !canCreateNewBranch && (
                  <div className="branch-select-dialog__empty">
                    {searchTerm ? t('empty.noMatchingBranches') : t('empty.noBranches')}
                  </div>
                )}
              </>
            )}
          </div>
        </div>

        <div className="branch-select-dialog__footer">
          {showOpenAfterCreate ? (
            <div className="branch-select-dialog__options">
              <Checkbox
                checked={openAfterCreate}
                onChange={(event) => setOpenAfterCreate(event.target.checked)}
                label={t('branchSelect.openAfterCreate.label')}
                description={t('branchSelect.openAfterCreate.description')}
              />
            </div>
          ) : null}
          <Button
            className="branch-select-dialog__btn branch-select-dialog__btn--cancel"
            variant="ghost"
            onClick={onClose}
          >
            {tCommon('actions.cancel')}
          </Button>
          <Button
            className="branch-select-dialog__btn branch-select-dialog__btn--confirm"
            variant="primary"
            onClick={handleConfirm}
            disabled={!selectedBranch}
          >
            {tCommon('actions.confirm')}
          </Button>
        </div>
      </div>
    </div>
  );

  if (typeof document === 'undefined') {
    return modalContent;
  }

  return createPortal(modalContent, document.body);
};

export default BranchSelectModal;
