/**
 * Speaks a welcome message on app startup.
 * Uses Edge TTS directly to avoid pulling in the full TtsPlayback store subscription.
 */

import { useEffect, useRef } from 'react';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { i18nService } from '@/infrastructure/i18n';
import { createTtsProvider } from '@/infrastructure/voice/TtsProviderFactory';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WelcomeVoice');

interface VoiceSettings {
  sttEnabled: boolean;
  ttsEnabled: boolean;
  sttProvider: 'webspeech' | 'faster-whisper';
  ttsProvider: 'edge' | 'elevenlabs';
  ttsVoice: string;
  language: string;
}

function loadVoiceConfig(): VoiceSettings {
  try {
    const stored = configManager.getConfig<VoiceSettings>('voice');
    return {
      sttEnabled: true,
      ttsEnabled: true,
      sttProvider: 'webspeech',
      ttsProvider: 'edge',
      ttsVoice: 'zh-CN-XiaoxiaoNeural',
      language: 'zh-CN',
      ...stored,
    };
  } catch {
    return {
      sttEnabled: true,
      ttsEnabled: true,
      sttProvider: 'webspeech',
      ttsProvider: 'edge',
      ttsVoice: 'zh-CN-XiaoxiaoNeural',
      language: 'zh-CN',
    };
  }
}

export function useWelcomeVoice() {
  const spokenRef = useRef(false);

  useEffect(() => {
    if (spokenRef.current) return;
    spokenRef.current = true;

    const handleWelcome = async () => {
      const cfg = loadVoiceConfig();
      if (!cfg.ttsEnabled) return;

      try {
        const provider = createTtsProvider(cfg.ttsProvider);
        provider.configure(cfg.ttsVoice, cfg.language);
        await provider.speak(i18nService.t('app.ttsWelcome', { ns: 'common' }));
        log.debug('Welcome voice spoken');
      } catch (err) {
        log.warn('Welcome voice failed', { err });
      }
    };

    // App already dispatched bitfun:main-window-shown before this hook ran (desktop).
    // Dispatch again so this hook catches it immediately.
    window.dispatchEvent(new CustomEvent('bitfun:main-window-shown'));

    window.addEventListener('bitfun:main-window-shown', handleWelcome);
    return () => window.removeEventListener('bitfun:main-window-shown', handleWelcome);
  }, []);
}