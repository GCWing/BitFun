/**
 * New Project Dialog Component
 */

import {
  Button,
  Icon,
  Input,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@bitfun/ui';
import React, { useState, useCallback, useMemo } from 'react';
import {
  FolderPlus,
  FolderOpen,
  FileText,
  FolderTree,
  AlertCircle,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { createLogger } from '@/shared/utils/logger';
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
      const { pickWorkspaceDirectory } = await import(
        '@/infrastructure/peer-device/pickWorkspaceDirectory'
      );
      const selected = await pickWorkspaceDirectory({
        title: t('newProject.selectParentDirectory'),
        defaultPath: parentPath || defaultParentPath,
      });

      if (selected) {
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
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) handleCancel(); }}
      size="sm"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{""}</DialogTitle>
        </DialogHeading>
        <DialogClose />
      </DialogHeader>
      <DialogBody inset="none">
      <div data-bf-component="new-project-dialog" data-bf-part="root" className="new-project-dialog">
        {/* Hero section */}
        <div data-bf-component="new-project-dialog" data-bf-part="hero" className="new-project-dialog__hero">
          <div className="new-project-dialog__icon-wrapper">
            <FolderPlus size={24} />
          </div>
          <h2 data-bf-component="new-project-dialog" data-bf-part="title" className="new-project-dialog__title">{t('newProject.title')}</h2>
          <p className="new-project-dialog__subtitle">{t('newProject.subtitle')}</p>
        </div>

        {/* Form content */}
        <div data-bf-component="new-project-dialog" data-bf-part="content" className="new-project-dialog__content">
          {/* Parent directory */}
          <div data-bf-component="new-project-dialog" data-bf-part="field" className="new-project-dialog__field">
            <label className="new-project-dialog__label">
              <FolderOpen size={14} />
              {t('newProject.parentDirectory')}
            </label>
            <div data-bf-component="new-project-dialog" data-bf-part="pathSelector" className="new-project-dialog__path-selector">
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
                variant="outline"
                size="sm"
                leadingIcon={<FolderOpen size={14} />}
                onClick={handleSelectParentPath}
              >
                {t('newProject.select')}
              </Button>
            </div>
          </div>

          {/* Project name */}
          <div data-bf-component="new-project-dialog" data-bf-part="field" className="new-project-dialog__field">
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
            <div data-bf-component="new-project-dialog" data-bf-part="preview" className="new-project-dialog__preview">
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
            <div data-bf-component="new-project-dialog" data-bf-part="error" className="new-project-dialog__error">
              <AlertCircle size={14} />
              <span>{error}</span>
            </div>
          )}
        </div>

        {/* Footer buttons */}
        <div data-bf-component="new-project-dialog" data-bf-part="footer" className="new-project-dialog__footer">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleCancel}
            disabled={isCreating}
            leadingIcon={<Icon name="xmark" size="sm" />}
          >

            {t('newProject.cancel')}
          </Button>
          <Button
            type="button"
            variant="fill"
            size="sm"
            onClick={handleConfirm}
            disabled={isCreating}
            loading={isCreating}
          >
            {isCreating ? (
              t('newProject.creating')
            ) : (
              <>
                <Icon name="check-line" size="sm" />
                {t('newProject.create')}
              </>
            )}
          </Button>
        </div>
      </div>
          </DialogBody>
    </Dialog>
  );
};
