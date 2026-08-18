/**
 * About dialog component.
 * Shows app version and license info.
 * Uses component library Modal.
 */

import React, { useCallback, useEffect, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { Tooltip, Modal, Button, Alert } from '@/component-library';
import { Copy, Check, Download, CheckCircle2 } from 'lucide-react';
import {
  getAboutInfo,
  formatDisplayedVersion,
  formatBuildDate
} from '@/shared/utils/version';
import { createLogger } from '@/shared/utils/logger';
import { systemAPI } from '@/infrastructure/api';
import type { CheckForUpdatesResponse } from '@/infrastructure/api/service-api/SystemAPI';
import { canCheckForAppUpdates, isTauriRuntime } from '@/infrastructure/update/tauriEnv';
import { UpdateAvailableDialog } from '@/infrastructure/update/UpdateAvailableDialog';
import { useUpdateInstallStore } from '@/infrastructure/update/updateInstallStore';
import { formatUpdateInstallError } from '@/infrastructure/update/updateErrorMessage';
import './AboutDialog.scss';

const log = createLogger('AboutDialog');

interface AboutDialogProps {
  /** Whether visible */
  isOpen: boolean;
  /** Close callback */
  onClose: () => void;
}

export const AboutDialog: React.FC<AboutDialogProps> = ({
  isOpen,
  onClose
}) => {
  const { t } = useI18n('common');
  const [copiedItem, setCopiedItem] = useState<string | null>(null);
  const [manualCheckBusy, setManualCheckBusy] = useState(false);
  const [manualCheckStatus, setManualCheckStatus] = useState<'idle' | 'latest' | 'error'>('idle');
  const [manualCheckErrorMessage, setManualCheckErrorMessage] = useState<string | null>(null);
  const [manualOpen, setManualOpen] = useState(false);
  const [manualData, setManualData] = useState<CheckForUpdatesResponse | null>(null);
  const [nativeVersion, setNativeVersion] = useState<string | null>(null);
  const updateStatus = useUpdateInstallStore(state => state.status);
  const updateProgress = useUpdateInstallStore(state => state.progress);
  const updateError = useUpdateInstallStore(state => state.error);
  const startUpdateInstall = useUpdateInstallStore(state => state.startInstall);

  const aboutInfo = getAboutInfo();
  const { version, license } = aboutInfo;
  const nativeRuntime = isTauriRuntime();
  const updateChecksAvailable = canCheckForAppUpdates();
  const displayedVersion = formatDisplayedVersion(
    version,
    nativeVersion,
    nativeRuntime,
    import.meta.env.DEV
  );
  const updateProgressPercent =
    updateProgress.total != null && updateProgress.total > 0
      ? Math.min(100, Math.round((updateProgress.downloaded / updateProgress.total) * 100))
      : null;

  useEffect(() => {
    if (isOpen) {
      setManualCheckStatus('idle');
      setManualCheckErrorMessage(null);
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen || !nativeRuntime) return;
    let active = true;
    void systemAPI.getAppVersion()
      .then(currentVersion => {
        if (active) setNativeVersion(currentVersion);
      })
      .catch(error => {
        log.warn('get_app_version failed; using generated version metadata', error);
      });
    return () => {
      active = false;
    };
  }, [isOpen, nativeRuntime]);

  const handleCheckForUpdates = useCallback(async () => {
    if (!canCheckForAppUpdates()) {
      return;
    }
    setManualCheckStatus('idle');
    setManualCheckErrorMessage(null);
    setManualCheckBusy(true);
    try {
      const res = await systemAPI.checkForUpdates();
      if (!res.updateAvailable) {
        setManualCheckStatus('latest');
      } else {
        setManualData(res);
        setManualOpen(true);
      }
    } catch (e) {
      log.error('check_for_updates failed', e);
      const msg = e instanceof Error ? e.message : String(e);
      setManualCheckErrorMessage(formatUpdateInstallError(msg, t));
      setManualCheckStatus('error');
    } finally {
      setManualCheckBusy(false);
    }
  }, [t]);

  const onManualLater = useCallback(() => {
    setManualOpen(false);
    setManualData(null);
  }, []);

  const onManualInstall = useCallback(() => {
    setManualOpen(false);
    setManualData(null);
    void startUpdateInstall();
  }, [startUpdateInstall]);

  const onRestart = useCallback(async () => {
    try {
      await systemAPI.restartApp();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      useUpdateInstallStore.setState({ status: 'error', error: msg });
    }
  }, []);

  const copyToClipboard = async (text: string, itemId: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedItem(itemId);
      setTimeout(() => setCopiedItem(null), 2000);
    } catch (err) {
      log.error('Failed to copy to clipboard', err);
    }
  };

  return (
    <>
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('header.about')}
      showCloseButton={true}
      size="medium"
      testId="about-dialog-modal"
    >
      <div
        className="bitfun-about-dialog__content"
        data-bf-component="about-dialog"
        data-bf-part="root"
      >
        {/* Hero section - product info */}
        <div className="bitfun-about-dialog__hero" data-bf-component="about-dialog" data-bf-part="hero">
          <h1 className="bitfun-about-dialog__title" data-bf-component="about-dialog" data-bf-part="title">{version.name}</h1>
          <div className="bitfun-about-dialog__version-badge" data-bf-component="about-dialog" data-bf-part="version">
            {t('about.version', { version: displayedVersion })}
          </div>
          <div className="bitfun-about-dialog__divider" data-bf-component="about-dialog" data-bf-part="decoration" />
          <div className="bitfun-about-dialog__dots" data-bf-component="about-dialog" data-bf-part="decoration">
            <span></span>
            <span></span>
            <span></span>
          </div>
        </div>

        {/* Scrollable area */}
        <div className="bitfun-about-dialog__scrollable" data-bf-component="about-dialog" data-bf-part="content">
          {updateChecksAvailable ? (
            <div
              className="bitfun-about-dialog__update-card"
              data-bf-component="about-dialog"
              data-bf-part="updateCard"
              data-bf-state={`${manualCheckBusy ? 'checking' : ''} ${manualCheckStatus} ${updateStatus}`.trim()}
            >
              <div className="bitfun-about-dialog__update-card-top" data-bf-component="about-dialog" data-bf-part="updateHeader">
                <div className="bitfun-about-dialog__update-card-main">
                  <div className="bitfun-about-dialog__update-card-head">
                    <div className="bitfun-about-dialog__update-card-icon" aria-hidden>
                      <Download size={18} strokeWidth={2} />
                    </div>
                    <div className="bitfun-about-dialog__update-card-meta">
                      <div className="bitfun-about-dialog__update-card-title">
                        {t('about.updateSectionTitle')}
                      </div>
                      <p className="bitfun-about-dialog__update-card-hint">
                        {t('about.updateSectionHint')}
                      </p>
                    </div>
                  </div>
                  <div className="bitfun-about-dialog__update-card-feedback" data-bf-component="about-dialog" data-bf-part="updateFeedback">
                    {manualCheckStatus === 'latest' ? (
                      <div
                        className="bitfun-about-dialog__update-status bitfun-about-dialog__update-status--success"
                        role="status"
                      >
                        <CheckCircle2 size={14} aria-hidden />
                        <span>{t('update.noUpdate')}</span>
                      </div>
                    ) : null}
                    {manualCheckStatus === 'error' && manualCheckErrorMessage ? (
                      <Alert
                        type="error"
                        message={manualCheckErrorMessage}
                        showIcon
                        className="bitfun-about-dialog__update-alert"
                      />
                    ) : null}
                  </div>
                </div>
                <div className="bitfun-about-dialog__update-card-actions" data-bf-component="about-dialog" data-bf-part="updateActions">
                  <Button
                    variant="secondary"
                    size="small"
                    isLoading={manualCheckBusy}
                    disabled={updateStatus === 'downloading' || updateStatus === 'installed'}
                    onClick={() => void handleCheckForUpdates()}
                  >
                    {!manualCheckBusy ? (
                      <Check size={14} className="bitfun-about-dialog__update-btn-icon" aria-hidden />
                    ) : null}
                    {manualCheckBusy ? t('update.checking') : t('update.checkForUpdates')}
                  </Button>
                </div>
              </div>
              {updateStatus === 'downloading' ? (
                <div className="bitfun-about-dialog__download-status" role="status">
                  <div
                    className="bitfun-about-dialog__download-bar"
                    data-bf-component="about-dialog"
                    data-bf-part="progress"
                    role="progressbar"
                    aria-valuemin={0}
                    aria-valuemax={100}
                    aria-valuenow={updateProgressPercent ?? undefined}
                    aria-label={t('update.downloadingTitle')}
                  >
                    <div
                      data-bf-component="about-dialog"
                      data-bf-part="progressFill"
                      className={
                        updateProgressPercent != null
                          ? 'bitfun-about-dialog__download-fill'
                          : 'bitfun-about-dialog__download-fill bitfun-about-dialog__download-fill--indeterminate'
                      }
                      style={
                        updateProgressPercent != null
                          ? { width: `${updateProgressPercent}%` }
                          : undefined
                      }
                    />
                  </div>
                  <div className="bitfun-about-dialog__download-meta">
                    <span>{t('update.backgroundDownloading')}</span>
                    <span>
                      {updateProgressPercent != null
                        ? t('update.progressPercent', { percent: String(updateProgressPercent) })
                        : t('update.progressUnknown')}
                    </span>
                  </div>
                  <p className="bitfun-about-dialog__download-hint">
                    {t('update.backgroundDownloadHint')}
                  </p>
                </div>
              ) : null}
              {updateStatus === 'installed' ? (
                <div className="bitfun-about-dialog__update-installed">
                  <div className="bitfun-about-dialog__update-status bitfun-about-dialog__update-status--success">
                    <CheckCircle2 size={14} aria-hidden />
                    <span>{t('update.installedMessage')}</span>
                  </div>
                  <Button variant="primary" size="small" onClick={onRestart}>
                    {t('update.restartNow')}
                  </Button>
                </div>
              ) : null}
              {updateStatus === 'error' && updateError ? (
                <Alert
                  type="error"
                  message={formatUpdateInstallError(updateError, t)}
                  showIcon
                  className="bitfun-about-dialog__update-alert"
                />
              ) : null}
            </div>
          ) : !nativeRuntime ? (
            <p className="bitfun-about-dialog__update-hint">{t('update.desktopOnly')}</p>
          ) : null}
          <div className="bitfun-about-dialog__info-section">
            <div className="bitfun-about-dialog__info-card" data-bf-component="about-dialog" data-bf-part="infoCard">
              <div className="bitfun-about-dialog__info-row" data-bf-component="about-dialog" data-bf-part="infoRow">
                <span className="bitfun-about-dialog__info-label" data-bf-component="about-dialog" data-bf-part="infoLabel">{t('about.buildDate')}</span>
                <span className="bitfun-about-dialog__info-value" data-bf-component="about-dialog" data-bf-part="infoValue">
                  {formatBuildDate(version.buildDate)}
                </span>
              </div>

              {version.gitCommit && (
                <div className="bitfun-about-dialog__info-row" data-bf-component="about-dialog" data-bf-part="infoRow">
                  <span className="bitfun-about-dialog__info-label" data-bf-component="about-dialog" data-bf-part="infoLabel">{t('about.commit')}</span>
                  <div className="bitfun-about-dialog__info-value-group">
                    <span className="bitfun-about-dialog__info-value bitfun-about-dialog__info-value--mono" data-bf-component="about-dialog" data-bf-part="infoValue">
                      {version.gitCommit}
                    </span>
                    <Tooltip content={t('about.copy')}>
                      <button
                        className="bitfun-about-dialog__copy-btn"
                        data-bf-component="about-dialog"
                        data-bf-part="copyButton"
                        onClick={() => copyToClipboard(version.gitCommit || '', 'commit')}
                      >
                        {copiedItem === 'commit' ? <Check size={12} /> : <Copy size={12} />}
                      </button>
                    </Tooltip>
                  </div>
                </div>
              )}

              {version.gitBranch && (
                <div className="bitfun-about-dialog__info-row" data-bf-component="about-dialog" data-bf-part="infoRow">
                  <span className="bitfun-about-dialog__info-label" data-bf-component="about-dialog" data-bf-part="infoLabel">{t('about.branch')}</span>
                  <span className="bitfun-about-dialog__info-value" data-bf-component="about-dialog" data-bf-part="infoValue">{version.gitBranch}</span>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="bitfun-about-dialog__footer" data-bf-component="about-dialog" data-bf-part="footer">
          <p className="bitfun-about-dialog__license" data-bf-component="about-dialog" data-bf-part="license">{license.text}</p>
          <p className="bitfun-about-dialog__copyright" data-bf-component="about-dialog" data-bf-part="copyright">
            {t('about.copyright')}
          </p>
        </div>
      </div>
    </Modal>

      <UpdateAvailableDialog
        isOpen={manualOpen}
        variant="manual"
        data={manualData}
        onLater={onManualLater}
        onInstall={onManualInstall}
      />
    </>
  );
};

export default AboutDialog;
