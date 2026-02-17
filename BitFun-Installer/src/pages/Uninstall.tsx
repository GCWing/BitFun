import { useTranslation } from 'react-i18next';
import { ProgressBar } from '../components/ProgressBar';

interface UninstallPageProps {
  installPath: string;
  isUninstalling: boolean;
  uninstallCompleted: boolean;
  uninstallError: string | null;
  uninstallProgress: number;
  onUninstall: () => Promise<void>;
  onClose: () => void;
}

export function UninstallPage({
  installPath,
  isUninstalling,
  uninstallCompleted,
  uninstallError,
  uninstallProgress,
  onUninstall,
  onClose,
}: UninstallPageProps) {
  const { t } = useTranslation();

  return (
    <div className="uninstall-page">
      <div className="uninstall-card">
        <div className="uninstall-title">
          {t('uninstall.title')}
        </div>
        <div className="uninstall-subtitle">
          {t('uninstall.subtitle')}
        </div>

        <div className="uninstall-inline-meta" title={installPath || t('uninstall.pathUnknown')}>
          <span className="uninstall-inline-label">{t('uninstall.installPath')}:</span>
          <span className="uninstall-inline-path">{installPath || t('uninstall.pathUnknown')}</span>
        </div>

        {uninstallError && (
          <div className="uninstall-error">
            {uninstallError}
          </div>
        )}

        {uninstallCompleted && (
          <div className="uninstall-success">
            {t('uninstall.completed')}
          </div>
        )}

        {(isUninstalling || uninstallCompleted) && (
          <div className="uninstall-progress-wrap">
            <ProgressBar percent={uninstallProgress} completed={uninstallCompleted} />
            <span className="uninstall-progress-text">{uninstallProgress}%</span>
          </div>
        )}

        <div className="uninstall-actions">
          <button className="btn btn-ghost" onClick={onClose}>
            {t(uninstallCompleted ? 'uninstall.close' : 'uninstall.cancel')}
          </button>
          {!uninstallCompleted && (
            <button className="btn btn-primary" disabled={isUninstalling} onClick={() => { void onUninstall(); }}>
              {isUninstalling ? t('uninstall.uninstalling') : t('uninstall.confirm')}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
