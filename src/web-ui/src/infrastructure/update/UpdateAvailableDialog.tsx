/**
 * Dialog when a remote update is available (daily prompt or manual check).
 */

import React from 'react';
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
  if (!isOpen || !data?.updateAvailable) {
    return null;
  }

  const latest = data.latestVersion ?? '';
  const notes = data.releaseNotes?.trim();

  return (
    <Modal
      isOpen={isOpen}
      onClose={onLater}
      title={t('update.availableTitle')}
      showCloseButton={true}
      size="medium"
      contentInset
    >
      <div className="bitfun-update-available">
        <div className="bitfun-update-available__lead">
          <div className="bitfun-update-available__lead-icon" aria-hidden>
            <Download size={18} strokeWidth={2} />
          </div>
          <p className="bitfun-update-available__subtitle">{t('update.availableSubtitle')}</p>
        </div>

        <div className="bitfun-update-available__versions bitfun-update-available__versions--card">
          <div className="bitfun-update-available__row">
            <span className="bitfun-update-available__label">{t('update.currentVersion')}</span>
            <span className="bitfun-update-available__value">{data.currentVersion}</span>
          </div>
          <div className="bitfun-update-available__row bitfun-update-available__row--highlight">
            <span className="bitfun-update-available__label">{t('update.latestVersion')}</span>
            <span className="bitfun-update-available__value">{latest}</span>
          </div>
        </div>

        {notes ? (
          <div className="bitfun-update-available__notes">
            <div className="bitfun-update-available__notes-label">{t('update.releaseNotes')}</div>
            <pre className="bitfun-update-available__notes-body">{notes}</pre>
          </div>
        ) : null}

        <div className="bitfun-update-available__actions">
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
                {t('update.install')}
              </Button>
            </>
          ) : (
            <>
              <Button variant="secondary" size="medium" onClick={onLater}>
                {t('update.cancel')}
              </Button>
              <Button variant="primary" size="medium" onClick={onInstall}>
                {t('update.install')}
              </Button>
            </>
          )}
        </div>
      </div>
    </Modal>
  );
};
