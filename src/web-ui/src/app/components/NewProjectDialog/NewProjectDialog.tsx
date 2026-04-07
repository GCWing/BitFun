/**
 * New Project Dialog Component
 */

import React, { useState, useCallback, useMemo } from 'react';
import { 
  FolderPlus, 
  FolderOpen, 
  FileText,
  FolderTree,
  AlertCircle,
  Check,
  X
} from 'lucide-react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTranslation } from 'react-i18next';
import { createLogger } from '@/shared/utils/logger';
import { Modal, Button, Input } from '@/component-library';
import './NewProjectDialog.scss';

const log = createLogger('NewProjectDialog');

export interface NewProjectDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (parentPath: string, projectName: string) => Promise<void>;
  defaultParentPath?: string;
}

export const NewProjectDialog: React.FC<NewProjectDialogProps> = ({
  isOpen,
  onClose,
  onConfirm,
  defaultParentPath
}) => {
  const { t } = useTranslation('common');
  const [parentPath, setParentPath] = useState<string>(defaultParentPath || '');
  const [projectName, setProjectName] = useState<string>('');
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string>('');

  // Combine parent path and project name
  const fullPath = useMemo(() => {
    if (!parentPath || !projectName.trim()) return '';
    const normalizedPath = parentPath.replace(/\\/g, '/');
    return `${normalizedPath}/${projectName.trim()}`;
  }, [parentPath, projectName]);

  // Open directory picker dialog
  const handleSelectParentPath = useCallback(async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('newProject.selectParentDirectory'),
        defaultPath: parentPath || defaultParentPath
      }) as string;

      if (selected && typeof selected === 'string') {
        setParentPath(selected);
        setError('');
      }
    } catch (error) {
      log.error('Failed to select directory', error);
    }
  }, [parentPath, defaultParentPath, t]);

  // Validate and create new project
  const handleConfirm = useCallback(async () => {
    // Validate form fields
    if (!parentPath || !parentPath.trim()) {
      setError(t('newProject.errorSelectParent'));
      return;
    }
    if (!projectName || !projectName.trim()) {
      setError(t('newProject.errorEnterName'));
      return;
    }

    setIsCreating(true);
    setError('');

    try {
      await onConfirm(parentPath, projectName.trim());
      setParentPath('');
      setProjectName('');
      onClose();
    } catch (error) {
      log.error('Failed to create project', error);
      setError(error instanceof Error ? error.message : t('newProject.errorCreateFailed'));
    } finally {
      setIsCreating(false);
    }
  }, [parentPath, projectName, onConfirm, onClose, t]);

  // Reset form and close dialog
  const handleCancel = useCallback(() => {
    setParentPath('');
    setProjectName('');
    setError('');
    onClose();
  }, [onClose]);

  // Update project name and clear errors
  const handleProjectNameChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setProjectName(e.target.value);
    if (error) setError('');
  }, [error]);

  return (
    <Modal
      isOpen={isOpen}
      onClose={handleCancel}
      title=""
      size="small"
      showCloseButton={true}
    >
      <div className="new-project-dialog">
        {/* Hero section */}
        <div className="new-project-dialog__hero">
          <div className="new-project-dialog__icon-wrapper">
            <FolderPlus size={24} />
          </div>
          <h2 className="new-project-dialog__title">{t('newProject.title')}</h2>
          <p className="new-project-dialog__subtitle">{t('newProject.subtitle')}</p>
        </div>

        {/* Form content */}
        <div className="new-project-dialog__content">
          {/* Parent directory */}
          <div className="new-project-dialog__field">
            <label className="new-project-dialog__label">
              <FolderOpen size={14} />
              {t('newProject.parentDirectory')}
            </label>
            <div className="new-project-dialog__path-selector">
              <div className="new-project-dialog__path-input">
                <Input
                  type="text"
                  value={parentPath}
                  readOnly
                  placeholder={t('newProject.parentDirectoryPlaceholder')}
                />
              </div>
              <Button
                type="button"
                className="new-project-dialog__select-btn"
                variant="secondary"
                size="small"
                onClick={handleSelectParentPath}
              >
                <FolderOpen size={14} />
                <span>{t('newProject.select')}</span>
              </Button>
            </div>
          </div>

          {/* Project name */}
          <div className="new-project-dialog__field">
            <label className="new-project-dialog__label">
              <FileText size={14} />
              {t('newProject.projectName')}
            </label>
            <div className="new-project-dialog__name-input">
              <Input
                type="text"
                value={projectName}
                onChange={handleProjectNameChange}
                placeholder={t('newProject.projectNamePlaceholder')}
                disabled={isCreating}
                autoFocus
              />
            </div>
          </div>

          {/* Full path display */}
          {fullPath && (
            <div className="new-project-dialog__preview">
              <div className="new-project-dialog__preview-icon">
                <FolderTree size={14} />
              </div>
              <div className="new-project-dialog__preview-content">
                <span className="new-project-dialog__preview-label">{t('newProject.fullPath')}</span>
                <span className="new-project-dialog__preview-path">{fullPath}</span>
              </div>
            </div>
          )}

          {/* Error message */}
          {error && (
            <div className="new-project-dialog__error">
              <AlertCircle size={14} />
              <span>{error}</span>
            </div>
          )}
        </div>

        {/* Footer buttons */}
        <div className="new-project-dialog__footer">
          <Button
            type="button"
            className="new-project-dialog__btn new-project-dialog__btn--cancel"
            variant="ghost"
            size="small"
            onClick={handleCancel}
            disabled={isCreating}
          >
            <X size={14} />
            {t('newProject.cancel')}
          </Button>
          <Button
            type="button"
            className="new-project-dialog__btn new-project-dialog__btn--confirm"
            variant="primary"
            size="small"
            onClick={handleConfirm}
            disabled={isCreating}
            isLoading={isCreating}
          >
            {isCreating ? (
              t('newProject.creating')
            ) : (
              <>
                <Check size={14} />
                {t('newProject.create')}
              </>
            )}
          </Button>
        </div>
      </div>
    </Modal>
  );
};
