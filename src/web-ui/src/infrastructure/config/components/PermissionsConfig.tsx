import React from 'react';
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  ConfigPageLoading,
  IconButton,
  Modal,
  NumberInput,
  Switch,
} from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { IS_TAURI_DESKTOP, useSessionSettingsConfig } from './useSessionSettingsConfig';
import './AIFeaturesConfig.scss';
import './DebugConfig.scss';

const PermissionsConfig: React.FC = () => {
  const { t } = useTranslation('settings/permissions');
  const {
    isLoading,
    settings,
    skipToolConfirmation,
    confirmationTimeout,
    executionTimeout,
    toolExecConfigLoading,
    computerUseEnabled,
    computerUseAccess,
    computerUseScreen,
    computerUseBusy,
    browserCdpAvailable,
    browserKind,
    browserVersion,
    browserPageCount,
    browserControlBusy,
    browserRestartPrompt,
    platform,
    handleSkipToolConfirmationChange,
    handleComputerUseEnabledChange,
    handleComputerUseOpenSettings,
    refreshComputerUseStatus,
    refreshBrowserControlStatus,
    handleBrowserControlLaunch,
    handleBrowserControlRestart,
    handleBrowserControlCreateLauncher,
    setBrowserRestartPrompt,
    handleToolTimeoutChange,
    tTools,
  } = useSessionSettingsConfig();

  if (isLoading || !settings) {
    return (
      <ConfigPageLayout className="bitfun-func-agent-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent className="bitfun-func-agent-config__content">
          <ConfigPageLoading text={t('loading.text')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-func-agent-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="bitfun-func-agent-config__content">
        <ConfigPageSection
          title={t('toolExecution.sectionTitle')}
          description={t('toolExecution.sectionDescription')}
        >
          <ConfigPageRow
            label={tTools('config.autoExecute')}
            description={tTools('config.autoExecuteDesc')}
            align="center"
          >
            <div className="bitfun-func-agent-config__row-control">
              <Switch
                checked={skipToolConfirmation}
                onChange={(e) => handleSkipToolConfirmationChange(e.target.checked)}
                disabled={toolExecConfigLoading}
                size="small"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={tTools('config.confirmTimeout')}
            description={tTools('config.confirmTimeoutDesc')}
            align="center"
          >
            <div className="bitfun-func-agent-config__row-control">
              <NumberInput
                value={confirmationTimeout === '' ? 0 : parseInt(confirmationTimeout, 10)}
                onChange={(val) => handleToolTimeoutChange('confirmation', val === 0 ? '' : String(val))}
                min={0}
                max={3600}
                step={5}
                unit={tTools('config.seconds')}
                size="small"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
          <ConfigPageRow
            label={tTools('config.executionTimeout')}
            description={tTools('config.executionTimeoutDesc')}
            align="center"
          >
            <div className="bitfun-func-agent-config__row-control">
              <NumberInput
                value={executionTimeout === '' ? 0 : parseInt(executionTimeout, 10)}
                onChange={(val) => handleToolTimeoutChange('execution', val === 0 ? '' : String(val))}
                min={0}
                max={3600}
                step={5}
                unit={tTools('config.seconds')}
                size="small"
                variant="compact"
              />
            </div>
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('computerUse.sectionTitle')}
          description={
            IS_TAURI_DESKTOP ? t('computerUse.sectionDescription') : t('computerUse.desktopOnly')
          }
        >
          {IS_TAURI_DESKTOP ? (
            <>
              <ConfigPageRow label={t('computerUse.enable')} description={t('computerUse.enableDesc')} align="center">
                <div className="bitfun-func-agent-config__row-control">
                  <Switch
                    checked={computerUseEnabled}
                    onChange={(e) => handleComputerUseEnabledChange(e.target.checked)}
                    disabled={computerUseBusy}
                    size="small"
                  />
                </div>
              </ConfigPageRow>
              <ConfigPageRow
                label={t('computerUse.accessibility')}
                description={t('computerUse.accessibilityDesc')}
                align="center"
                balanced
              >
                <div
                  className="bitfun-func-agent-config__row-control"
                  style={{ display: 'flex', flexDirection: 'row', flexWrap: 'nowrap', alignItems: 'center', justifyContent: 'flex-end', gap: 8 }}
                >
                  <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
                    <span className={computerUseAccess ? 'bitfun-func-agent-config__perm-status--granted' : undefined}>
                      {computerUseAccess ? t('computerUse.granted') : t('computerUse.notGranted')}
                    </span>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('computerUse.refreshStatus')}
                      tooltip={t('computerUse.refreshStatus')}
                      disabled={computerUseBusy}
                      onClick={() => void refreshComputerUseStatus()}
                    >
                      <RefreshCw size={14} />
                    </IconButton>
                  </span>
                  <Button
                    className="bitfun-func-agent-config__row-action-btn"
                    size="small"
                    variant="secondary"
                    disabled={computerUseBusy}
                    onClick={() => void handleComputerUseOpenSettings('accessibility')}
                  >
                    {t('computerUse.openSettings')}
                  </Button>
                </div>
              </ConfigPageRow>
              <ConfigPageRow
                label={t('computerUse.screenCapture')}
                description={t('computerUse.screenCaptureDesc')}
                align="center"
                balanced
              >
                <div
                  className="bitfun-func-agent-config__row-control"
                  style={{ display: 'flex', flexDirection: 'row', flexWrap: 'nowrap', alignItems: 'center', justifyContent: 'flex-end', gap: 8 }}
                >
                  <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
                    <span className={computerUseScreen ? 'bitfun-func-agent-config__perm-status--granted' : undefined}>
                      {computerUseScreen ? t('computerUse.granted') : t('computerUse.notGranted')}
                    </span>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('computerUse.refreshStatus')}
                      tooltip={t('computerUse.refreshStatus')}
                      disabled={computerUseBusy}
                      onClick={() => void refreshComputerUseStatus()}
                    >
                      <RefreshCw size={14} />
                    </IconButton>
                  </span>
                  <Button
                    className="bitfun-func-agent-config__row-action-btn"
                    size="small"
                    variant="secondary"
                    disabled={computerUseBusy}
                    onClick={() => void handleComputerUseOpenSettings('screen_capture')}
                  >
                    {t('computerUse.openSettings')}
                  </Button>
                </div>
              </ConfigPageRow>
            </>
          ) : null}
        </ConfigPageSection>

        <ConfigPageSection
          title={t('browserControl.sectionTitle')}
          description={
            IS_TAURI_DESKTOP ? t('browserControl.sectionDescription') : t('browserControl.desktopOnly')
          }
        >
          {IS_TAURI_DESKTOP ? (
            <>
              <ConfigPageRow
                label={t('browserControl.status')}
                description={t('browserControl.statusDesc') || undefined}
                align="center"
                balanced
              >
                <div
                  className="bitfun-func-agent-config__row-control"
                  style={{
                    display: 'flex',
                    flexDirection: 'row',
                    flexWrap: 'wrap',
                    alignItems: 'center',
                    justifyContent: 'flex-end',
                    gap: 8,
                    minWidth: 0,
                  }}
                >
                  <span
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: 6,
                      minWidth: 0,
                      maxWidth: '100%',
                    }}
                    title={browserCdpAvailable && browserVersion ? `${browserKind} ${browserVersion}` : undefined}
                  >
                    <span
                      className={browserCdpAvailable ? 'bitfun-func-agent-config__perm-status--granted' : undefined}
                      style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', minWidth: 0 }}
                    >
                      {browserCdpAvailable
                        ? `${browserKind} · ${browserPageCount} ${t('browserControl.tabs')}`
                        : t('browserControl.notConnected')}
                    </span>
                    <IconButton
                      type="button"
                      size="small"
                      variant="ghost"
                      aria-label={t('browserControl.refreshStatus')}
                      tooltip={t('browserControl.refreshStatus')}
                      disabled={browserControlBusy}
                      onClick={() => void refreshBrowserControlStatus()}
                    >
                      <RefreshCw size={14} />
                    </IconButton>
                  </span>
                  {!browserCdpAvailable && (
                    <Button
                      className="bitfun-func-agent-config__row-action-btn"
                      size="small"
                      variant="secondary"
                      disabled={browserControlBusy}
                      onClick={() => void handleBrowserControlLaunch()}
                    >
                      {t('browserControl.connect')}
                    </Button>
                  )}
                </div>
              </ConfigPageRow>
              {platform === 'macos' && (
                <ConfigPageRow
                  label={t('browserControl.createLauncher')}
                  description={t('browserControl.createLauncherDesc')}
                  align="center"
                >
                  <div className="bitfun-func-agent-config__row-control">
                    <Button
                      className="bitfun-func-agent-config__row-action-btn"
                      size="small"
                      variant="secondary"
                      disabled={browserControlBusy}
                      onClick={() => void handleBrowserControlCreateLauncher()}
                    >
                      {t('browserControl.createLauncher')}
                    </Button>
                  </div>
                </ConfigPageRow>
              )}
            </>
          ) : null}
        </ConfigPageSection>

        <Modal
          isOpen={browserRestartPrompt !== null}
          onClose={() => {
            if (!browserControlBusy) setBrowserRestartPrompt(null);
          }}
          title={t('browserControl.restartModal.title')}
          size="small"
          closeOnOverlayClick={!browserControlBusy}
        >
          <div className="bitfun-debug-config__modal-body">
            <p>{t('browserControl.restartModal.description', { browser: browserRestartPrompt?.browserKind || browserKind })}</p>
            <p>{t('browserControl.restartModal.warning')}</p>
            {browserRestartPrompt?.message ? (
              <p className="bitfun-func-agent-config__hint">{browserRestartPrompt.message}</p>
            ) : null}
          </div>
          <div className="bitfun-debug-config__modal-footer">
            <Button
              variant="secondary"
              size="small"
              onClick={() => setBrowserRestartPrompt(null)}
              disabled={browserControlBusy}
            >
              {t('browserControl.restartModal.cancel')}
            </Button>
            <Button
              variant="primary"
              size="small"
              onClick={() => void handleBrowserControlRestart()}
              disabled={browserControlBusy}
            >
              {browserControlBusy
                ? t('browserControl.restartModal.restarting')
                : t('browserControl.restartModal.confirm')}
            </Button>
          </div>
        </Modal>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default PermissionsConfig;
