import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Switch, Select } from '@/component-library';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow } from './common';
import { configManager } from '../services/ConfigManager';
import { createLogger } from '@/shared/utils/logger';
import './VoiceConfig.scss';

const log = createLogger('VoiceConfig');

interface VoiceSettings {
  sttEnabled: boolean;
  ttsEnabled: boolean;
  sttProvider: 'webspeech' | 'faster-whisper';
  ttsProvider: 'edge' | 'elevenlabs';
  ttsVoice: string;
  language: string;
}

const DEFAULT_SETTINGS: VoiceSettings = {
  sttEnabled: true,
  ttsEnabled: true,
  sttProvider: 'webspeech',
  ttsProvider: 'edge',
  ttsVoice: 'zh-CN-XiaoxiaoNeural',
  language: 'zh-CN',
};

const EDGE_TTS_VOICES = [
  { id: 'zh-CN-XiaoxiaoNeural', label: 'Xiaoxiao (Mandarin Chinese)' },
  { id: 'zh-CN-YunxiNeural', label: 'Yunxi (Mandarin Chinese)' },
  { id: 'en-US-AriaNeural', label: 'Aria (US English)' },
  { id: 'en-US-JennyNeural', label: 'Jenny (US English)' },
  { id: 'en-US-GuyNeural', label: 'Guy (US English)' },
  { id: 'en-GB-SoniaNeural', label: 'Sonia (UK English)' },
  { id: 'ja-JP-NanamiNeural', label: 'Nanami (Japanese)' },
  { id: 'ko-KR-SunHiNeural', label: 'SunHi (Korean)' },
];

const LANGUAGES = [
  { id: 'zh-CN', label: 'Chinese (Simplified)' },
  { id: 'en-US', label: 'English (US)' },
  { id: 'en-GB', label: 'English (UK)' },
  { id: 'zh-TW', label: 'Chinese (Traditional)' },
  { id: 'ja-JP', label: 'Japanese' },
  { id: 'ko-KR', label: 'Korean' },
];

const VoiceConfig: React.FC = () => {
  const { t } = useTranslation('settings/voice');

  const [settings, setSettings] = useState<VoiceSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);

  const loadSettings = useCallback(async () => {
    setIsLoading(true);
    try {
      const stored = configManager.getConfig<VoiceSettings>('voice');
      setSettings({ ...DEFAULT_SETTINGS, ...stored });
    } catch (err) {
      log.error('Failed to load voice settings', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadSettings();
  }, [loadSettings]);

  const saveSettings = useCallback(async (newSettings: VoiceSettings) => {
    setSettings(newSettings);
    try {
      await configManager.setConfig('voice', newSettings);
    } catch (err) {
      log.error('Failed to save voice settings', err);
    }
  }, []);

  const handleSttEnabledChange = useCallback((checked: boolean) => {
    void saveSettings({ ...settings, sttEnabled: checked });
  }, [settings, saveSettings]);

  const handleTtsEnabledChange = useCallback((checked: boolean) => {
    void saveSettings({ ...settings, ttsEnabled: checked });
  }, [settings, saveSettings]);

  const handleSttProviderChange = useCallback((value: string) => {
    void saveSettings({ ...settings, sttProvider: value as VoiceSettings['sttProvider'] });
  }, [settings, saveSettings]);

  const handleTtsProviderChange = useCallback((value: string) => {
    void saveSettings({ ...settings, ttsProvider: value as VoiceSettings['ttsProvider'] });
  }, [settings, saveSettings]);

  const handleTtsVoiceChange = useCallback((value: string) => {
    void saveSettings({ ...settings, ttsVoice: value });
  }, [settings, saveSettings]);

  const handleLanguageChange = useCallback((value: string) => {
    void saveSettings({ ...settings, language: value });
  }, [settings, saveSettings]);

  if (isLoading) {
    return (
      <ConfigPageLayout className="bitfun-voice-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent className="bitfun-voice-config__content">
          <ConfigPageSection title="">
            <ConfigPageRow label="" align="center">
              <span />
            </ConfigPageRow>
          </ConfigPageSection>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout className="bitfun-voice-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
      <ConfigPageContent className="bitfun-voice-config__content">
        <ConfigPageSection title="">
          <ConfigPageRow
            label={t('stt.enabled.label')}
            description={t('stt.enabled.description')}
            align="center"
          >
            <Switch
              checked={settings.sttEnabled}
              onChange={(e) => { void handleSttEnabledChange(e.target.checked); }}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={t('tts.enabled.label')}
            description={t('tts.enabled.description')}
            align="center"
          >
            <Switch
              checked={settings.ttsEnabled}
              onChange={(e) => { void handleTtsEnabledChange(e.target.checked); }}
              size="small"
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection title={t('sections.stt')}>
          <ConfigPageRow
            label={t('stt.provider')}
            description={t('stt.providerDescription')}
          >
            <Select
              value={settings.sttProvider}
              onChange={(value) => { void handleSttProviderChange(String(value)); }}
              options={[
                { value: 'webspeech', label: t('stt.providers.webspeech') },
                { value: 'faster-whisper', label: t('stt.providers.fasterWhisper') },
              ]}
              disabled={!settings.sttEnabled}
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection title={t('sections.tts')}>
          <ConfigPageRow
            label={t('tts.provider')}
            description={t('tts.providerDescription')}
          >
            <Select
              value={settings.ttsProvider}
              onChange={(value) => { void handleTtsProviderChange(String(value)); }}
              options={[
                { value: 'edge', label: t('tts.providers.edge') },
                { value: 'elevenlabs', label: t('tts.providers.elevenlabs') },
              ]}
              disabled={!settings.ttsEnabled}
            />
          </ConfigPageRow>

          {settings.ttsProvider === 'edge' && (
            <ConfigPageRow
              label={t('tts.voice')}
            >
              <Select
                value={settings.ttsVoice || 'en-US-AriaNeural'}
                onChange={(value) => { void handleTtsVoiceChange(String(value)); }}
                options={EDGE_TTS_VOICES.map((v) => ({ value: v.id, label: v.label }))}
                disabled={!settings.ttsEnabled}
              />
            </ConfigPageRow>
          )}

          <ConfigPageRow
            label={t('tts.language')}
          >
            <Select
              value={settings.language}
              onChange={(value) => { void handleLanguageChange(String(value)); }}
              options={LANGUAGES.map((l) => ({ value: l.id, label: l.label }))}
              disabled={!settings.ttsEnabled}
            />
          </ConfigPageRow>
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default VoiceConfig;