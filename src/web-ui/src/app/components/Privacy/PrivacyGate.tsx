import React, { useCallback, useEffect, useState } from 'react';
import { AlertTriangle, LoaderCircle } from 'lucide-react';
import { Button, Checkbox, Modal } from '@/component-library';
import { hideStartupOverlay } from '@/app/startup/startupOverlay';
import { privacyAPI } from '@/infrastructure/api/service-api/PrivacyAPI';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { createLogger } from '@/shared/utils/logger';
import { PrivacyDocument } from './PrivacyDocument';
import { usePrivacy } from './PrivacyContext';
import copyByLocale from './privacyGateCopy.json';
import './Privacy.scss';

const log = createLogger('PrivacyGate');
type PrivacyLocale = keyof typeof copyByLocale;

function detectedLocale(): PrivacyLocale {
  const locale = navigator.language.toLowerCase();
  if (locale.includes('hant') || locale.startsWith('zh-tw') || locale.startsWith('zh-hk')) {
    return 'zh-TW';
  }
  if (locale.startsWith('zh')) return 'zh-CN';
  return 'en-US';
}

export const PrivacyGate: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const {
    status,
    initialize,
    refresh,
    accept,
    enterNotAccepted,
    applyCollectionPolicy,
  } = usePrivacy();
  const [dismissed, setDismissed] = useState(false);
  const [checked, setChecked] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [loadError, setLoadError] = useState(false);
  const [mutationError, setMutationError] = useState(false);
  const [applyRetryRequired, setApplyRetryRequired] = useState(false);
  const locale = detectedLocale();
  const copy = copyByLocale[locale];

  const reveal = useCallback(async () => {
    await hideStartupOverlay();
    if (!isTauriRuntime()) return;
    try {
      await privacyAPI.showGateWindow();
    } catch (error) {
      log.warn('Failed to reveal privacy window', error);
    }
  }, []);

  const loadStatus = useCallback(async () => {
    setLoadError(false);
    if (!isTauriRuntime()) return;
    try {
      const next = await initialize();
      if (next.lifecycleState === 'choice_required' || next.lifecycleState === 'resource_error') {
        await reveal();
      }
    } catch (error) {
      log.error('Privacy initialization failed', error);
      setLoadError(true);
      await reveal();
    }
  }, [initialize, reveal]);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const needsChoice = status?.enabled && status.lifecycleState === 'choice_required';
  const resourceError = loadError || status?.lifecycleState === 'resource_error';
  const choicePanelVisible = needsChoice || applyRetryRequired;
  const overlayVisible = !dismissed && (choicePanelVisible || resourceError);

  const dismiss = useCallback(() => {
    if (!submitting) setDismissed(true);
  }, [submitting]);

  useEffect(() => {
    if (!overlayVisible) return;
    const handleBack = () => dismiss();
    window.addEventListener('popstate', handleBack);
    return () => {
      window.removeEventListener('popstate', handleBack);
    };
  }, [dismiss, overlayVisible]);

  const handleAccept = async () => {
    const policy = status?.policy;
    if (!checked || !policy || !status.releaseReady || submitting) return;
    setSubmitting(true);
    setMutationError(false);
    try {
      await accept({
        policyUpdatedAt: policy.updatedAt,
        consentVersion: policy.consentVersion,
        documentSha256: policy.documentSha256,
        locale: policy.locale,
      });
      setDismissed(true);
    } catch (error) {
      log.error('Privacy consent could not be saved or applied', error);
      setMutationError(true);
      try {
        const next = await refresh(locale);
        setApplyRetryRequired(
          next.lifecycleState === 'full' && next.effectiveMode === 'privacy_not_accepted',
        );
      } catch {
        setApplyRetryRequired(false);
      }
    } finally {
      setSubmitting(false);
    }
  };

  const handleApplyRetry = async () => {
    if (submitting) return;
    setSubmitting(true);
    setMutationError(false);
    try {
      await applyCollectionPolicy('full', locale);
      setApplyRetryRequired(false);
      setDismissed(true);
    } catch (error) {
      log.error('Full privacy mode could not be applied', error);
      setMutationError(true);
    } finally {
      setSubmitting(false);
    }
  };

  const handleNotAccepted = async () => {
    if (submitting) return;
    setSubmitting(true);
    setMutationError(false);
    try {
      await enterNotAccepted(status?.policy?.locale ?? locale);
      setDismissed(true);
    } catch (error) {
      log.error('Privacy not-accepted state could not be saved', error);
      setMutationError(true);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <>
      {children}
      <Modal
        isOpen={overlayVisible && resourceError}
        onClose={dismiss}
        title={copy.loadError}
        size="medium"
        contentClassName="bitfun-privacy-resource-error"
        showCloseButton={!submitting}
        closeOnOverlayClick={!submitting}
        testId="privacy-resource-error"
      >
        <AlertTriangle size={28} aria-hidden />
        <p>{copy.resourceErrorHint}</p>
        <div className="bitfun-privacy-gate__actions">
          <Button variant="secondary" disabled={submitting} onClick={dismiss}>
            {copy.closeAndContinue}
          </Button>
          <Button disabled={submitting} onClick={() => void loadStatus()}>{copy.retry}</Button>
        </div>
      </Modal>
      <Modal
        isOpen={Boolean(
          overlayVisible && choicePanelVisible && !resourceError && status?.policy,
        )}
        onClose={dismiss}
        title={copy.title}
        size="xlarge"
        contentClassName="bitfun-privacy-dialog bitfun-privacy-consent-dialog"
        showCloseButton={!submitting}
        closeOnOverlayClick={!submitting}
        testId="privacy-consent-gate"
      >
        <div data-bf-component="privacy-gate" data-bf-part="root">
        {status?.policy ? (
          <>
            <div className="bitfun-privacy-dialog__metadata" data-bf-component="privacy-gate" data-bf-part="metadata">
              <span>{copy.effective}: {status.policy.effectiveAt.slice(0, 10)}</span>
              <span>{copy.updated}: {status.policy.updatedAt.slice(0, 10)}</span>
            </div>
            <div className="bitfun-privacy-consent-dialog__intro">{copy.intro}</div>
            <div className="bitfun-privacy-dialog__document" aria-label={copy.title} data-bf-component="privacy-gate" data-bf-part="document">
              <PrivacyDocument content={status.policy.content} />
            </div>
            <div className="bitfun-privacy-consent-dialog__footer" data-bf-component="privacy-gate" data-bf-part="footer">
              {!status.releaseReady && (
                <div className="bitfun-privacy-gate__configuration-error">{copy.releaseBlocked}</div>
              )}
              {mutationError && (
                <div className="bitfun-privacy-gate__configuration-error" role="alert">
                  {copy.saveFailed}
                </div>
              )}
              <div className="bitfun-privacy-gate__consent-row">
                {!applyRetryRequired ? (
                  <Checkbox
                    checked={checked}
                    disabled={submitting}
                    onChange={event => setChecked(event.target.checked)}
                    label={copy.checkbox}
                    data-testid="privacy-consent-checkbox"
                  />
                ) : <span />}
                <div className="bitfun-privacy-gate__actions" data-bf-component="privacy-gate" data-bf-part="actions">
                  <Button variant="secondary" disabled={submitting} onClick={() => void handleNotAccepted()}>
                    {copy.disagree}
                  </Button>
                  <Button
                    disabled={!applyRetryRequired && (!checked || !status.releaseReady)}
                    isLoading={submitting}
                    onClick={() => void (applyRetryRequired ? handleApplyRetry() : handleAccept())}
                    data-testid="privacy-accept"
                  >
                    {applyRetryRequired ? copy.retryFullMode : copy.agree}
                  </Button>
                </div>
              </div>
            </div>
          </>
        ) : null}
        </div>
      </Modal>
      {!status && isTauriRuntime() && !loadError && (
        <div className="bitfun-privacy-loading" aria-live="polite">
          <LoaderCircle className="bitfun-privacy-gate__loading-icon" size={18} aria-hidden />
          <span className="sr-only">{copy.loading}</span>
        </div>
      )}
    </>
  );
};
