import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, FolderOpen, HardDrive, RefreshCw, ShieldCheck, Trash2 } from 'lucide-react';
import {
  Badge,
  Button,
  Select,
  Switch,
  confirmDanger,
  type BadgeVariant,
  type SelectOption,
} from '@/component-library';
import {
  DEFAULT_MAX_RECORDING_SECONDS,
  LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_ID,
  LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID,
  speechAPI,
  workspaceAPI,
  type SpeechModelInstallState,
  type SpeechModelStatus,
} from '@/infrastructure/api';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { useAIExperienceSettings } from '../hooks';
import { configManager } from '../services/ConfigManager';
import { getProviderDisplayName } from '../services/modelConfigs';
import {
  aiExperienceConfigService,
  type AIExperienceSettings,
} from '../services/AIExperienceConfigService';
import type { AIModelConfig, DefaultModelsConfig, VoiceInputSettings } from '../types';
import { VoiceInputDiagnostics } from './VoiceInputDiagnostics';
import {
  ConfigPageContent,
  ConfigPageHeader,
  ConfigPageLayout,
  ConfigPageLoading,
  ConfigPageMessage,
  ConfigPageRow,
  ConfigPageSection,
} from './common';
import './VoiceInputConfig.scss';

const log = createLogger('VoiceInputConfig');

const normalizeSelectValue = (value: string | number | (string | number)[]): string =>
  String(Array.isArray(value) ? (value[0] ?? '') : value);

const DEFAULT_LOCAL_VOICE_MODEL_ID = LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID;
const QWEN_ASR_FLASH_MODEL_ID = 'qwen3-asr-flash';
const QWEN_ASR_BASE_URL = 'https://dashscope.aliyuncs.com/compatible-mode/v1';

const MODEL_RESOURCE_HINT_KEYS: Record<string, string> = {
  [LOCAL_SENSEVOICE_SMALL_INT8_MODEL_ID]: 'model.resourceHints.sensevoice',
  [LOCAL_QWEN3_ASR_0_6B_INT8_MODEL_ID]: 'model.resourceHints.qwen3',
};

type VoiceInputProvider = 'local' | 'cloud';
type CloudSpeechProviderPreset = 'qwen' | 'custom';

interface CloudSpeechDraft {
  configId?: string;
  preset: CloudSpeechProviderPreset;
  name: string;
  baseUrl: string;
  modelName: string;
  apiKey: string;
}

function trimTrailingSlashes(value: string): string {
  return value.trim().replace(/\/+$/, '');
}

function hasHttpUrlScheme(value: string): boolean {
  return /^https?:\/\//i.test(value.trim());
}

function resolveTranscriptionRequestUrl(baseUrl: string): string {
  const trimmed = trimTrailingSlashes(baseUrl);
  if (trimmed.endsWith('/audio/transcriptions')) {
    return trimmed;
  }
  return `${trimmed}/audio/transcriptions`;
}

function isQwenAsrConfig(model?: AIModelConfig | null): boolean {
  if (!model) return true;
  return (
    model.model_name === QWEN_ASR_FLASH_MODEL_ID ||
    model.base_url.includes('dashscope.aliyuncs.com/compatible-mode')
  );
}

function createDefaultCloudSpeechDraft(): CloudSpeechDraft {
  return {
    preset: 'qwen',
    name: 'Qwen ASR',
    baseUrl: QWEN_ASR_BASE_URL,
    modelName: QWEN_ASR_FLASH_MODEL_ID,
    apiKey: '',
  };
}

function createCloudSpeechDraftFromModel(model?: AIModelConfig | null): CloudSpeechDraft {
  if (!model) {
    return createDefaultCloudSpeechDraft();
  }
  const preset = isQwenAsrConfig(model) ? 'qwen' : 'custom';
  return {
    configId: model.id,
    preset,
    name: getProviderDisplayName(model),
    baseUrl: model.base_url || (preset === 'qwen' ? QWEN_ASR_BASE_URL : ''),
    modelName: model.model_name || (preset === 'qwen' ? QWEN_ASR_FLASH_MODEL_ID : ''),
    apiKey: model.api_key || '',
  };
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return '0 B';
  }
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const digits = value >= 10 || unitIndex === 0 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unitIndex]}`;
}

function clampRecordingSeconds(value: number): number {
  if (!Number.isFinite(value)) {
    return DEFAULT_MAX_RECORDING_SECONDS;
  }
  return Math.min(300, Math.max(5, Math.round(value)));
}

function statusBadgeVariant(state: SpeechModelInstallState): BadgeVariant {
  switch (state) {
    case 'installed':
      return 'success';
    case 'downloading':
    case 'verifying':
      return 'info';
    case 'corrupt':
    case 'error':
      return 'error';
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
  const [cloudModels, setCloudModels] = useState<AIModelConfig[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModelsConfig>({});
  const [cloudDraft, setCloudDraft] = useState<CloudSpeechDraft>(createDefaultCloudSpeechDraft);
  const [loading, setLoading] = useState(speechRuntimeSupported);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const cancelDownloadRequestedRef = useRef<Set<string>>(new Set());

  const voiceInput = settings?.voice_input;
  const selectedProvider: VoiceInputProvider = voiceInput?.provider === 'cloud' ? 'cloud' : 'local';
  const selectedLocalModelId = selectedProvider === 'local'
    ? (voiceInput?.model_id || DEFAULT_LOCAL_VOICE_MODEL_ID)
    : DEFAULT_LOCAL_VOICE_MODEL_ID;
  const selectedCloudModelId = selectedProvider === 'cloud'
    ? voiceInput?.model_id
    : defaultModels.speech_recognition;
  const selectedModel = useMemo(
    () => models.find(item => item.modelId === selectedLocalModelId) ?? models[0],
    [models, selectedLocalModelId],
  );
  const selectedCloudModel = useMemo(
    () => cloudModels.find(model => model.id === selectedCloudModelId) ?? cloudModels[0],
    [cloudModels, selectedCloudModelId],
  );
  const anyDownloading = models.some(item => item.state === 'downloading');
  const selectedLocalModelUsable = selectedModel?.state === 'installed';
  const firstInstalledLocalModel = useMemo(
    () => models.find(item => item.state === 'installed'),
    [models],
  );
  const providerOptions = useMemo<SelectOption[]>(() => [
    { label: t('composer.provider.local'), value: 'local' },
    { label: t('composer.provider.cloud'), value: 'cloud' },
  ], [t]);
  const cloudPresetOptions = useMemo<SelectOption[]>(() => [
    { label: t('cloudConfig.presets.qwen'), value: 'qwen' },
    { label: t('cloudConfig.presets.custom'), value: 'custom' },
  ], [t]);
  const localModelOptions = useMemo<SelectOption[]>(() => models.map(item => ({
    label: item.displayName,
    value: item.modelId,
    disabled: item.state !== 'installed',
  })), [models]);
  const cloudModelOptions = useMemo<SelectOption[]>(() => cloudModels.map(model => ({
    label: `${getProviderDisplayName(model)} / ${model.model_name}`,
    value: model.id || '',
  })), [cloudModels]);
  const languageOptions = useMemo<SelectOption[]>(() => {
    const languages = selectedModel?.languages?.length
      ? selectedModel.languages
      : ['auto', 'zh', 'yue', 'en', 'ja', 'ko'];
    return languages.map(language => ({
      label: t(`languages.${language}`, { defaultValue: language.toUpperCase() }),
      value: language,
    }));
  }, [selectedModel, t]);

  useEffect(() => {
    setCloudDraft(createCloudSpeechDraftFromModel(selectedCloudModel));
  }, [selectedCloudModel]);

  const loadData = useCallback(async () => {
    if (!speechRuntimeSupported) {
      setLoading(false);
      return;
    }
    try {
      setLoading(true);
      const [modelResponse, aiModels, defaultModelsConfig] = await Promise.all([
        speechAPI.listModels(),
        configManager.getConfig<AIModelConfig[]>('ai.models'),
        configManager.getConfig<DefaultModelsConfig>('ai.default_models'),
      ]);
      setModels(modelResponse.models);
      setCloudModels((aiModels || []).filter(model => {
        const capabilities = Array.isArray(model.capabilities) ? model.capabilities : [];
        return !!model.enabled && (
          model.category === 'speech_recognition' ||
          capabilities.includes('speech_recognition')
        );
      }));
      setDefaultModels(defaultModelsConfig || {});
    } catch (error) {
      log.error('Failed to load voice input settings', { error });
      notificationService.error(t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [speechRuntimeSupported, t]);

  useEffect(() => {
    if (!speechRuntimeSupported) {
      return undefined;
    }
    void loadData();
    const unsubscribeProgress = speechAPI.onModelProgress(event => {
      setModels(previous => previous.map(item =>
        item.modelId === event.status.modelId ? event.status : item
      ));
    });
    const unsubscribeStatus = speechAPI.onModelStatusChanged(status => {
      setModels(previous => previous.map(item =>
        item.modelId === status.modelId ? status : item
      ));
    });
    const unsubscribeAiModels = configManager.watch('ai.models', () => {
      void loadData();
    });
    const unsubscribeDefaultModels = configManager.watch('ai.default_models', () => {
      void loadData();
    });

    return () => {
      unsubscribeProgress();
      unsubscribeStatus();
      unsubscribeAiModels();
      unsubscribeDefaultModels();
    };
  }, [loadData, speechRuntimeSupported]);

  const updateVoiceInput = useCallback(async (
    patch: Partial<VoiceInputSettings>,
    options?: { silent?: boolean },
  ) => {
    if (!settings) {
      notificationService.error(t('messages.loadFailed'));
      return;
    }
    const nextSettings: AIExperienceSettings = {
      ...settings,
      voice_input: {
        ...settings.voice_input,
        ...patch,
      },
    };
    try {
      await aiExperienceConfigService.saveSettings(nextSettings);
      if (!options?.silent) {
        notificationService.success(t('messages.saveSuccess'));
      }
    } catch (error) {
      log.error('Failed to save voice input settings', { error });
      notificationService.error(t('messages.saveFailed'));
    }
  }, [settings, t]);

  const updateModelStatus = useCallback((status: SpeechModelStatus) => {
    setModels(previous => previous.map(item =>
      item.modelId === status.modelId ? status : item
    ));
  }, []);

  const handleCloudPresetChange = useCallback((value: string | number | (string | number)[]) => {
    const preset = normalizeSelectValue(value) as CloudSpeechProviderPreset;
    setCloudDraft(previous => {
      if (preset === 'qwen') {
        return {
          ...previous,
          preset,
          name: previous.name.trim() || 'Qwen ASR',
          baseUrl: QWEN_ASR_BASE_URL,
          modelName: QWEN_ASR_FLASH_MODEL_ID,
        };
      }
      return {
        ...previous,
        preset: 'custom',
      };
    });
  }, []);

  const handleSaveCloudModel = useCallback(async () => {
    const name = cloudDraft.name.trim() || t('cloudConfig.defaults.providerName');
    const baseUrl = trimTrailingSlashes(cloudDraft.baseUrl);
    const modelName = cloudDraft.modelName.trim();
    const apiKey = cloudDraft.apiKey.trim();

    if (!name || !baseUrl || !modelName || !apiKey) {
      notificationService.warning(t('cloudConfig.messages.fillRequired'));
      return;
    }
    if (!hasHttpUrlScheme(baseUrl)) {
      notificationService.warning(t('cloudConfig.messages.invalidBaseUrl'));
      return;
    }

    setBusyAction('saveCloudModel');
    try {
      const allModels = await configManager.getConfig<AIModelConfig[]>('ai.models') || [];
      const modelId = cloudDraft.configId || selectedCloudModel?.id || `speech_cloud_${Date.now()}`;
      const nextModel: AIModelConfig = {
        id: modelId,
        name,
        provider: 'openai',
        api_key: apiKey,
        base_url: baseUrl,
        request_url: resolveTranscriptionRequestUrl(baseUrl),
        model_name: modelName,
        context_window: 0,
        max_tokens: 0,
        enabled: true,
        category: 'speech_recognition',
        capabilities: ['speech_recognition'],
        recommended_for: ['voice_input'],
        metadata: {
          ...(selectedCloudModel?.metadata || {}),
          speech_provider_preset: cloudDraft.preset,
        },
        auth: { type: 'api_key' },
      };
      const replaced = allModels.some(model => model.id === modelId);
      const nextModels = replaced
        ? allModels.map(model => model.id === modelId ? nextModel : model)
        : [...allModels, nextModel];
      const currentDefaultModels = await configManager.getConfig<DefaultModelsConfig>('ai.default_models') || {};

      await configManager.setConfig('ai.models', nextModels);
      await configManager.setConfig('ai.default_models', {
        ...currentDefaultModels,
        speech_recognition: modelId,
      });
      await updateVoiceInput({
        provider: 'cloud',
        model_id: modelId,
      }, { silent: true });
      setCloudDraft(createCloudSpeechDraftFromModel(nextModel));
      setCloudModels(nextModels.filter(model => {
        const capabilities = Array.isArray(model.capabilities) ? model.capabilities : [];
        return !!model.enabled && (
          model.category === 'speech_recognition' ||
          capabilities.includes('speech_recognition')
        );
      }));
      notificationService.success(t('cloudConfig.messages.saveSuccess'));
    } catch (error) {
      log.error('Failed to save cloud speech model', { error });
      notificationService.error(t('cloudConfig.messages.saveFailed'));
    } finally {
      setBusyAction(null);
    }
  }, [cloudDraft, selectedCloudModel, t, updateVoiceInput]);

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

    void speechAPI.downloadModel(model.modelId).then(status => {
      updateModelStatus(status);
      notificationService.success(t('messages.downloadSuccess'));
    }).catch(error => {
      if (cancelDownloadRequestedRef.current.has(model.modelId)) {
        return;
      }
      log.error('Failed to download speech model', { modelId: model.modelId, error });
      notificationService.error(t('messages.downloadFailed'));
      void loadData();
    }).finally(() => {
      cancelDownloadRequestedRef.current.delete(model.modelId);
    });
  }, [loadData, t, updateModelStatus]);

  const handleCancelDownload = useCallback(async (model: SpeechModelStatus) => {
    cancelDownloadRequestedRef.current.add(model.modelId);
    setBusyAction(`cancel:${model.modelId}`);
    try {
      const status = await speechAPI.cancelModelDownload(model.modelId);
      updateModelStatus(status);
      notificationService.info(t('messages.downloadCancelled'));
    } catch (error) {
      log.error('Failed to cancel speech model download', { modelId: model.modelId, error });
      notificationService.error(t('messages.cancelFailed'));
    } finally {
      setBusyAction(null);
    }
  }, [t, updateModelStatus]);

  const handleVerify = useCallback(async (model: SpeechModelStatus) => {
    setBusyAction(`verify:${model.modelId}`);
    try {
      const status = await speechAPI.verifyModel(model.modelId);
      updateModelStatus(status);
      notificationService.success(t('messages.verifySuccess'));
    } catch (error) {
      log.error('Failed to verify speech model', { modelId: model.modelId, error });
      notificationService.error(t('messages.verifyFailed'));
    } finally {
      setBusyAction(null);
    }
  }, [t, updateModelStatus]);

  const handleOpenFolder = useCallback(async (model: SpeechModelStatus) => {
    if (!model?.installedPath) return;
    try {
      await workspaceAPI.revealInExplorer(model.installedPath);
    } catch (error) {
      log.error('Failed to reveal speech model path', { modelId: model.modelId, error });
      notificationService.error(t('messages.openFolderFailed'));
    }
  }, [t]);

  const handleDelete = useCallback(async (model: SpeechModelStatus) => {
    const confirmed = await confirmDanger(
      t('model.deleteConfirmTitle'),
      t('model.deleteConfirmMessage', { name: model.displayName }),
      {
        confirmText: t('model.delete'),
        cancelText: t('model.keep'),
      },
    );
    if (!confirmed) return;

    setBusyAction(`delete:${model.modelId}`);
    try {
      const status = await speechAPI.deleteModel(model.modelId);
      updateModelStatus(status);
      notificationService.success(t('messages.deleteSuccess'));
    } catch (error) {
      log.error('Failed to delete speech model', { modelId: model.modelId, error });
      notificationService.error(t('messages.deleteFailed'));
    } finally {
      setBusyAction(null);
    }
  }, [t, updateModelStatus]);

  if (!speechRuntimeSupported) {
    return (
      <ConfigPageLayout className="voice-input-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageMessage message={{ type: 'info', text: t('messages.unsupported') }} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (loading || settingsLoading) {
    return (
      <ConfigPageLayout className="voice-input-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageLoading text={t('loading')} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (settingsError || !settings || !voiceInput) {
    return (
      <ConfigPageLayout className="voice-input-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <ConfigPageMessage message={{ type: 'error', text: t('messages.loadFailed') }} />
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="voice-input-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />

      <ConfigPageContent className="voice-input-config__content">
        <ConfigPageSection title={t('sections.composer')}>
          <ConfigPageRow
            label={t('composer.enabled.label')}
            description={t('composer.enabled.description')}
            align="center"
          >
            <Switch
              checked={voiceInput.enabled}
              onChange={(event) => updateVoiceInput({ enabled: event.target.checked })}
              size="small"
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('composer.provider.label')}
            description={t('composer.provider.description')}
            align="center"
          >
            <Select
              value={selectedProvider}
              onChange={(value) => {
                const provider = normalizeSelectValue(value) as VoiceInputProvider;
                if (provider === 'cloud') {
                  void updateVoiceInput({
                    provider: 'cloud',
                    model_id: selectedCloudModel?.id || '',
                  });
                  return;
                }
                void updateVoiceInput({
                  provider: 'local',
                  model_id: selectedLocalModelUsable
                    ? selectedModel.modelId
                    : (firstInstalledLocalModel?.modelId || DEFAULT_LOCAL_VOICE_MODEL_ID),
                });
              }}
              options={providerOptions}
              size="small"
              className="voice-input-config__select"
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('composer.model.label')}
            description={selectedProvider === 'cloud'
              ? t('composer.model.cloudActiveDescription')
              : t('composer.model.localDescription')}
            align="center"
          >
            <Select
              value={selectedProvider === 'cloud'
                ? (selectedCloudModel?.id || '')
                : (selectedModel?.modelId ?? selectedLocalModelId)}
              onChange={(value) => updateVoiceInput({
                provider: selectedProvider,
                model_id: normalizeSelectValue(value),
              })}
              options={selectedProvider === 'cloud' ? cloudModelOptions : localModelOptions}
              placeholder={selectedProvider === 'cloud' ? t('composer.model.cloudPlaceholder') : undefined}
              disabled={selectedProvider === 'cloud' && cloudModelOptions.length === 0}
              size="small"
              className="voice-input-config__model-select"
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('composer.language.label')}
            description={t('composer.language.description')}
            align="center"
          >
            <Select
              value={voiceInput.default_language}
              onChange={(value) => updateVoiceInput({ default_language: normalizeSelectValue(value) })}
              options={languageOptions}
              size="small"
              className="voice-input-config__select"
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('composer.maxRecording.label')}
            description={t('composer.maxRecording.description')}
            align="center"
          >
            <input
              className="voice-input-config__number-input"
              type="number"
              min={5}
              max={300}
              step={5}
              value={voiceInput.max_recording_seconds}
              onChange={(event) => {
                updateVoiceInput({
                  max_recording_seconds: clampRecordingSeconds(Number(event.target.value)),
                });
              }}
              aria-label={t('composer.maxRecording.label')}
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={t('sections.cloudModel')}
          titleSuffix={selectedCloudModel ? (
            <Badge variant="info">
              {t('cloudConfig.inUse')}
            </Badge>
          ) : null}
        >
          <div className="voice-input-config__cloud-note">
            {t('cloudConfig.note')}
          </div>

          <ConfigPageRow
            label={t('cloudConfig.preset.label')}
            description={t('cloudConfig.preset.description')}
            align="center"
          >
            <Select
              value={cloudDraft.preset}
              onChange={handleCloudPresetChange}
              options={cloudPresetOptions}
              size="small"
              className="voice-input-config__select"
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('cloudConfig.providerName.label')}
            description={t('cloudConfig.providerName.description')}
            align="center"
            wide
          >
            <input
              className="voice-input-config__text-input"
              value={cloudDraft.name}
              onChange={(event) => setCloudDraft(previous => ({
                ...previous,
                name: event.target.value,
              }))}
              placeholder={t('cloudConfig.providerName.placeholder')}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('cloudConfig.baseUrl.label')}
            description={t('cloudConfig.baseUrl.description')}
            align="center"
            wide
          >
            <input
              className="voice-input-config__text-input voice-input-config__text-input--wide"
              value={cloudDraft.baseUrl}
              onChange={(event) => setCloudDraft(previous => ({
                ...previous,
                baseUrl: event.target.value,
              }))}
              placeholder={QWEN_ASR_BASE_URL}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('cloudConfig.modelName.label')}
            description={t('cloudConfig.modelName.description')}
            align="center"
            wide
          >
            <input
              className="voice-input-config__text-input"
              value={cloudDraft.modelName}
              onChange={(event) => setCloudDraft(previous => ({
                ...previous,
                modelName: event.target.value,
              }))}
              placeholder={QWEN_ASR_FLASH_MODEL_ID}
            />
          </ConfigPageRow>

          <ConfigPageRow
            label={t('cloudConfig.apiKey.label')}
            description={t('cloudConfig.apiKey.description')}
            align="center"
            wide
          >
            <input
              className="voice-input-config__text-input voice-input-config__text-input--wide"
              type="password"
              autoComplete="off"
              value={cloudDraft.apiKey}
              onChange={(event) => setCloudDraft(previous => ({
                ...previous,
                apiKey: event.target.value,
              }))}
              placeholder={t('cloudConfig.apiKey.placeholder')}
            />
          </ConfigPageRow>

          <div className="voice-input-config__cloud-actions">
            <Button
              variant="primary"
              size="small"
              onClick={() => void handleSaveCloudModel()}
              isLoading={busyAction === 'saveCloudModel'}
              disabled={busyAction !== null && busyAction !== 'saveCloudModel'}
            >
              {t('cloudConfig.save')}
            </Button>
          </div>
        </ConfigPageSection>

        <VoiceInputDiagnostics
          settings={voiceInput}
          modelInstalled={selectedProvider === 'local' && selectedModel?.state === 'installed'}
          onDeviceChange={async microphoneDeviceId => {
            await updateVoiceInput({ microphone_device_id: microphoneDeviceId });
          }}
        />

        <ConfigPageSection
          title={t('sections.model')}
          titleSuffix={selectedModel ? (
            <Badge variant={statusBadgeVariant(selectedModel.state)}>
              {t(`states.${selectedModel.state}`)}
            </Badge>
          ) : null}
          extra={(
            <Button
              variant="ghost"
              size="small"
              onClick={() => void loadData()}
              disabled={busyAction !== null || anyDownloading}
            >
              <RefreshCw size={14} />
              {t('model.refresh')}
            </Button>
          )}
        >
          {models.length > 0 ? (
            <div className="voice-input-config__model-list">
              {models.map(model => {
                const isUsable = model.state === 'installed';
                const isSelected = model.modelId === selectedLocalModelId && selectedProvider === 'local' && isUsable;
                const isDownloading = model.state === 'downloading';
                const progressPercent = Math.min(100, Math.max(0, model.progress?.percent ?? 0));
                const busyKey = busyAction?.endsWith(`:${model.modelId}`) ? busyAction.split(':')[0] : null;
                const resourceHintKey = MODEL_RESOURCE_HINT_KEYS[model.modelId] ?? 'model.resourceHints.default';

                return (
                  <div
                    className={`voice-input-config__model-card${isSelected ? ' voice-input-config__model-card--selected' : ''}`}
                    key={model.modelId}
                  >
                    <div className="voice-input-config__model-main">
                      <div className="voice-input-config__model-icon" aria-hidden="true">
                        <HardDrive size={18} />
                      </div>
                      <div className="voice-input-config__model-copy">
                        <div className="voice-input-config__model-title-row">
                          <div className="voice-input-config__model-name">{model.displayName}</div>
                          {isSelected ? <Badge variant="info">{t('model.selected')}</Badge> : null}
                          <Badge variant={statusBadgeVariant(model.state)}>
                            {t(`states.${model.state}`)}
                          </Badge>
                        </div>
                        <div className="voice-input-config__model-meta">
                          <span>{model.provider}</span>
                          <span>{t('model.version', { version: model.version })}</span>
                          <span>{t('model.size', { size: formatBytes(model.expectedBytes || model.installedBytes) })}</span>
                        </div>
                        <div className="voice-input-config__model-description">{model.description}</div>
                        <div className="voice-input-config__model-resource">{t(resourceHintKey)}</div>
                        {model.installedPath ? (
                          <div className="voice-input-config__model-path">{model.installedPath}</div>
                        ) : null}
                        {model.error ? (
                          <div className="voice-input-config__model-error">{model.error}</div>
                        ) : null}
                      </div>
                    </div>

                    {isDownloading ? (
                      <div className="voice-input-config__progress">
                        <div className="voice-input-config__progress-track" aria-hidden="true">
                          <div
                            className="voice-input-config__progress-value"
                            style={{ width: `${progressPercent}%` }}
                          />
                        </div>
                        <span className="voice-input-config__progress-text">
                          {t('model.progress', {
                            percent: Math.round(progressPercent),
                            downloaded: formatBytes(model.progress?.downloadedBytes ?? model.installedBytes),
                            total: formatBytes(model.progress?.totalBytes ?? model.expectedBytes),
                          })}
                        </span>
                      </div>
                    ) : null}

                    <div className="voice-input-config__model-actions">
                      <Button
                        variant={isSelected ? 'secondary' : 'ghost'}
                        size="small"
                        onClick={() => void updateVoiceInput({
                          provider: 'local',
                          model_id: model.modelId,
                        })}
                        disabled={busyAction !== null || isSelected || !isUsable}
                      >
                        {isSelected ? t('model.selected') : t('model.select')}
                      </Button>

                      {isDownloading ? (
                        <Button
                          variant="secondary"
                          size="small"
                          onClick={() => void handleCancelDownload(model)}
                          isLoading={busyKey === 'cancel'}
                          disabled={busyAction !== null && busyKey !== 'cancel'}
                        >
                          {t('model.cancel')}
                        </Button>
                      ) : (
                        <Button
                          variant="primary"
                          size="small"
                          onClick={() => void handleDownload(model)}
                          disabled={busyAction !== null || model.state === 'installed'}
                        >
                          <Download size={14} />
                          {model.state === 'installed' ? t('model.downloaded') : t('model.download')}
                        </Button>
                      )}

                      <Button
                        variant="secondary"
                        size="small"
                        onClick={() => void handleOpenFolder(model)}
                        disabled={busyAction !== null || !model.installedPath}
                      >
                        <FolderOpen size={14} />
                        {t('model.openFolder')}
                      </Button>

                      <Button
                        variant="secondary"
                        size="small"
                        onClick={() => void handleVerify(model)}
                        isLoading={busyKey === 'verify'}
                        disabled={busyAction !== null || model.state !== 'installed'}
                      >
                        <ShieldCheck size={14} />
                        {t('model.verify')}
                      </Button>

                      <Button
                        variant="danger"
                        size="small"
                        onClick={() => void handleDelete(model)}
                        isLoading={busyKey === 'delete'}
                        disabled={busyAction !== null || model.state !== 'installed'}
                      >
                        <Trash2 size={14} />
                        {t('model.delete')}
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="voice-input-config__empty">{t('model.empty')}</div>
          )}
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default VoiceInputConfig;
