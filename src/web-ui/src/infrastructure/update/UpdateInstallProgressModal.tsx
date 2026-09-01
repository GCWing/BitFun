/**
 * Full-screen style modal showing download progress for in-app updates.
 */

import {
  Alert,
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@bitfun/ui';
import React, { useMemo } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import type { UpdateDownloadProgressPayload } from './installUpdateWithProgress';
import { formatUpdateInstallError } from './updateErrorMessage';
import './UpdateInstallProgressModal.scss';

export interface UpdateInstallProgressModalProps {
  isOpen: boolean;
  error: string | null;
  installed?: boolean;
  progress: UpdateDownloadProgressPayload;
  onCloseError?: () => void;
  onCloseInstalled?: () => void;
  onRestart?: () => void;
}

export const UpdateInstallProgressModal: React.FC<UpdateInstallProgressModalProps> = ({
  isOpen,
  error,
  installed,
  progress,
  onCloseError,
  onCloseInstalled,
  onRestart
}) => {
  const { t } = useI18n('common');
  const { downloaded, total } = progress;
  const pct =
    total != null && total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null;

  const errorMessage = useMemo(
    () => (error ? formatUpdateInstallError(error, t) : null),
    [error, t]
  );
  let title = t('update.downloadingTitle');
  if (error) {
    title = t('update.downloadFailedTitle');
  } else if (installed) {
    title = t('update.installedTitle');
  }

  let onClose = () => {};
  if (error) {
    onClose = onCloseError ?? (() => {});
  } else if (installed) {
    onClose = onCloseInstalled ?? (() => {});
  }

  let body: React.ReactNode = null;
  if (errorMessage) {
    body = (
      <div data-bf-component="update" data-bf-part="alert">
        <Alert
          tone="error"
          message={errorMessage}
          showIcon
          className="bitfun-update-progress__alert"
        />
      </div>
    );
  } else if (installed) {
    body = (
      <>
        <div data-bf-component="update" data-bf-part="alert">
          <Alert
            tone="success"
            message={t('update.installedMessage')}
            showIcon
            className="bitfun-update-progress__alert"
          />
        </div>
        <div className="bitfun-update-progress__actions" data-bf-component="update" data-bf-part="actions">
          <Button variant="outline" size="md" onClick={onCloseInstalled}>
            {t('update.restartLater')}
          </Button>
          <Button variant="fill" size="md" onClick={onRestart}>
            {t('update.restartNow')}
          </Button>
        </div>
      </>
    );
  } else {
    body = (
      <>
        <div
          className="bitfun-update-progress__bar"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={pct ?? undefined}
          aria-label={t('update.downloadingTitle')}
          data-bf-component="update"
          data-bf-part="progressBar"
        >
          <div
            className={
              pct != null
                ? 'bitfun-update-progress__fill'
                : 'bitfun-update-progress__fill bitfun-update-progress__fill--indeterminate'
            }
            style={pct != null ? { transform: `scaleX(${pct / 100})` } : undefined}
            data-bf-component="update"
            data-bf-part="progressFill"
            data-bf-state={pct == null ? 'indeterminate' : undefined}
          />
        </div>
        <p className="bitfun-update-progress__hint" data-bf-component="update" data-bf-part="progressHint">
          {pct != null
            ? t('update.progressPercent', { percent: String(pct) })
            : t('update.progressUnknown')}
        </p>
        <p className="bitfun-update-progress__restart" data-bf-component="update" data-bf-part="restartHint">{t('update.restartHint')}</p>
      </>
    );
  }

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) onClose(); }}
      size="sm"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeading>
        {!!error || !!installed && <DialogClose />}
      </DialogHeader>
      <DialogBody inset="none">
      <div
        className="bitfun-update-progress"
        data-bf-component="update"
        data-bf-part="progressRoot"
        data-bf-status={error ? 'error' : installed ? 'installed' : 'downloading'}
      >
        {body}
      </div>
          </DialogBody>
    </Dialog>
  );
};
