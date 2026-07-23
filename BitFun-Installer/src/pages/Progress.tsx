import { useTranslation } from 'react-i18next';
import { ProgressBar } from '../components/ProgressBar';
import { InstallErrorPanel } from '../components/InstallErrorPanel';
import type { InstallProgress } from '../types/installer';

interface ProgressProps {
  progress: InstallProgress;
  error: string | null;
  canConfirmProgress: boolean;
  onConfirmProgress: () => void;
  onRetry: () => Promise<void>;
  onBackToOptions: () => void;
}

export function ProgressPage({
  progress,
  error,
  canConfirmProgress,
  onConfirmProgress,
  onRetry,
  onBackToOptions,
}: ProgressProps) {
  const { t } = useTranslation();
  const isCompleted = canConfirmProgress || progress.percent >= 100;

  const STEP_LABELS: Record<string, string> = {
    prepare: t('progress.prepare'),
    extract: t('progress.extract'),
    registry: t('progress.registry'),
    shortcuts: t('progress.shortcuts'),
    path: t('progress.path'),
    config: t('progress.config'),
    complete: t('progress.complete'),
  };

  const stepLabel = STEP_LABELS[progress.step] || progress.step || t('progress.starting');

  return (
    <div className="page-shell">
      <div className="page-scroll">
        <div
          className="page-container page-container--center"
          style={{ maxWidth: 420, alignItems: 'center', textAlign: 'center' }}
        >
          {!error ? (
            <>
              <p style={{ fontSize: 16, fontWeight: 600, color: 'var(--color-text-primary)', marginBottom: 6 }}>
                {t('progress.title')}
              </p>
              <p style={{ fontSize: 12, color: 'var(--color-text-muted)', marginBottom: 22 }}>
                {stepLabel}
              </p>
              <div style={{ width: '100%', maxWidth: 320 }}>
                <ProgressBar percent={progress.percent} completed={isCompleted} />
                <div
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    gap: 8,
                    marginTop: 8,
                    fontSize: 11,
                    color: 'var(--color-text-muted)',
                    opacity: 0.7,
                    flexWrap: 'wrap',
                  }}
                >
                  <span>{stepLabel}</span>
                  <span>{progress.percent}%</span>
                </div>
              </div>
            </>
          ) : (
            <>
              <svg
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="var(--color-error)"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                style={{ marginBottom: 14, animation: 'scaleIn 350ms ease forwards' }}
              >
                <circle cx="12" cy="12" r="10" /><line x1="15" y1="9" x2="9" y2="15" /><line x1="9" y1="9" x2="15" y2="15" />
              </svg>
              <p style={{ fontSize: 14, fontWeight: 500, color: 'var(--color-text-primary)', marginBottom: 8 }}>{t('progress.failed')}</p>
              <InstallErrorPanel message={error} variant="bare" />
            </>
          )}
        </div>
      </div>

      {!error ? (
        canConfirmProgress && (
          <div className="page-footer page-footer--center">
            <button
              type="button"
              className="btn btn-primary"
              onClick={onConfirmProgress}
              style={{ justifyContent: 'center' }}
            >
              {t('progress.confirmContinue')}
            </button>
          </div>
        )
      ) : (
        <div className="page-footer page-footer--center">
          <button className="btn btn-ghost" onClick={onBackToOptions}>
            {t('options.title')}
          </button>
          <button className="btn btn-primary" onClick={() => { void onRetry(); }}>
            {t('options.install')}
          </button>
        </div>
      )}
    </div>
  );
}
