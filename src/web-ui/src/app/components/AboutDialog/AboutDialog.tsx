/**
 * About dialog component.
 * Shows product identity, build metadata, license, app-update status, and the
 * persistent GitHub repository entry point.
 */

import {
  Alert,
  Button,
  FieldGroup,
  FieldRow,
  Icon,
  IconButton,
  StatusPill,
  Tooltip,
  Dialog,
  DialogBody,
  DialogClose,
  DialogTitle,
} from '@openbitfun/ui';
import React, { useCallback, useEffect, useState } from 'react';
import { useI18n } from '@/infrastructure/i18n';
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
import { AboutBrandMark } from './AboutBrandMark';
import './AboutDialog.scss';

const log = createLogger('AboutDialog');
const GITHUB_REPOSITORY_URL = 'https://github.com/GCWing/OpenBitFun';

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
  const requestInstall = useUpdateInstallStore(state => state.requestInstall);
  const updateVersion = useUpdateInstallStore(state => state.version);
  const updateInitialized = useUpdateInstallStore(state => state.initialized);

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
    if (canCheckForAppUpdates()) void useUpdateInstallStore.getState().initialize();
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

  const onRestart = useCallback(() => {
    onClose();
    requestInstall();
  }, [onClose, requestInstall]);

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
  const updateBusy = !updateInitialized || manualCheckBusy || updateStatus === 'downloading' || updateStatus === 'ready' || updateStatus === 'installing';

  return (
    <>
      <Dialog
        open={isOpen}
        onOpenChange={(nextOpen) => { if (!nextOpen) onClose(); }}
        size="xl"
        className="openbitfun-about-dialog"
        aria-label={t('about.dialogTitle')}
        data-testid="about-dialog-modal"
      >
        <DialogClose className="openbitfun-about-dialog__close" />
        <DialogBody className="openbitfun-about-dialog__modal-content" inset="none">
          <div
            className="openbitfun-about-dialog__content"
            data-openbitfun-component="about-dialog"
            data-openbitfun-part="root"
          >
            <div className="openbitfun-about-dialog__body">
              <div
                className="openbitfun-about-dialog__brand"
                data-openbitfun-component="about-dialog"
                data-openbitfun-part="hero"
                aria-hidden="true"
              >
                <div className="openbitfun-about-dialog__artwork">
                  <AboutBrandMark active={isOpen} />
                </div>
                <p className="openbitfun-about-dialog__brand-statement">
                  {t('about.brandStatement')}
                </p>
              </div>

              <section
                className="openbitfun-about-dialog__metadata"
                data-openbitfun-component="about-dialog"
                data-openbitfun-part="content"
                aria-label={t('about.details')}
              >
                <header className="openbitfun-about-dialog__brand-copy">
                  <DialogTitle
                    className="openbitfun-about-dialog__title"
                    data-openbitfun-component="about-dialog"
                    data-openbitfun-part="title"
                  >
                    {version.name}
                  </DialogTitle>
                  <p className="openbitfun-about-dialog__tagline">{t('about.tagline')}</p>
                </header>

                <FieldGroup className="openbitfun-about-dialog__details" appearance="plain" dividers={false}>
                  <FieldRow padding="none">
                    <dl className="openbitfun-about-dialog__info-row" data-openbitfun-component="about-dialog" data-openbitfun-part="infoRow">
                      <dt className="openbitfun-about-dialog__info-label" data-openbitfun-component="about-dialog" data-openbitfun-part="infoLabel">
                        <span>{t('about.versionLabel')}</span>
                      </dt>
                      <dd className="openbitfun-about-dialog__info-value-group">
                        <span
                          className="openbitfun-about-dialog__info-value"
                          data-openbitfun-component="about-dialog"
                          data-openbitfun-part="infoValue"
                          data-testid="about-version-value"
                        >
                          {displayedVersion}
                        </span>
                        <span
                          className="openbitfun-about-dialog__channel-badge"
                          data-openbitfun-component="about-dialog"
                          data-openbitfun-part="channelBadge"
                        >
                          <StatusPill tone="neutral">{releaseLabel}</StatusPill>
                        </span>
                      </dd>
                    </dl>
                  </FieldRow>

                  <FieldRow padding="none">
                    <dl className="openbitfun-about-dialog__info-row" data-openbitfun-component="about-dialog" data-openbitfun-part="infoRow">
                      <dt className="openbitfun-about-dialog__info-label" data-openbitfun-component="about-dialog" data-openbitfun-part="infoLabel">
                        <span>{t('about.buildDate')}</span>
                      </dt>
                      <dd className="openbitfun-about-dialog__info-value-group">
                        <span className="openbitfun-about-dialog__info-value" data-openbitfun-component="about-dialog" data-openbitfun-part="infoValue">
                          {formatBuildDate(version.buildDate)}
                        </span>
                      </dd>
                    </dl>
                  </FieldRow>

                  <FieldRow padding="none">
                    <dl className="openbitfun-about-dialog__info-row" data-openbitfun-component="about-dialog" data-openbitfun-part="infoRow">
                      <dt className="openbitfun-about-dialog__info-label" data-openbitfun-component="about-dialog" data-openbitfun-part="infoLabel">
                        <span>{t('about.commit')}</span>
                      </dt>
                      <dd className="openbitfun-about-dialog__info-value-group">
                        <span
                          className="openbitfun-about-dialog__info-value openbitfun-about-dialog__info-value--mono"
                          data-openbitfun-component="about-dialog"
                          data-openbitfun-part="infoValue"
                        >
                          {version.gitCommit ?? t('about.notAvailable')}
                        </span>
                        {version.gitCommit ? (
                          <span
                            className="openbitfun-about-dialog__copy-action"
                            data-openbitfun-component="about-dialog"
                            data-openbitfun-part="copyButton"
                          >
                            <Tooltip content={t('about.copy')}>
                              <IconButton
                                size="xs"
                                variant="quiet"
                                icon={<Icon name={copiedItem === 'commit' ? 'check-line' : 'duplicate'} size="sm" />}
                                onClick={() => void copyToClipboard(version.gitCommit ?? '', 'commit')}
                                aria-label={t('about.copyCommit')}
                              />
                            </Tooltip>
                          </span>
                        ) : null}
                      </dd>
                    </dl>
                  </FieldRow>

                  <FieldRow padding="none">
                    <dl className="openbitfun-about-dialog__info-row" data-openbitfun-component="about-dialog" data-openbitfun-part="infoRow">
                      <dt className="openbitfun-about-dialog__info-label" data-openbitfun-component="about-dialog" data-openbitfun-part="infoLabel">
                        <span>{t('about.branch')}</span>
                      </dt>
                      <dd className="openbitfun-about-dialog__info-value-group">
                        <span
                          className="openbitfun-about-dialog__info-value"
                          data-openbitfun-component="about-dialog"
                          data-openbitfun-part="infoValue"
                          data-testid="about-branch-value"
                          title={version.gitBranch}
                        >
                          {version.gitBranch ?? t('about.notAvailable')}
                        </span>
                      </dd>
                    </dl>
                  </FieldRow>

                  <FieldRow padding="none">
                    <dl className="openbitfun-about-dialog__info-row" data-openbitfun-component="about-dialog" data-openbitfun-part="infoRow">
                      <dt className="openbitfun-about-dialog__info-label" data-openbitfun-component="about-dialog" data-openbitfun-part="infoLabel">
                        <span>{t('about.license')}</span>
                      </dt>
                      <dd className="openbitfun-about-dialog__info-value-group">
                        <span
                          className="openbitfun-about-dialog__info-value"
                          data-openbitfun-component="about-dialog"
                          data-openbitfun-part="license"
                          data-testid="about-license-value"
                        >
                          {licenseName}
                        </span>
                      </dd>
                    </dl>
                  </FieldRow>
                </FieldGroup>

                {updateChecksAvailable ? (
                  <div
                    className="openbitfun-about-dialog__update-card"
                    data-openbitfun-component="about-dialog"
                    data-openbitfun-part="updateCard"
                    data-openbitfun-state={updateState}
                  >
                    <div
                      className="openbitfun-about-dialog__update-card-actions"
                      data-openbitfun-component="about-dialog"
                      data-openbitfun-part="updateActions"
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
                      className="openbitfun-about-dialog__update-feedback"
                      data-openbitfun-component="about-dialog"
                      data-openbitfun-part="updateFeedback"
                    >
                      {manualCheckStatus === 'error' && manualCheckErrorMessage ? (
                        <Alert
                          tone="error"
                          message={manualCheckErrorMessage}
                          showIcon
                          className="openbitfun-about-dialog__update-alert"
                        />
                      ) : null}
                      {updateStatus === 'downloading' ? (
                        <div className="openbitfun-about-dialog__download-status" role="status">
                          <div
                            className="openbitfun-about-dialog__download-bar"
                            data-openbitfun-component="about-dialog"
                            data-openbitfun-part="progress"
                            role="progressbar"
                            aria-valuemin={0}
                            aria-valuemax={100}
                            aria-valuenow={updateProgressPercent ?? undefined}
                            aria-label={t('update.downloadingTitle')}
                          >
                            <div
                              data-openbitfun-component="about-dialog"
                              data-openbitfun-part="progressFill"
                              className={updateProgressPercent != null
                                ? 'openbitfun-about-dialog__download-fill'
                                : 'openbitfun-about-dialog__download-fill openbitfun-about-dialog__download-fill--indeterminate'}
                              style={updateProgressPercent != null
                                ? { width: `${updateProgressPercent}%` }
                                : undefined}
                            />
                          </div>
                          <div className="openbitfun-about-dialog__download-meta">
                            <span>{t('update.backgroundDownloading')}</span>
                            <span>
                              {updateProgressPercent != null
                                ? t('update.progressPercent', { percent: String(updateProgressPercent) })
                                : t('update.progressUnknown')}
                            </span>
                          </div>
                          <p className="openbitfun-about-dialog__download-hint">
                            {t('update.backgroundDownloadHint')}
                          </p>
                        </div>
                      ) : null}
                      {updateStatus === 'ready' || updateStatus === 'installing' ? (
                        <div className="openbitfun-about-dialog__update-installed">
                          <div className="openbitfun-about-dialog__update-status openbitfun-about-dialog__update-status--success">
                            <Icon name="check-circle" size="sm" className="openbitfun-about-dialog__update-status-icon" aria-hidden="true" />
                            <span>{t('update.readyVersion', { version: updateVersion ?? '' })}</span>
                          </div>
                          <Button variant="fill" size="sm" disabled={updateStatus === 'installing'} onClick={onRestart}>
                            {t(updateStatus === 'installing' ? 'update.installing' : 'update.installAndRestart')}
                          </Button>
                        </div>
                      ) : null}
                      {updateStatus === 'error' && updateError ? (
                        <Alert
                          tone="error"
                          message={formatUpdateInstallError(updateError, t)}
                          showIcon
                          className="openbitfun-about-dialog__update-alert"
                        />
                      ) : null}
                    </div>
                  </div>
                ) : null}
              </section>
            </div>

            <footer
              className="openbitfun-about-dialog__footer"
              data-openbitfun-component="about-dialog"
              data-openbitfun-part="footer"
            >
              <div
                className="openbitfun-about-dialog__star-callout"
                data-openbitfun-component="about-dialog"
                data-openbitfun-part="starCallout"
                role="group"
                aria-labelledby="openbitfun-about-star-title"
              >
                <div className="openbitfun-about-dialog__star-copy">
                  <h3 id="openbitfun-about-star-title" className="openbitfun-about-dialog__star-title">
                    {t('about.githubStarTitle')}
                  </h3>
                  <p className="openbitfun-about-dialog__star-description">
                    {t('about.githubStarDescription')}
                  </p>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  className="openbitfun-about-dialog__star-button"
                  leadingIcon={<Icon name="star" size="sm" aria-hidden="true" />}
                  onClick={handleGithubStar}
                  data-testid="about-github-star"
                >
                  {t('about.githubStarAction')}
                </Button>
              </div>
              <p
                className="openbitfun-about-dialog__copyright"
                data-openbitfun-component="about-dialog"
                data-openbitfun-part="copyright"
              >
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
