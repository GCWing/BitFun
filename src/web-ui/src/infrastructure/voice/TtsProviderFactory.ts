import { createLogger } from '@/shared/utils/logger';

const log = createLogger('EdgeTtsProvider');

export interface TtsProvider {
  speak(text: string): Promise<void>;
  stop(): void;
  setVoice(voiceId: string): void;
  setVolume(volume: number): void;
  configure(voiceId: string, language: string): void;
}

export class EdgeTtsProvider implements TtsProvider {
  private voice = 'en-US-AriaNeural';
  private lang = 'en-US';
  private volume = 1.0;

  configure(voiceId: string, language: string): void {
    this.voice = voiceId || 'en-US-AriaNeural';
    this.lang = language || 'en-US';
  }

  private getVoice(): SpeechSynthesisVoice | null {
    const voices = speechSynthesis.getVoices();
    if (!voices.length) return null;
    return (
      voices.find(v => v.name === this.voice) ||
      voices.find(v => v.lang.startsWith(this.lang.slice(0, 2))) ||
      voices[0]
    );
  }

  async speak(text: string): Promise<void> {
    if (typeof speechSynthesis === 'undefined') {
      log.warn('SpeechSynthesis not available');
      return;
    }

    speechSynthesis.cancel();

    return new Promise((resolve) => {
      const utterance = new SpeechSynthesisUtterance(text);
      utterance.voice = this.getVoice();
      utterance.lang = this.lang;
      utterance.rate = 1.0;
      utterance.volume = this.volume;

      utterance.onstart = () => log.debug('TTS started');
      utterance.onend = () => resolve();
      utterance.onerror = (e) => {
        log.warn('TTS error', e.error);
        resolve();
      };
      utterance.onboundary = (e) => {
        log.debug('TTS boundary', { charIndex: e.charIndex });
      };

      speechSynthesis.speak(utterance);
    });
  }

  stop(): void {
    if (typeof speechSynthesis !== 'undefined') {
      speechSynthesis.cancel();
    }
  }

  setVoice(voiceId: string): void {
    this.voice = voiceId;
  }

  setVolume(volume: number): void {
    this.volume = Math.max(0, Math.min(1, volume));
  }
}

export function createTtsProvider(provider: string): TtsProvider {
  switch (provider) {
    case 'edge':
    default:
      return new EdgeTtsProvider();
  }
}