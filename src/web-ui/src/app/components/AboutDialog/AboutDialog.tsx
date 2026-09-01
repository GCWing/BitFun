/**
 * About dialog component.
 * Shows product identity, build metadata, license, app-update status, and the
 * persistent GitHub repository entry point.
 */

import {
  Alert,
  Button,
  Icon,
  ScrollArea,
  Tooltip,
  Dialog,
  DialogBody,
  DialogClose,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@bitfun/ui';
import React, { useCallback, useEffect, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import { CalendarDays, Code2, ShieldCheck, Sparkle, Tag } from 'lucide-react';
import {
  formatBuildDate,
  formatDisplayedVersion,
  getAboutInfo,
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
const GITHUB_REPOSITORY_URL = 'https://github.com/GCWing/BitFun';
const ABOUT_DOT_MATRIX_COLUMNS = 13;
const ABOUT_DOT_MATRIX_ROWS = 7;
const ABOUT_DOT_MATRIX = Array.from(
  { length: ABOUT_DOT_MATRIX_COLUMNS * ABOUT_DOT_MATRIX_ROWS },
  (_, index) => {
    const column = index % ABOUT_DOT_MATRIX_COLUMNS;
    const row = Math.floor(index / ABOUT_DOT_MATRIX_COLUMNS);
    return {
      id: index,
      opacity: Math.max(0.12, 0.44 - column * 0.017 - row * 0.022),
    };
  },
);

interface AboutDialogProps {
  /** Whether visible */
  isOpen: boolean;
  /** Close callback */
  onClose: () => void;
}

export const AboutDialog: React.FC<AboutDialogProps> = ({
  isOpen,
  onClose,
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
    import.meta.env.DEV,
  );
  const updateProgressPercent =
    updateProgress.total != null && updateProgress.total > 0
      ? Math.min(100, Math.round((updateProgress.downloaded / updateProgress.total) * 100))
      : null;
  const licenseName = license.type === 'MIT' ? 'MIT License' : license.type;
  const licenseCopyright = license.text?.startsWith(`${licenseName} - `)
    ? license.text.slice(`${licenseName} - `.length)
    : license.text;
  const legalCopyright = licenseCopyright
    ? `${licenseCopyright.replace(/\.$/, '')}. ${t('about.allRightsReserved')}`
    : t('about.copyright');

  let releaseLabel = t('about.stableBuild');
  if (displayedVersion.endsWith('-dev')) {
    releaseLabel = t('about.developmentBuild');
  } else if (version.releaseChannel === 'beta') {
    releaseLabel = t('about.betaBuild');
  } else if (version.releaseChannel === 'nightly') {
    releaseLabel = t('about.nightlyBuild');
  }

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
    if (!canCheckForAppUpdates()) return;

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
    } catch (error) {
      log.error('check_for_updates failed', error);
      const message = error instanceof Error ? error.message : String(error);
      setManualCheckErrorMessage(formatUpdateInstallError(message, t));
      setManualCheckStatus('error');
    } finally {
      setManualCheckBusy(false);
    }
  }, [t]);

  const handleGithubStar = useCallback(() => {
    systemAPI.openExternal(GITHUB_REPOSITORY_URL).catch(error => {
      log.error('Failed to open the GitHub repository', { url: GITHUB_REPOSITORY_URL, error });
    });
  }, []);

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
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      useUpdateInstallStore.setState({ status: 'error', error: message });
    }
  }, []);

  const copyToClipboard = async (text: string, itemId: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedItem(itemId);
      window.setTimeout(() => setCopiedItem(null), 2000);
    } catch (error) {
      log.error('Failed to copy to clipboard', error);
    }
  };

  const updateState = `${manualCheckBusy ? 'checking' : ''} ${manualCheckStatus} ${updateStatus}`.trim();
  const updateBusy = manualCheckBusy || updateStatus === 'downloading' || updateStatus === 'installed';

  return (
    <>
      <Dialog
        open={isOpen}
        onOpenChange={(nextOpen) => { if (!nextOpen) onClose(); }}
        size="2xl"
        data-testid="about-dialog-modal"
      >
        <DialogHeader>
          <DialogHeading>
            <DialogTitle>{t('about.dialogTitle')}</DialogTitle>
          </DialogHeading>
          <DialogClose />
        </DialogHeader>
        <DialogBody className="bitfun-about-dialog__modal-content" inset="none">
        <div
          className="bitfun-about-dialog__content"
          data-bf-component="about-dialog"
          data-bf-part="root"
        >
          <div className="bitfun-about-dialog__body">
            <ScrollArea
              className="bitfun-about-dialog__brand"
              data-bf-component="about-dialog"
              data-bf-part="hero"
              aria-labelledby="bitfun-about-product-name"
            >
              <div className="bitfun-about-dialog__brand-copy">
                <h1
                  id="bitfun-about-product-name"
                  className="bitfun-about-dialog__title"
                  data-bf-component="about-dialog"
                  data-bf-part="title"
                >
                  {version.name}
                </h1>
                <p className="bitfun-about-dialog__tagline">{t('about.tagline')}</p>
              </div>

              <div className="bitfun-about-dialog__release">
                <div className="bitfun-about-dialog__release-summary">
                  <span className="bitfun-about-dialog__release-version">{displayedVersion}</span>
                  <span
                    className="bitfun-about-dialog__channel-badge"
                    data-bf-component="about-dialog"
                    data-bf-part="channelBadge"
                  >
                    {releaseLabel}
                  </span>
                </div>

                {updateChecksAvailable ? (
                  <div
                    className="bitfun-about-dialog__update-card"
                    data-bf-component="about-dialog"
                    data-bf-part="updateCard"
                    data-bf-state={updateState}
                  >
                    <div
                      className="bitfun-about-dialog__update-card-actions"
                      data-bf-component="about-dialog"
                      data-bf-part="updateActions"
                    >
                      {manualCheckStatus === 'latest' && updateStatus === 'idle' ? (
                        <Button
                          variant="outline"
                          size="sm"
                          leadingIcon={<Icon name="check-circle" size="sm" aria-hidden="true" />}
                          onClick={() => void handleCheckForUpdates()}
                          data-testid="about-check-updates"
                        >
                          {t('update.noUpdate')}
                        </Button>
                      ) : (
                        <Button
                          variant="outline"
                          size="sm"
                          leadingIcon={<Icon name="refresh" size="sm" aria-hidden="true" />}
                          loading={manualCheckBusy}
                          disabled={updateBusy}
                          onClick={() => void handleCheckForUpdates()}
                          data-testid="about-check-updates"
                        >
                          {manualCheckBusy ? t('update.checking') : t('update.checkForUpdates')}
                        </Button>
                      )}
                    </div>

                    <div
                      className="bitfun-about-dialog__update-feedback"
                      data-bf-component="about-dialog"
                      data-bf-part="updateFeedback"
                    >
                      {manualCheckStatus === 'error' && manualCheckErrorMessage ? (
                        <Alert
                          tone="error"
                          message={manualCheckErrorMessage}
                          showIcon
                          className="bitfun-about-dialog__update-alert"
                        />
                      ) : null}
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
                              className={updateProgressPercent != null
                                ? 'bitfun-about-dialog__download-fill'
                                : 'bitfun-about-dialog__download-fill bitfun-about-dialog__download-fill--indeterminate'}
                              style={updateProgressPercent != null
                                ? { width: `${updateProgressPercent}%` }
                                : undefined}
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
                            <Icon name="check-circle" size="sm" aria-hidden="true" />
                            <span>{t('update.installedMessage')}</span>
                          </div>
                          <Button variant="fill" size="sm" onClick={onRestart}>
                            {t('update.restartNow')}
                          </Button>
                        </div>
                      ) : null}
                      {updateStatus === 'error' && updateError ? (
                        <Alert
                          tone="error"
                          message={formatUpdateInstallError(updateError, t)}
                          showIcon
                          className="bitfun-about-dialog__update-alert"
                        />
                      ) : null}
                    </div>
                  </div>
                ) : null}
              </div>

              <div
                className="bitfun-about-dialog__dot-matrix"
                aria-hidden="true"
                data-testid="about-dot-matrix"
              >
                {ABOUT_DOT_MATRIX.map(dot => (
                  <span key={dot.id} style={{ opacity: dot.opacity }} />
                ))}
              </div>
            </ScrollArea>

            <ScrollArea
              className="bitfun-about-dialog__metadata"
              data-bf-component="about-dialog"
              data-bf-part="content"
              aria-label={t('about.details')}
            >
              <div className="bitfun-about-dialog__info-row" data-bf-component="about-dialog" data-bf-part="infoRow">
                <div className="bitfun-about-dialog__info-label" data-bf-component="about-dialog" data-bf-part="infoLabel">
                  <Tag size={21} strokeWidth={1.75} aria-hidden="true" />
                  <span>{t('about.versionLabel')}</span>
                </div>
                <span
                  className="bitfun-about-dialog__info-value bitfun-about-dialog__info-value--version"
                  data-bf-component="about-dialog"
                  data-bf-part="infoValue"
                  data-testid="about-version-value"
                >
                  {displayedVersion}
                </span>
              </div>

              <div className="bitfun-about-dialog__info-row" data-bf-component="about-dialog" data-bf-part="infoRow">
                <div className="bitfun-about-dialog__info-label" data-bf-component="about-dialog" data-bf-part="infoLabel">
                  <CalendarDays size={21} strokeWidth={1.75} aria-hidden="true" />
                  <span>{t('about.buildDate')}</span>
                </div>
                <span className="bitfun-about-dialog__info-value" data-bf-component="about-dialog" data-bf-part="infoValue">
                  {formatBuildDate(version.buildDate)}
                </span>
              </div>

              <div className="bitfun-about-dialog__info-row" data-bf-component="about-dialog" data-bf-part="infoRow">
                <div className="bitfun-about-dialog__info-label" data-bf-component="about-dialog" data-bf-part="infoLabel">
                  <Code2 size={21} strokeWidth={1.75} aria-hidden="true" />
                  <span>{t('about.commit')}</span>
                </div>
                <div className="bitfun-about-dialog__info-value-group">
                  <span
                    className="bitfun-about-dialog__info-value bitfun-about-dialog__info-value--mono"
                    data-bf-component="about-dialog"
                    data-bf-part="infoValue"
                  >
                    {version.gitCommit ?? t('about.notAvailable')}
                  </span>
                  {version.gitCommit ? (
                    <Tooltip content={t('about.copy')}>
                      <button
                        type="button"
                        className="bitfun-about-dialog__copy-btn"
                        data-bf-component="about-dialog"
                        data-bf-part="copyButton"
                        onClick={() => void copyToClipboard(version.gitCommit ?? '', 'commit')}
                        aria-label={t('about.copyCommit')}
                      >
                        {copiedItem === 'commit' ? <Icon name="check-line" size="sm" /> : <Icon name="duplicate" size="sm" />}
                      </button>
                    </Tooltip>
                  ) : null}
                </div>
              </div>

              <div className="bitfun-about-dialog__info-row" data-bf-component="about-dialog" data-bf-part="infoRow">
                <div className="bitfun-about-dialog__info-label" data-bf-component="about-dialog" data-bf-part="infoLabel">
                  <Icon name="git" size="lg" aria-hidden="true" />
                  <span>{t('about.branch')}</span>
                </div>
                <span
                  className="bitfun-about-dialog__info-value bitfun-about-dialog__info-value--branch"
                  data-bf-component="about-dialog"
                  data-bf-part="infoValue"
                  data-testid="about-branch-value"
                  title={version.gitBranch}
                >
                  {version.gitBranch ?? t('about.notAvailable')}
                </span>
              </div>

              <div className="bitfun-about-dialog__info-row" data-bf-component="about-dialog" data-bf-part="infoRow">
                <div className="bitfun-about-dialog__info-label" data-bf-component="about-dialog" data-bf-part="infoLabel">
                  <ShieldCheck size={21} strokeWidth={1.75} aria-hidden="true" />
                  <span>{t('about.license')}</span>
                </div>
                <span
                  className="bitfun-about-dialog__info-value"
                  data-bf-component="about-dialog"
                  data-bf-part="license"
                  data-testid="about-license-value"
                >
                  {licenseName}
                </span>
              </div>
            </ScrollArea>
          </div>

          <section
            className="bitfun-about-dialog__star-callout"
            data-bf-component="about-dialog"
            data-bf-part="starCallout"
            aria-labelledby="bitfun-about-star-title"
          >
            <span
              className="bitfun-about-dialog__star-rule bitfun-about-dialog__star-rule--leading"
              aria-hidden="true"
            />
            <Icon name="star" size="lg" className="bitfun-about-dialog__star-icon" aria-hidden="true" />
            <div className="bitfun-about-dialog__star-copy">
              <h2 id="bitfun-about-star-title" className="bitfun-about-dialog__star-title">
                <span>{t('about.githubStarTitle')}</span>
                <Sparkle size={13} fill="currentColor" aria-hidden="true" />
              </h2>
              <p className="bitfun-about-dialog__star-description">
                {t('about.githubStarDescription')}
              </p>
            </div>
            <span className="bitfun-about-dialog__star-rule" aria-hidden="true" />
            <Button
              variant="fill"
              size="md"
              className="bitfun-about-dialog__star-button"
              trailingIcon={<Icon name="arrow-right" size="lg" aria-hidden="true" />}
              onClick={handleGithubStar}
              data-testid="about-github-star"
            >
              {t('about.githubStarAction')}
            </Button>
          </section>

          <footer className="bitfun-about-dialog__footer" data-bf-component="about-dialog" data-bf-part="footer">
            <p className="bitfun-about-dialog__copyright" data-bf-component="about-dialog" data-bf-part="copyright">
              {legalCopyright}
            </p>
          </footer>
        </div>
        </DialogBody>
      </Dialog>

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
