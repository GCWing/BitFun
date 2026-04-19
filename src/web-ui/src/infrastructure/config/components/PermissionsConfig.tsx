import React from 'react';
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  ConfigPageLoading,
  IconButton,
  NumberInput,
  Switch,
} from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { IS_TAURI_DESKTOP, useSessionSettingsConfig } from './useSessionSettingsConfig';
import './AIFeaturesConfig.scss';

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
    handleSkipToolConfirmationChange,
    handleComputerUseEnabledChange,
    handleComputerUseOpenSettings,
    refreshComputerUseStatus,
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
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default PermissionsConfig;
