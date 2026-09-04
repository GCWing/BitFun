/**
 * Dialog when a remote update is available (daily prompt or manual check).
 */

import {
  Button,
  Icon,
  ScrollArea,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@openbitfun/ui';
import React, { useRef } from 'react';
;
import { useI18n } from '@/infrastructure/i18n';
import type { CheckForUpdatesResponse } from '@/infrastructure/api/service-api/SystemAPI';
import './UpdateAvailableDialog.scss';

export interface UpdateAvailableDialogProps {
  isOpen: boolean;
  variant: 'daily' | 'manual';
  data: CheckForUpdatesResponse | null;
  onLater: () => void;
  onSkip?: () => void;
  onInstall: () => void;
}

export const UpdateAvailableDialog: React.FC<UpdateAvailableDialogProps> = ({
  isOpen,
  variant,
  data,
  onLater,
  onSkip,
  onInstall
}) => {
  const { t } = useI18n('common');
  const lastAvailableDataRef = useRef<CheckForUpdatesResponse | null>(null);
  if (data?.updateAvailable) {
    lastAvailableDataRef.current = data;
  }
  const displayData = data?.updateAvailable ? data : lastAvailableDataRef.current;
  const modalOpen = isOpen && data?.updateAvailable === true;

  if (!displayData) {
    return null;
  }

  const latest = displayData.latestVersion ?? '';
  const notes = displayData.releaseNotes?.trim();

  return (
    <Dialog
      open={modalOpen}
      onOpenChange={(nextOpen) => { if (!nextOpen) onLater(); }}
      size="md"
    >
      <DialogHeader>
        <DialogHeading>
          <DialogTitle>{t('update.availableTitle')}</DialogTitle>
        </DialogHeading>
        <DialogClose />
      </DialogHeader>
      <DialogBody>
      <div
        className="openbitfun-update-available"
        data-openbitfun-component="update"
        data-openbitfun-part="availableRoot"
        data-openbitfun-variant={variant}
      >
        <div
          className="openbitfun-update-available__lead"
          data-openbitfun-component="update"
          data-openbitfun-part="lead"
        >
          <div
            className="openbitfun-update-available__lead-icon"
            aria-hidden
            data-openbitfun-component="update"
            data-openbitfun-part="leadIcon"
          >
            <Icon name="arrow-down" size="lg" />
          </div>
          <p
            className="openbitfun-update-available__subtitle"
            data-openbitfun-component="update"
            data-openbitfun-part="subtitle"
          >{t('update.availableSubtitle')}</p>
        </div>

        <div
          className="openbitfun-update-available__versions openbitfun-update-available__versions--card"
          data-openbitfun-component="update"
          data-openbitfun-part="versions"
        >
          <div
            className="openbitfun-update-available__row"
            data-openbitfun-component="update"
            data-openbitfun-part="versionRow"
          >
            <span className="openbitfun-update-available__label" data-openbitfun-component="update" data-openbitfun-part="versionLabel">{t('update.currentVersion')}</span>
            <span className="openbitfun-update-available__value" data-openbitfun-component="update" data-openbitfun-part="versionValue">{displayData.currentVersion}</span>
          </div>
          <div
            className="openbitfun-update-available__row openbitfun-update-available__row--highlight"
            data-openbitfun-component="update"
            data-openbitfun-part="versionRow"
            data-openbitfun-state="highlight"
          >
            <span className="openbitfun-update-available__label" data-openbitfun-component="update" data-openbitfun-part="versionLabel">{t('update.latestVersion')}</span>
            <span className="openbitfun-update-available__value" data-openbitfun-component="update" data-openbitfun-part="versionValue">{latest}</span>
          </div>
        </div>

        {notes ? (
          <div className="openbitfun-update-available__notes" data-openbitfun-component="update" data-openbitfun-part="notes">
            <div className="openbitfun-update-available__notes-label" data-openbitfun-component="update" data-openbitfun-part="notesLabel">{t('update.releaseNotes')}</div>
            <ScrollArea className="openbitfun-update-available__notes-body" data-openbitfun-component="update" data-openbitfun-part="notesBody">
              <pre>{notes}</pre>
            </ScrollArea>
          </div>
        ) : null}

        <div className="openbitfun-update-available__actions" data-openbitfun-component="update" data-openbitfun-part="actions">
          {variant === 'daily' ? (
            <>
              <Button variant="outline" size="md" onClick={onLater}>
                {t('update.later')}
              </Button>
              {onSkip ? (
                <Button variant="outline" size="md" onClick={onSkip}>
                  {t('update.skipVersion')}
                </Button>
              ) : null}
              <Button variant="fill" size="md" onClick={onInstall}>
                {t('update.backgroundInstall')}
              </Button>
            </>
          ) : (
            <>
              <Button variant="outline" size="md" onClick={onLater}>
                {t('update.cancel')}
              </Button>
              <Button variant="fill" size="md" onClick={onInstall}>
                {t('update.backgroundInstall')}
              </Button>
            </>
          )}
        </div>
      </div>
          </DialogBody>
    </Dialog>
  );
};
