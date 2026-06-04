export interface TtsProvider {
  speak(text: string): Promise<void>;
  stop(): void;
  setVoice(voiceId: string): void;
}

export class EdgeTtsProvider implements TtsProvider {
  private currentVoice = 'zh-CN-XiaoxiaoNeural';

  async speak(text: string): Promise<void> {

    const utterance = new SpeechSynthesisUtterance(text);
    utterance.voice = speechSynthesis.getVoices().find(
      v => v.name === this.currentVoice || v.lang.startsWith('zh')
    ) || null;
    utterance.lang = 'zh-CN';
    utterance.rate = 1.0;

    return new Promise((resolve) => {
      utterance.onend = () => {
        resolve();
      };
      utterance.onerror = () => {
        resolve();
      };
      speechSynthesis.speak(utterance);
    });
  }

  stop(): void {
    speechSynthesis.cancel();
  }

  setVoice(voiceId: string): void {
    this.currentVoice = voiceId;
  }
}

export function createTtsProvider(provider: string): TtsProvider {
  switch (provider) {
    case 'edge':
      return new EdgeTtsProvider();
    default:
      return new EdgeTtsProvider();
  }
}