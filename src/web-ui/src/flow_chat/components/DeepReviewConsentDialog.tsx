import React, { useCallback, useState } from 'react';
import { Clock, Coins, ShieldCheck, X } from 'lucide-react';
import { estimateTokenConsumption, formatTokenCount } from '../utils/deepReviewExperience';
import { useTranslation } from 'react-i18next';
import { Button, Checkbox, Modal } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './DeepReviewConsentDialog.scss';

const log = createLogger('DeepReviewConsentDialog');
const SKIP_DEEP_REVIEW_CONFIRMATION_STORAGE_KEY = 'bitfun.deepReview.skipCostConfirmation';

interface PendingConsent {
  resolve: (confirmed: boolean) => void;
}

export interface DeepReviewConsentControls {
  confirmDeepReviewLaunch: () => Promise<boolean>;
  deepReviewConsentDialog: React.ReactNode;
}

export function useDeepReviewConsent(): DeepReviewConsentControls {
  const { t } = useTranslation('flow-chat');
  const [pendingConsent, setPendingConsent] = useState<PendingConsent | null>(null);
  const [dontShowAgain, setDontShowAgain] = useState(false);

  const confirmDeepReviewLaunch = useCallback(async () => {
    try {
      if (localStorage.getItem(SKIP_DEEP_REVIEW_CONFIRMATION_STORAGE_KEY) === 'true') {
        return true;
      }
    } catch (error) {
      log.warn('Failed to read Deep Review confirmation preference from local storage', error);
    }

    return new Promise<boolean>((resolve) => {
      setDontShowAgain(false);
      setPendingConsent({ resolve });
    });
  }, []);

  const settleConsent = useCallback(async (confirmed: boolean) => {
    const pending = pendingConsent;
    if (!pending) {
      return;
    }

    if (confirmed && dontShowAgain) {
      try {
        localStorage.setItem(SKIP_DEEP_REVIEW_CONFIRMATION_STORAGE_KEY, 'true');
      } catch (error) {
        log.warn('Failed to persist Deep Review confirmation preference to local storage', error);
      }
    }

    setPendingConsent(null);
    pending.resolve(confirmed);
  }, [dontShowAgain, pendingConsent]);

  const deepReviewConsentDialog = pendingConsent ? (
    <Modal
      isOpen={true}
      onClose={() => void settleConsent(false)}
      size="large"
      closeOnOverlayClick={false}
      showCloseButton={false}
      contentClassName="deep-review-consent-modal"
    >
      <div className="deep-review-consent">
        <div className="deep-review-consent__header">
          <div className="deep-review-consent__heading">
            <span className="deep-review-consent__eyebrow">
              {t('deepReviewConsent.eyebrow', { defaultValue: 'Code review team' })}
            </span>
            <h3>{t('deepReviewConsent.title')}</h3>
          </div>
          <button
            type="button"
            className="deep-review-consent__close"
            aria-label={t('deepReviewConsent.cancel')}
            onClick={() => void settleConsent(false)}
          >
            <X size={16} />
          </button>
        </div>

        <p className="deep-review-consent__lead">{t('deepReviewConsent.body')}</p>

        <div className="deep-review-consent__safety-note">
          <div className="deep-review-consent__fact-icon">
            <ShieldCheck size={16} />
          </div>
          <div>
            <span className="deep-review-consent__fact-title">
              {t('deepReviewConsent.readonlyLabel', { defaultValue: 'Read-only first pass' })}
            </span>
            <p>{t('deepReviewConsent.readonly')}</p>
          </div>
        </div>

        <div className="deep-review-consent__facts" aria-label={t('deepReviewConsent.windowTitle', { defaultValue: 'Deep Review' })}>
          <div className="deep-review-consent__fact">
            <div className="deep-review-consent__fact-icon">
              <Coins size={16} />
            </div>
            <div>
              <span className="deep-review-consent__fact-title">
                {t('deepReviewConsent.costLabel', { defaultValue: 'Higher token usage' })}
              </span>
              <p>{t('deepReviewConsent.cost')}</p>
              <p className="deep-review-consent__token-estimate">
                {(() => {
                  const est = estimateTokenConsumption(5);
                  return t('deepReviewConsent.estimatedTokens', {
                    min: formatTokenCount(est.min),
                    max: formatTokenCount(est.max),
                    defaultValue: 'Estimated: {{min}} - {{max}} tokens',
                  });
                })()}
              </p>
            </div>
          </div>
          <div className="deep-review-consent__fact">
            <div className="deep-review-consent__fact-icon">
              <Clock size={16} />
            </div>
            <div>
              <span className="deep-review-consent__fact-title">
                {t('deepReviewConsent.timeLabel', { defaultValue: 'Longer runtime' })}
              </span>
              <p>{t('deepReviewConsent.time')}</p>
            </div>
          </div>
        </div>

        <div className="deep-review-consent__footer">
          <Checkbox
            className="deep-review-consent__checkbox"
            checked={dontShowAgain}
            onChange={(event) => setDontShowAgain(event.target.checked)}
            label={t('deepReviewConsent.dontShowAgain')}
          />
          <div className="deep-review-consent__actions">
            <Button
              variant="secondary"
              size="small"
              onClick={() => void settleConsent(false)}
            >
              {t('deepReviewConsent.cancel')}
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={() => void settleConsent(true)}
            >
              {t('deepReviewConsent.confirm')}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  ) : null;

  return {
    confirmDeepReviewLaunch,
    deepReviewConsentDialog,
  };
}
