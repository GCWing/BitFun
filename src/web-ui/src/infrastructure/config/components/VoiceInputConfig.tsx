import { Button, Icon, Input, Select, type SelectOption, StatusPill, type StatusPillTone, Switch } from '@bitfun/ui';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CloudOff, HardDrive, PhoneCall } from 'lucide-react';
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
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigLoadingState,
  ConfigMessage,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
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

type VoiceInputStatus = 'ready' | 'setup' | 'downloading' | 'unavailable' | 'error';

function statusBadgeVariant(status: VoiceInputStatus): StatusPillTone {
  switch (status) {
    case 'ready':
      return 'success';
    case 'downloading':
      return 'info';
    case 'unavailable':
    case 'error':
      return 'danger';
    default:
      return 'neutral';
  }
}

const VoiceInputConfig: React.FC = () => {
  const { t } = useTranslation('settings/voice-input');
  const speechRuntimeSupported = isTauriRuntime();
  const {
    settings,
    isLoading: settingsLoading,
    error: settingsError,
  } = useAIExperienceSettings();
  const [models, setModels] = useState<SpeechModelStatus[]>([]);
  const [modelsLoading, setModelsLoading] = useState(speechRuntimeSupported);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [localModelsOpen, setLocalModelsOpen] = useState(false);
  const [voiceCallDraft, setVoiceCallDraft] = useState<SpeechRealtimeConfig | null>(null);
  const cancelDownloadRequestedRef = useRef<Set<string>>(new Set());

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

  useEffect(() => {
    if (!speechRuntimeSupported) return undefined;
    let active = true;
    void speechAPI.getRealtimeConfig().then(config => {
      if (active) setVoiceCallDraft(config);
    }).catch(error => {
      log.error('Failed to load controller realtime voice call settings', { error });
      notificationService.error(t('voiceCall.messages.loadFailed'));
    });
    return () => {
      active = false;
    };
  }, [speechRuntimeSupported, t]);

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
      const response = await speechAPI.listModels();
      setModels(response.models);
    } catch (error) {
      log.error('Failed to load local speech model status', { error });
      notificationService.error(t('messages.loadFailed'));
    } finally {
      setModelsLoading(false);
    }
  }, [speechRuntimeSupported, t]);

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

  const updateVoiceInput = useCallback(async (patch: Partial<VoiceInputSettings>) => {
    if (!settings) {
      notificationService.error(t('messages.loadFailed'));
      return false;
    }
    try {
      await aiExperienceConfigService.saveSettings({ voice_input: patch });
      return true;
    } catch (error) {
      log.error('Failed to save voice input settings', { error });
      notificationService.error(t('messages.saveFailed'));
      return false;
    }
  }, [settings, t]);

  const updateModelStatus = useCallback((status: SpeechModelStatus) => {
    setModels(previous => previous.map(model =>
      model.modelId === status.modelId ? status : model
    ));
  }, []);

  const saveVoiceCall = useCallback(async () => {
    if (!voiceCallDraft) {
      notificationService.error(t('messages.loadFailed'));
      return;
    }
    try {
      setBusyAction('save-voice-call');
      const saved = await speechAPI.saveRealtimeConfig({
        enabled: voiceCallDraft.enabled,
        apiKey: voiceCallDraft.apiKey.trim(),
        voice: voiceCallDraft.voice.trim(),
        speed: voiceCallDraft.speed,
        loudness: voiceCallDraft.loudness,
        microphoneDeviceId: voiceCallDraft.microphoneDeviceId,
      });
      setVoiceCallDraft(saved);
      window.dispatchEvent(new CustomEvent('bitfun:realtime-voice-config-changed', {
        detail: saved,
      }));
      notificationService.success(t('voiceCall.messages.saved'));
    } catch (error) {
      log.error('Failed to save realtime voice call settings', { error });
      notificationService.error(t('voiceCall.messages.saveFailed'));
    } finally {
      setBusyAction(null);
    }
  }, [t, voiceCallDraft]);

  const handleDownload = useCallback((model: SpeechModelStatus) => {
    if (model.state === 'downloading') return;
    cancelDownloadRequestedRef.current.delete(model.modelId);
    updateModelStatus({
      ...model,
      state: 'downloading',
      installedBytes: 0,
      progress: {
        modelId: model.modelId,
        downloadedBytes: 0,
        totalBytes: model.expectedBytes,
        percent: 0,
      },
      error: null,
    });
    setBusyAction(`download:${model.modelId}`);
    void speechAPI.downloadModel(model.modelId).then(async status => {
      updateModelStatus(status);
      await updateVoiceInput({ provider: 'local', model_id: model.modelId });
      notificationService.success(t('messages.downloadSuccess'));
    }).catch(error => {
      if (cancelDownloadRequestedRef.current.has(model.modelId)) return;
      log.error('Failed to download local speech model', { modelId: model.modelId, error });
      notificationService.error(t('messages.downloadFailed'));
      void loadModels();
    }).finally(() => {
      cancelDownloadRequestedRef.current.delete(model.modelId);
      setBusyAction(null);
    });
  }, [loadModels, t, updateModelStatus, updateVoiceInput]);

  const handleCancelDownload = useCallback(async (model: SpeechModelStatus) => {
    cancelDownloadRequestedRef.current.add(model.modelId);
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
          <ConfigMessage message={{ type: 'error', text: t('messages.loadFailed') }} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  let status: VoiceInputStatus = 'setup';
  if (legacyCloudSelection) status = 'unavailable';
  else if (!selectedModel || selectedModel.state === 'error' || selectedModel.state === 'corrupt') status = 'error';
  else if (selectedModel.state === 'installed') status = 'ready';
  else if (selectedModel.state === 'downloading' || selectedModel.state === 'verifying') status = 'downloading';

  const progressPercent = Math.min(100, Math.max(0, selectedModel?.progress?.percent ?? 0));
  const statusIcon = status === 'ready'
    ? <Icon name="check-circle" size="lg" />
    : status === 'unavailable'
      ? <CloudOff size={18} />
      : status === 'setup'
        ? <Icon name="download" size="lg" />
        : <HardDrive size={18} />;

  return (
    <ConfigPageLayout className="voice-input-config" data-bf-component="voice-input-config" data-bf-part="root">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="voice-input-config__content">
        <ConfigPageSection title={t('sections.basic')}>
          <ConfigPageRow
            label={t('composer.enabled.label')}
            description={t('composer.enabled.description')}
            align="center"
          >
            <Switch
              checked={voiceInput.enabled}
              onChange={(event) => void updateVoiceInput({ enabled: event.target.checked })}
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('status.label')} multiline>
            <div className="voice-input-config__status-panel">
              <div
                className={`voice-input-config__status-card voice-input-config__status-card--${status}`}
                data-bf-component="voice-input-config"
                data-bf-part="statusCard"
                data-bf-status={status}
              >
                <div className="voice-input-config__status-icon" aria-hidden="true">{statusIcon}</div>
                <div className="voice-input-config__status-copy">
                  <div className="voice-input-config__status-heading">
                    <div className="voice-input-config__status-title">{t(`status.${status}.title`)}</div>
                    <StatusPill tone={statusBadgeVariant(status)}>{t(`status.${status}.badge`)}</StatusPill>
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
                </div>
                <div className="voice-input-config__status-actions" data-bf-component="voice-input-config" data-bf-part="statusActions">
                  {status === 'unavailable' ? (
                    <Button variant="fill" size="sm" onClick={() => void handleUseLocal()}>
                      {t('status.useLocal')}
                    </Button>
                  ) : null}
                  {status === 'setup' && selectedModel ? (
                    <Button
                      variant="fill"
                      size="sm"
                      onClick={() => handleDownload(selectedModel)}
                      loading={busyAction === `download:${selectedModel.modelId}`}
                      leadingIcon={<Icon name="download" size="sm" />}
                    >

                      {t('status.downloadAndEnable')}
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
                    variant={status === 'error' ? 'outline' : 'outline'}
                    size="sm"
                    onClick={() => setLocalModelsOpen(true)}
                    leadingIcon={<HardDrive size={14} />}
                  >

                    {status === 'error' ? t('status.repair') : t('status.manageModels')}
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
              className="voice-input-config__select"
            />
          </ConfigPageRow>

          <VoiceInputDiagnostics
            settings={voiceInput}
            modelInstalled={!legacyCloudSelection && selectedModel?.state === 'installed'}
            unavailableReason={legacyCloudSelection ? t('diagnostics.recognition.cloudUnavailable') : undefined}
            onDeviceChange={async microphoneDeviceId => {
              await updateVoiceInput({ microphone_device_id: microphoneDeviceId });
            }}
          />
        </ConfigPageSection>

        {voiceCallDraft ? (
          <ConfigPageSection title={t('voiceCall.title')}>
            <ConfigPageRow
              label={t('voiceCall.enabled.label')}
              description={t('voiceCall.enabled.description')}
              align="center"
            >
              <Switch
                checked={voiceCallDraft.enabled}
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
                className="voice-input-config__credential-input"
                type="password"
                size="sm"
                autoComplete="off"
                value={voiceCallDraft.apiKey}
                placeholder={t('voiceCall.apiKey.placeholder')}
                disabled={!voiceCallDraft.enabled || !speechRuntimeSupported}
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
                className="voice-input-config__credential-input"
                size="sm"
                value={voiceCallDraft.voice}
                disabled={!voiceCallDraft.enabled || !speechRuntimeSupported}
                onChange={(event) => setVoiceCallDraft(previous => previous ? ({
                  ...previous,
                  voice: event.target.value,
                }) : previous)}
              />
            </ConfigPageRow>
            <ConfigPageRow
              label={t('voiceCall.status.label')}
              description={speechRuntimeSupported
                ? t('voiceCall.status.description')
                : t('messages.unsupported')}
              align="center"
            >
              <Button
                variant="fill"
                size="sm"
                leadingIcon={<PhoneCall size={14} />}
                loading={busyAction === 'save-voice-call'}
                disabled={
                  !speechRuntimeSupported
                  || !voiceCallDraft.voice.trim()
                  || (voiceCallDraft.enabled && !voiceCallDraft.apiKey.trim())
                }
                onClick={() => void saveVoiceCall()}
              >
                {t('voiceCall.save')}
              </Button>
            </ConfigPageRow>
          </ConfigPageSection>
        ) : null}

      </ConfigPageContent>
      <LocalVoiceModelsConfig
        isOpen={localModelsOpen}
        onClose={() => setLocalModelsOpen(false)}
      />
    </ConfigPageLayout>
  );
};

export default VoiceInputConfig;
