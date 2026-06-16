import { createSttProvider, SttProvider } from './SttProviderFactory';
import { createTtsProvider, TtsProvider } from './TtsProviderFactory';

export interface VoiceConfig {
  stt_enabled: boolean;
  tts_enabled: boolean;
  stt_provider: string;
  tts_provider: string;
  language: string;
  tts_voice: string;
  tts_speed: number;
}

export class VoiceService {
  private sttProvider: SttProvider;
  private ttsProvider: TtsProvider;
  private config: VoiceConfig;
  private listening = false;

  constructor(config: VoiceConfig) {
    this.config = config;
    this.sttProvider = createSttProvider(config.stt_provider);
    this.ttsProvider = createTtsProvider(config.tts_provider);
  }

  startListening(onChunk: (text: string) => void): void {
    if (!this.config.stt_enabled) return;

    this.sttProvider.startListening((text, isFinal) => {
      if (isFinal) {
        onChunk(text);
      }
    });
    this.listening = true;
  }

  stopListening(): void {
    this.sttProvider.stopListening();
    this.listening = false;
  }

  async speak(text: string): Promise<void> {
    if (!this.config.tts_enabled) return;
    await this.ttsProvider.speak(text);
  }

  stopSpeaking(): void {
    this.ttsProvider.stop();
  }

  isListening(): boolean {
    return this.listening;
  }

  updateConfig(config: VoiceConfig): void {
    this.config = config;
    this.sttProvider = createSttProvider(config.stt_provider);
    this.ttsProvider = createTtsProvider(config.tts_provider);
  }
}