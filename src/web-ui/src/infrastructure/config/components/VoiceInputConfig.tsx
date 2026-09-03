import { Button, Input, Select, type SelectOption, StatusPill, type StatusPillTone, Switch } from '@bitfun/ui';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { CloudOff, HardDrive } from 'lucide-react';
import {
  LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID,
  speechAPI,
  type SpeechModelStatus,
  type SpeechRealtimeConfig,
} from '@/infrastructure/api';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { useAIExperienceSettings } from '../hooks';
import { aiExperienceConfigService } from '../services/AIExperienceConfigService';
import type { VoiceInputSettings } from '../types';
import LocalVoiceModelsConfig from './LocalVoiceModelsConfig';
import { VoiceInputDiagnostics } from './VoiceInputDiagnostics';
import {
  ConfigActionBar,
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigLoadingState,
  ConfigMessage,
  ConfigPageRow,
  ConfigPageSection,
  ConfigRetryState,
} from './common';
import { useSettingsDraft } from '@/infrastructure/config/settingsDraftRegistry';
import './VoiceInputConfig.scss';

const log = createLogger('VoiceInputConfig');
const DEFAULT_LOCAL_VOICE_MODEL_ID = LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID;

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KiB', 'MiB', 'GiB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const digits = value >= 10 || unitIndex === 0 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unitIndex]}`;
}

type VoiceInputStatus =
  | 'ready'
  | 'setup'
  | 'downloading'
  | 'verifying'
  | 'deleting'
  | 'unavailable'
  | 'error';

function statusBadgeVariant(status: VoiceInputStatus): StatusPillTone {
  switch (status) {
    case 'ready':
      return 'success';
    case 'downloading':
    case 'verifying':
      return 'info';
    case 'unavailable':
    case 'error':
      return 'danger';
    default:
      return 'neutral';
  }
}

function statusActionKey(status: VoiceInputStatus): string {
  switch (status) {
    case 'setup':
      return 'status.downloadModel';
    case 'downloading':
    case 'verifying':
    case 'deleting':
      return 'status.viewDetails';
    case 'error':
      return 'status.repair';
    default:
      return 'status.manageModels';
  }
}

function realtimeConfigsEqual(
  left: SpeechRealtimeConfig,
  right: SpeechRealtimeConfig,
): boolean {
  return left.enabled === right.enabled
    && left.provider === right.provider
    && left.apiKey === right.apiKey
    && left.voice === right.voice
    && left.speed === right.speed
    && left.loudness === right.loudness
    && left.microphoneDeviceId === right.microphoneDeviceId;
}

const VoiceInputConfig: React.FC = () => {
  const { t } = useTranslation('settings/voice-input');
  const speechRuntimeSupported = isTauriRuntime();
  const {
    settings,
    isLoading: settingsLoading,
    error: settingsError,
    reload: reloadSettings,
  } = useAIExperienceSettings();
  const [models, setModels] = useState<SpeechModelStatus[]>([]);
  const [modelsLoading, setModelsLoading] = useState(speechRuntimeSupported);
  const [modelsLoadFailed, setModelsLoadFailed] = useState(false);
  const [voiceInputSaving, setVoiceInputSaving] = useState(false);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [localModelsOpen, setLocalModelsOpen] = useState(false);
  const [voiceCallDraft, setVoiceCallDraft] = useState<SpeechRealtimeConfig | null>(null);
  const [trustedVoiceCallConfig, setTrustedVoiceCallConfig] = useState<SpeechRealtimeConfig | null>(null);
  const [voiceCallLoading, setVoiceCallLoading] = useState(speechRuntimeSupported);
  const [voiceCallLoadFailed, setVoiceCallLoadFailed] = useState(false);
  const [voiceCallSaveError, setVoiceCallSaveError] = useState<string | null>(null);
  const voiceCallRequestIdRef = useRef(0);
  const voiceCallSavingRef = useRef(false);
  const voiceInputSaveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const pendingVoiceInputSaveCountRef = useRef(0);

  const voiceInput = settings?.voice_input;
  const legacyCloudSelection = voiceInput?.provider === 'cloud';
  const selectedLocalModelId = !legacyCloudSelection && voiceInput?.model_id
    ? voiceInput.model_id
    : DEFAULT_LOCAL_VOICE_MODEL_ID;
  const selectedModel = useMemo(
    () => models.find(model => model.modelId === selectedLocalModelId)
      ?? models.find(model => model.modelId === DEFAULT_LOCAL_VOICE_MODEL_ID)
      ?? models[0],
    [models, selectedLocalModelId],
  );
  const firstInstalledModel = useMemo(
    () => models.find(model => model.state === 'installed'),
    [models],
  );

  const loadVoiceCallConfig = useCallback(async () => {
    if (!speechRuntimeSupported) {
      setVoiceCallLoading(false);
      return;
    }
    const requestId = ++voiceCallRequestIdRef.current;
    setVoiceCallLoading(true);
    setVoiceCallLoadFailed(false);
    try {
      const config = await speechAPI.getRealtimeConfig();
      if (requestId !== voiceCallRequestIdRef.current) return;
      setVoiceCallDraft(config);
      setTrustedVoiceCallConfig(config);
      setVoiceCallSaveError(null);
    } catch (error) {
      if (requestId !== voiceCallRequestIdRef.current) return;
      log.error('Failed to load controller realtime voice call settings', { error });
      setVoiceCallDraft(null);
      setTrustedVoiceCallConfig(null);
      setVoiceCallLoadFailed(true);
    } finally {
      if (requestId === voiceCallRequestIdRef.current) {
        setVoiceCallLoading(false);
      }
    }
  }, [speechRuntimeSupported]);

  useEffect(() => {
    void loadVoiceCallConfig();
    return () => {
      voiceCallRequestIdRef.current += 1;
    };
  }, [loadVoiceCallConfig]);

  const languageOptions = useMemo<SelectOption[]>(() => {
    const languages = selectedModel?.languages?.length
      ? selectedModel.languages
      : ['auto', 'zh', 'yue', 'en', 'ja', 'ko'];
    return languages.map(language => ({
      label: t(`languages.${language}`, { defaultValue: language.toUpperCase() }),
      value: language,
    }));
  }, [selectedModel, t]);

  const loadModels = useCallback(async () => {
    if (!speechRuntimeSupported) {
      setModelsLoading(false);
      return;
    }
    try {
      setModelsLoading(true);
      setModelsLoadFailed(false);
      const response = await speechAPI.listModels();
      setModels(response.models);
    } catch (error) {
      log.error('Failed to load local speech model status', { error });
      setModelsLoadFailed(true);
    } finally {
      setModelsLoading(false);
    }
  }, [speechRuntimeSupported]);

  useEffect(() => {
    if (!speechRuntimeSupported) return undefined;
    void loadModels();
    const unsubscribeProgress = speechAPI.onModelProgress(event => {
      setModels(previous => previous.map(model =>
        model.modelId === event.status.modelId ? event.status : model
      ));
    });
    const unsubscribeStatus = speechAPI.onModelStatusChanged(status => {
      setModels(previous => previous.map(model =>
        model.modelId === status.modelId ? status : model
      ));
    });
    return () => {
      unsubscribeProgress();
      unsubscribeStatus();
    };
  }, [loadModels, speechRuntimeSupported]);

  const updateVoiceInput = useCallback((patch: Partial<VoiceInputSettings>): Promise<boolean> => {
    if (!settings) {
      notificationService.error(t('messages.loadFailed'));
      return Promise.resolve(false);
    }
    pendingVoiceInputSaveCountRef.current += 1;
    setVoiceInputSaving(true);
    const operation = voiceInputSaveQueueRef.current.then(async () => {
      try {
        await aiExperienceConfigService.saveSettings({ voice_input: patch });
        return true;
      } catch (error) {
        log.error('Failed to save voice input settings', { error });
        notificationService.error(t('messages.saveFailed'));
        return false;
      } finally {
        pendingVoiceInputSaveCountRef.current -= 1;
        if (pendingVoiceInputSaveCountRef.current === 0) {
          setVoiceInputSaving(false);
        }
      }
    });
    voiceInputSaveQueueRef.current = operation.then(() => undefined, () => undefined);
    return operation;
  }, [settings, t]);

  const updateModelStatus = useCallback((status: SpeechModelStatus) => {
    setModels(previous => previous.map(model =>
      model.modelId === status.modelId ? status : model
    ));
  }, []);

  const saveVoiceCall = useCallback(async (): Promise<boolean> => {
    if (voiceCallSavingRef.current) return false;
    if (!voiceCallDraft || !trustedVoiceCallConfig) {
      setVoiceCallSaveError(t('messages.loadFailed'));
      notificationService.error(t('messages.loadFailed'));
      return false;
    }
    if (realtimeConfigsEqual(voiceCallDraft, trustedVoiceCallConfig)) {
      return true;
    }
    if (voiceCallDraft.enabled && (
      !voiceCallDraft.voice.trim()
      || !voiceCallDraft.apiKey.trim()
    )) {
      setVoiceCallSaveError(t('voiceCall.messages.required'));
      notificationService.error(t('voiceCall.messages.required'));
      return false;
    }
    try {
      voiceCallSavingRef.current = true;
      setBusyAction('save-voice-call');
      setVoiceCallSaveError(null);
      const saved = await speechAPI.saveRealtimeConfig({
        enabled: voiceCallDraft.enabled,
        apiKey: voiceCallDraft.apiKey.trim(),
        voice: voiceCallDraft.voice.trim(),
        speed: voiceCallDraft.speed,
        loudness: voiceCallDraft.loudness,
        microphoneDeviceId: voiceCallDraft.microphoneDeviceId,
      });
      setVoiceCallDraft(saved);
      setTrustedVoiceCallConfig(saved);
      setVoiceCallSaveError(null);
      window.dispatchEvent(new CustomEvent('bitfun:realtime-voice-config-changed', {
        detail: saved,
      }));
      notificationService.success(t('voiceCall.messages.saved'));
      return true;
    } catch (error) {
      log.error('Failed to save realtime voice call settings', { error });
      setVoiceCallSaveError(t('voiceCall.messages.saveFailed'));
      notificationService.error(t('voiceCall.messages.saveFailed'));
      return false;
    } finally {
      voiceCallSavingRef.current = false;
      setBusyAction(null);
    }
  }, [t, trustedVoiceCallConfig, voiceCallDraft]);

  const voiceCallDirty = voiceCallDraft !== null
    && trustedVoiceCallConfig !== null
    && !realtimeConfigsEqual(voiceCallDraft, trustedVoiceCallConfig);

  const discardVoiceCall = useCallback(() => {
    setVoiceCallDraft(trustedVoiceCallConfig);
    setVoiceCallSaveError(null);
  }, [trustedVoiceCallConfig]);

  useSettingsDraft({
    id: 'realtime-voice-call',
    pageId: 'application.voice',
    label: t('voiceCall.title'),
    dirty: voiceCallDirty,
    saving: busyAction === 'save-voice-call',
    save: saveVoiceCall,
    discard: discardVoiceCall,
  });

  const handleCancelDownload = useCallback(async (model: SpeechModelStatus) => {
    setBusyAction(`cancel:${model.modelId}`);
    try {
      const status = await speechAPI.cancelModelDownload(model.modelId);
      updateModelStatus(status);
      notificationService.info(t('messages.downloadCancelled'));
    } catch (error) {
      log.error('Failed to cancel local speech model download', { modelId: model.modelId, error });
      notificationService.error(t('messages.cancelFailed'));
    } finally {
      setBusyAction(null);
    }
  }, [t, updateModelStatus]);

  const handleUseLocal = useCallback(async () => {
    const modelId = firstInstalledModel?.modelId
      ?? selectedModel?.modelId
      ?? DEFAULT_LOCAL_VOICE_MODEL_ID;
    const saved = await updateVoiceInput({ provider: 'local', model_id: modelId });
    if (saved) notificationService.success(t('messages.localActivated'));
  }, [firstInstalledModel, selectedModel, t, updateVoiceInput]);

  if (!speechRuntimeSupported) {
    return (
      <ConfigPageLayout className="voice-input-config" data-bf-component="voice-input-config" data-bf-part="root">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigMessage message={{ type: 'info', text: t('messages.unsupported') }} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (modelsLoading || settingsLoading) {
    return (
      <ConfigPageLayout className="voice-input-config" data-bf-component="voice-input-config" data-bf-part="root">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigLoadingState label={t('loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (settingsError || !settings || !voiceInput) {
    return (
      <ConfigPageLayout className="voice-input-config" data-bf-component="voice-input-config" data-bf-part="root">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigRetryState
            message={t('messages.loadFailed')}
            retryLabel={t('messages.retry')}
            onRetry={() => void reloadSettings()}
            loading={settingsLoading}
          />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  let status: VoiceInputStatus = 'setup';
  if (legacyCloudSelection) status = 'unavailable';
  else if (!selectedModel) status = 'error';
  else {
    switch (selectedModel.state) {
      case 'installed':
        status = 'ready';
        break;
      case 'downloading':
        status = 'downloading';
        break;
      case 'verifying':
        status = 'verifying';
        break;
      case 'deleting':
        status = 'deleting';
        break;
      case 'corrupt':
      case 'error':
        status = 'error';
        break;
      default:
        status = 'setup';
    }
  }

  const progressPercent = Math.min(100, Math.max(0, selectedModel?.progress?.percent ?? 0));
  const statusIcon = status === 'ready' || status === 'setup'
    ? null
    : status === 'unavailable'
      ? <CloudOff size={18} />
      : <HardDrive size={18} />;

  return (
    <ConfigPageLayout className="voice-input-config" data-bf-component="voice-input-config" data-bf-part="root">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="voice-input-config__content">
        <ConfigPageSection
          title={t('sections.basic')}
          description={t('sections.basicDescription')}
        >
          <ConfigPageRow
            label={t('composer.enabled.label')}
            description={t('composer.enabled.description')}
            align="center"
          >
            <Switch
              checked={voiceInput.enabled}
              disabled={voiceInputSaving || (modelsLoadFailed && !voiceInput.enabled)}
              onChange={(event) => void updateVoiceInput({ enabled: event.target.checked })}
            />
          </ConfigPageRow>
          {modelsLoadFailed ? (
            <ConfigRetryState
              message={t('messages.modelsLoadFailed')}
              retryLabel={t('messages.retry')}
              onRetry={() => void loadModels()}
              loading={modelsLoading}
            />
          ) : (
            <>
              <ConfigPageRow label={t('status.label')} multiline>
                <div className="voice-input-config__status-panel">
                  <div
                    className={`voice-input-config__status-card voice-input-config__status-card--${status}`}
                    data-bf-component="voice-input-config"
                    data-bf-part="statusCard"
                    data-bf-status={status}
                  >
                    {statusIcon ? (
                      <div className="voice-input-config__status-icon" aria-hidden="true">{statusIcon}</div>
                    ) : null}
                    <div className="voice-input-config__status-copy">
                      {status === 'setup' ? (
                        <p className="voice-input-config__status-summary">
                          <Trans
                            i18nKey="status.setup.summary"
                            t={t}
                            components={{
                              warning: <span className="voice-input-config__status-warning" />,
                            }}
                          />
                        </p>
                      ) : status === 'ready' ? (
                        <p className="voice-input-config__status-summary">
                          <Trans
                            i18nKey="status.ready.summary"
                            t={t}
                            values={{
                              model: selectedModel?.displayName ?? t('status.unknownModel'),
                            }}
                            components={{
                              model: <span className="voice-input-config__status-model" />,
                            }}
                          />
                        </p>
                      ) : (
                        <>
                          <div className="voice-input-config__status-heading">
                            <div className="voice-input-config__status-title">{t(`status.${status}.title`)}</div>
                            <StatusPill tone={statusBadgeVariant(status)}>
                              {t(`status.${status}.badge`)}
                            </StatusPill>
                          </div>
                          <div className="voice-input-config__status-description">
                            {t(`status.${status}.description`, {
                              model: selectedModel?.displayName ?? t('status.unknownModel'),
                              size: formatBytes(selectedModel?.expectedBytes ?? 0),
                            })}
                          </div>
                          {selectedModel?.error && status === 'error' ? (
                            <div className="voice-input-config__status-error">{selectedModel.error}</div>
                          ) : null}
                        </>
                      )}
                    </div>
                    <div
                      className="voice-input-config__status-actions"
                      data-bf-component="voice-input-config"
                      data-bf-part="statusActions"
                    >
                      {status === 'unavailable' ? (
                        <Button
                          variant="fill"
                          size="sm"
                          onClick={() => void handleUseLocal()}
                          disabled={voiceInputSaving}
                        >
                          {t('status.useLocal')}
                        </Button>
                      ) : null}
                      {status === 'downloading' && selectedModel?.state === 'downloading' ? (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => void handleCancelDownload(selectedModel)}
                          loading={busyAction === `cancel:${selectedModel.modelId}`}
                        >
                          {t('model.cancel')}
                        </Button>
                      ) : null}
                      <Button
                        variant={status === 'setup' ? 'fill' : 'outline'}
                        size="sm"
                        onClick={() => setLocalModelsOpen(true)}
                      >
                        {t(statusActionKey(status))}
                      </Button>
                    </div>
                  </div>

                  {status === 'downloading' && selectedModel ? (
                    <div className="voice-input-config__progress voice-input-config__status-progress">
                      <div className="voice-input-config__progress-track" aria-hidden="true">
                        <div className="voice-input-config__progress-value" style={{ width: `${progressPercent}%` }} />
                      </div>
                      <span className="voice-input-config__progress-text">
                        {t('model.progress', {
                          percent: Math.round(progressPercent),
                          downloaded: formatBytes(selectedModel.progress?.downloadedBytes ?? selectedModel.installedBytes),
                          total: formatBytes(selectedModel.progress?.totalBytes ?? selectedModel.expectedBytes),
                        })}
                      </span>
                    </div>
                  ) : null}
                </div>
              </ConfigPageRow>

              {status === 'ready' ? (
                <>
                  <ConfigPageRow
                    label={t('composer.language.label')}
                    description={t('composer.language.description')}
                    align="center"
                  >
                    <Select
                      value={voiceInput.default_language}
                      onValueChange={(value) => void updateVoiceInput({ default_language: String(value) })}
                      options={languageOptions}
                      size="sm"
                      disabled={voiceInputSaving}
                    />
                  </ConfigPageRow>

                  <VoiceInputDiagnostics
                    settings={voiceInput}
                    onDeviceChange={async microphoneDeviceId => {
                      await updateVoiceInput({ microphone_device_id: microphoneDeviceId });
                    }}
                  />
                </>
              ) : null}
            </>
          )}
        </ConfigPageSection>

        <ConfigPageSection
          title={t('voiceCall.title')}
          description={t('voiceCall.description')}
        >
          {voiceCallLoading ? (
            <ConfigLoadingState label={t('voiceCall.loading')} />
          ) : voiceCallLoadFailed || !voiceCallDraft || !trustedVoiceCallConfig ? (
            <ConfigRetryState
              message={t('voiceCall.messages.loadFailed')}
              retryLabel={t('messages.retry')}
              onRetry={() => void loadVoiceCallConfig()}
              loading={voiceCallLoading}
            />
          ) : (
            <>
            <ConfigPageRow
              label={t('voiceCall.enabled.label')}
              description={t('voiceCall.enabled.description')}
              align="center"
            >
              <Switch
                checked={voiceCallDraft.enabled}
                disabled={busyAction === 'save-voice-call'}
                onChange={(event) => setVoiceCallDraft(previous => previous ? ({
                  ...previous,
                  enabled: event.target.checked,
                }) : previous)}
              />
            </ConfigPageRow>
            <ConfigPageRow
              label={t('voiceCall.apiKey.label')}
              description={t('voiceCall.apiKey.description')}
              align="center"
            >
              <Input
                type="password"
                size="sm"
                autoComplete="off"
                value={voiceCallDraft.apiKey}
                placeholder={t('voiceCall.apiKey.placeholder')}
                disabled={
                  !voiceCallDraft.enabled
                  || !speechRuntimeSupported
                  || busyAction === 'save-voice-call'
                }
                onChange={(event) => setVoiceCallDraft(previous => previous ? ({
                  ...previous,
                  apiKey: event.target.value,
                }) : previous)}
              />
            </ConfigPageRow>
            <ConfigPageRow
              label={t('voiceCall.voice.label')}
              description={t('voiceCall.voice.description')}
              align="center"
            >
              <Input
                size="sm"
                value={voiceCallDraft.voice}
                disabled={
                  !voiceCallDraft.enabled
                  || !speechRuntimeSupported
                  || busyAction === 'save-voice-call'
                }
                onChange={(event) => setVoiceCallDraft(previous => previous ? ({
                  ...previous,
                  voice: event.target.value,
                }) : previous)}
              />
            </ConfigPageRow>
            <ConfigActionBar
              status={busyAction === 'save-voice-call'
                ? 'saving'
                : voiceCallSaveError
                  ? 'error'
                  : voiceCallDirty
                  ? 'unsaved'
                  : 'saved'}
              statusMessage={voiceCallSaveError}
              saving={busyAction === 'save-voice-call'}
              saveDisabled={
                !speechRuntimeSupported
                || !voiceCallDirty
                || (voiceCallDraft.enabled && (
                  !voiceCallDraft.voice.trim()
                  || !voiceCallDraft.apiKey.trim()
                ))
              }
              discardDisabled={!voiceCallDirty}
              saveLabel={t('voiceCall.save')}
              onSave={() => void saveVoiceCall()}
              onDiscard={discardVoiceCall}
            />
            </>
          )}
        </ConfigPageSection>

      </ConfigPageContent>
      <LocalVoiceModelsConfig
        isOpen={localModelsOpen}
        onClose={() => setLocalModelsOpen(false)}
      />
    </ConfigPageLayout>
  );
};

export default VoiceInputConfig;
