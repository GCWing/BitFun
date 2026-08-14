/**
 * Dialog when a remote update is available (daily prompt or manual check).
 */

import React, { useRef } from 'react';
import { Modal, Button } from '@/component-library';
import { Download } from 'lucide-react';
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
    <Modal
      isOpen={modalOpen}
      onClose={onLater}
      title={t('update.availableTitle')}
      showCloseButton={true}
      size="medium"
      contentInset
    >
      <div
        className="bitfun-update-available"
        data-bf-component="update"
        data-bf-part="availableRoot"
        data-bf-variant={variant}
      >
        <div
          className="bitfun-update-available__lead"
          data-bf-component="update"
          data-bf-part="lead"
        >
          <div
            className="bitfun-update-available__lead-icon"
            aria-hidden
            data-bf-component="update"
            data-bf-part="leadIcon"
          >
            <Download size={18} strokeWidth={2} />
          </div>
          <p
            className="bitfun-update-available__subtitle"
            data-bf-component="update"
            data-bf-part="subtitle"
          >{t('update.availableSubtitle')}</p>
        </div>

        <div
          className="bitfun-update-available__versions bitfun-update-available__versions--card"
          data-bf-component="update"
          data-bf-part="versions"
        >
          <div
            className="bitfun-update-available__row"
            data-bf-component="update"
            data-bf-part="versionRow"
          >
            <span className="bitfun-update-available__label" data-bf-component="update" data-bf-part="versionLabel">{t('update.currentVersion')}</span>
            <span className="bitfun-update-available__value" data-bf-component="update" data-bf-part="versionValue">{displayData.currentVersion}</span>
          </div>
          <div
            className="bitfun-update-available__row bitfun-update-available__row--highlight"
            data-bf-component="update"
            data-bf-part="versionRow"
            data-bf-state="highlight"
          >
            <span className="bitfun-update-available__label" data-bf-component="update" data-bf-part="versionLabel">{t('update.latestVersion')}</span>
            <span className="bitfun-update-available__value" data-bf-component="update" data-bf-part="versionValue">{latest}</span>
          </div>
        </div>

        {notes ? (
          <div className="bitfun-update-available__notes" data-bf-component="update" data-bf-part="notes">
            <div className="bitfun-update-available__notes-label" data-bf-component="update" data-bf-part="notesLabel">{t('update.releaseNotes')}</div>
            <pre className="bitfun-update-available__notes-body" data-bf-component="update" data-bf-part="notesBody">{notes}</pre>
          </div>
        ) : null}

        <div className="bitfun-update-available__actions" data-bf-component="update" data-bf-part="actions">
          {variant === 'daily' ? (
            <>
              <Button variant="secondary" size="medium" onClick={onLater}>
                {t('update.later')}
              </Button>
              {onSkip ? (
                <Button variant="ghost" size="medium" onClick={onSkip}>
                  {t('update.skipVersion')}
                </Button>
              ) : null}
              <Button variant="primary" size="medium" onClick={onInstall}>
                {t('update.backgroundInstall')}
              </Button>
            </>
          ) : (
            <>
              <Button variant="secondary" size="medium" onClick={onLater}>
                {t('update.cancel')}
              </Button>
              <Button variant="primary" size="medium" onClick={onInstall}>
                {t('update.backgroundInstall')}
              </Button>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
};
