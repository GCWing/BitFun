/**
 * New Project Dialog Component
 */

import {
  Button,
  Icon,
  Input,
  Field,
  Dialog,
  DialogBody,
  DialogClose,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@openbitfun/ui';
import React, { useState, useCallback, useMemo, useId } from 'react';
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

  const formId = useId();

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
    if (isCreating) return;
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
  }, [parentPath, projectName, onConfirm, onClose, t, isCreating]);

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
      onOpenChange={(nextOpen) => { if (!nextOpen && !isCreating) handleCancel(); }}
      size="sm"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle data-openbitfun-component="new-project-dialog" data-openbitfun-part="title">
            {t('newProject.title')}
          </DialogTitle>
          <DialogDescription>{t('newProject.subtitle')}</DialogDescription>
        </DialogHeading>
        <DialogClose disabled={isCreating} />
      </DialogHeader>
      <DialogBody>
        <form
          id={formId}
          data-openbitfun-component="new-project-dialog"
          data-openbitfun-part="root"
          className="new-project-dialog"
          aria-busy={isCreating}
          onSubmit={(event) => {
            event.preventDefault();
            void handleConfirm();
          }}
        >
          <div data-openbitfun-component="new-project-dialog" data-openbitfun-part="content" className="new-project-dialog__content">
            <div data-openbitfun-component="new-project-dialog" data-openbitfun-part="field" className="new-project-dialog__field">
              <div data-openbitfun-component="new-project-dialog" data-openbitfun-part="pathSelector">
                <Field
                  label={t('newProject.parentDirectory')}
                  controlWidth="fill"
                  controlTrailing={
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      leadingIcon={<Icon name="folder" size="sm" />}
                      onClick={handleSelectParentPath}
                      disabled={isCreating}
                    >
                      {t('newProject.select')}
                    </Button>
                  }
                >
                  <Input
                    size="sm"
                    type="text"
                    value={parentPath}
                    title={parentPath}
                    readOnly
                    disabled={isCreating}
                    placeholder={t('newProject.parentDirectoryPlaceholder')}
                  />
                </Field>
              </div>
            </div>
            <div data-openbitfun-component="new-project-dialog" data-openbitfun-part="field" className="new-project-dialog__field">
              <Field label={t('newProject.projectName')} controlWidth="fill">
                <Input
                  size="sm"
                  type="text"
                  value={projectName}
                  onChange={handleProjectNameChange}
                  placeholder={t('newProject.projectNamePlaceholder')}
                  disabled={isCreating}
                  autoFocus
                />
              </Field>
            </div>
            {fullPath && (
              <div data-openbitfun-component="new-project-dialog" data-openbitfun-part="preview" className="new-project-dialog__preview">
                <span className="new-project-dialog__preview-label">{t('newProject.fullPath')}</span>
                <span className="new-project-dialog__preview-path">{fullPath}</span>
              </div>
            )}
          </div>
          {error && (
            <div role="alert" data-openbitfun-component="new-project-dialog" data-openbitfun-part="error" className="new-project-dialog__error">
              <Icon name="info" size="sm" />
              <span>{error}</span>
            </div>
          )}
        </form>
      </DialogBody>
      <DialogFooter data-openbitfun-component="new-project-dialog" data-openbitfun-part="footer">
        <Button type="button" variant="outline" size="sm" onClick={handleCancel} disabled={isCreating}>
          {t('newProject.cancel')}
        </Button>
        <Button type="submit" form={formId} variant="fill" size="sm" disabled={isCreating} loading={isCreating}>
          {isCreating ? t('newProject.creating') : t('newProject.create')}
        </Button>
      </DialogFooter>
    </Dialog>
  );
};
