import React, { useCallback, useEffect, useState } from 'react';
import { Alert, Button, Checkbox, ConfirmDialog, Modal } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import { PrivacyDocument } from './PrivacyDocument';
import { usePrivacy } from './PrivacyContext';

const log = createLogger('PrivacyStatementDialog');

interface PrivacyStatementDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onModeChangeComplete?: () => void;
  variant?: 'about' | 'readonly';
}

type OperationError = 'accept_save' | 'apply' | 'withdraw' | 'mark_viewed' | null;

export const PrivacyStatementDialog: React.FC<PrivacyStatementDialogProps> = ({
  isOpen,
  onClose,
  onModeChangeComplete,
  variant = 'about',
}) => {
  const { t, currentLanguage, formatDate } = useI18n('common');
  const {
    status,
    refresh,
    accept,
    enterNotAccepted,
    markViewed,
    applyCollectionPolicy,
  } = usePrivacy();
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [confirmWithdraw, setConfirmWithdraw] = useState(false);
  const [operationError, setOperationError] = useState<OperationError>(null);
  const [openedWithUpdate, setOpenedWithUpdate] = useState(false);

  useEffect(() => {
    if (!isOpen) {
      setChecked(false);
      setBusy(false);
      setConfirmWithdraw(false);
      setOperationError(null);
      setOpenedWithUpdate(false);
      return;
    }
    if (!status?.enabled) return;

    void refresh(currentLanguage)
      .then(async next => {
        setOpenedWithUpdate(next.hasUnreadUpdate);
        if (next.hasUnreadUpdate && next.policy) {
          try {
            await markViewed(next.policy.updatedAt, currentLanguage);
          } catch (error) {
            log.warn('Privacy policy viewed state could not be saved', error);
            setOperationError('mark_viewed');
          }
        }
      })
      .catch(error => {
        log.warn('Privacy status could not be refreshed', error);
      });
  }, [currentLanguage, isOpen, markViewed, refresh, status?.enabled]);

  const policy = status?.policy;
  const fullMode = status?.lifecycleState === 'full' && status.effectiveMode === 'full';
  const fullModeNeedsRetry =
    status?.lifecycleState === 'full' && status.effectiveMode === 'privacy_not_accepted';

  const close = useCallback(() => {
    if (!busy) onClose();
  }, [busy, onClose]);

  const handleAccept = async () => {
    if (!policy || !checked || busy) return;
    setBusy(true);
    setOperationError(null);
    try {
      await accept({
        policyUpdatedAt: policy.updatedAt,
        consentVersion: policy.consentVersion,
        documentSha256: policy.documentSha256,
        locale: policy.locale,
      });
      setChecked(false);
      onModeChangeComplete?.();
    } catch (error) {
      log.warn('Privacy consent could not be saved or applied', error);
      try {
        const next = await refresh(currentLanguage);
        setOperationError(next.lifecycleState === 'full' ? 'apply' : 'accept_save');
      } catch {
        setOperationError('accept_save');
      }
    } finally {
      setBusy(false);
    }
  };

  const handleApplyRetry = async () => {
    if (busy) return;
    setBusy(true);
    setOperationError(null);
    try {
      await applyCollectionPolicy('full', currentLanguage);
      onModeChangeComplete?.();
    } catch (error) {
      log.warn('Full privacy mode could not be applied', error);
      setOperationError('apply');
    } finally {
      setBusy(false);
    }
  };

  const handleWithdraw = async () => {
    if (busy) return;
    setConfirmWithdraw(false);
    setBusy(true);
    setOperationError(null);
    try {
      await enterNotAccepted(policy?.locale ?? currentLanguage);
      onModeChangeComplete?.();
    } catch (error) {
      log.warn('Privacy withdrawal state could not be saved', error);
      setOperationError('withdraw');
    } finally {
      setBusy(false);
    }
  };

  if (!policy) return null;

  const errorMessage =
    operationError === 'accept_save'
      ? t('privacy.acceptSaveFailed')
      : operationError === 'apply'
        ? t('privacy.applyFailed')
        : operationError === 'withdraw'
          ? t('privacy.withdrawFailed')
          : operationError === 'mark_viewed'
            ? t('privacy.markViewedFailed')
            : null;

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={close}
        title={t('privacy.title')}
        size="xlarge"
        contentClassName="bitfun-privacy-dialog"
        showCloseButton={!busy}
      >
        <div className="bitfun-privacy-dialog__metadata">
          <span>
            {t('privacy.effectiveAt', {
              date: formatDate(new Date(policy.effectiveAt), { dateStyle: 'long' }),
            })}
          </span>
          <span>
            {t('privacy.updatedAt', {
              date: formatDate(new Date(policy.updatedAt), { dateStyle: 'long' }),
            })}
          </span>
          {openedWithUpdate && policy.changeType === 'editorial' ? (
            <strong>{t('privacy.editorialChange')}</strong>
          ) : null}
        </div>
        {variant === 'about' ? (
          <div className="bitfun-privacy-dialog__mode" role="status">
            <strong>{t(fullMode ? 'privacy.fullMode' : 'privacy.notAcceptedMode')}</strong>
            <span>
              {t(
                fullMode
                  ? 'privacy.fullModeDescription'
                  : 'privacy.notAcceptedModeDescription',
              )}
            </span>
          </div>
        ) : null}
        <div className="bitfun-privacy-dialog__document">
          <PrivacyDocument content={policy.content} />
        </div>
        {errorMessage ? (
          <div className="bitfun-privacy-dialog__error">
            <Alert type="error" message={errorMessage} showIcon />
          </div>
        ) : null}
        {variant === 'about' ? (
          <div className="bitfun-privacy-dialog__actions">
            {fullMode ? (
              <Button
                variant="danger"
                disabled={busy}
                onClick={() => setConfirmWithdraw(true)}
              >
                {t('privacy.withdraw')}
              </Button>
            ) : fullModeNeedsRetry ? (
              <Button isLoading={busy} onClick={() => void handleApplyRetry()}>
                {t('privacy.retryFullMode')}
              </Button>
            ) : (
              <div className="bitfun-privacy-dialog__consent">
                <Checkbox
                  checked={checked}
                  disabled={busy}
                  onChange={event => setChecked(event.target.checked)}
                  label={t('privacy.consentCheckbox')}
                />
                <Button
                  disabled={!checked}
                  isLoading={busy}
                  onClick={() => void handleAccept()}
                >
                  {t('privacy.enableFull')}
                </Button>
              </div>
            )}
          </div>
        ) : null}
      </Modal>
      <ConfirmDialog
        isOpen={confirmWithdraw}
        onClose={() => setConfirmWithdraw(false)}
        onConfirm={() => void handleWithdraw()}
        title={t('privacy.withdrawConfirmTitle')}
        message={t('privacy.withdrawConfirmMessage')}
        confirmText={t('privacy.withdrawConfirmAction')}
        cancelText={t('privacy.cancel')}
        confirmDanger
      />
    </>
  );
};
