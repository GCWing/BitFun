export interface SttProvider {
  startListening(onChunk: (text: string, isFinal: boolean) => void): void;
  stopListening(): void;
  isListening(): boolean;
}

export interface SpeechRecognitionEvent extends Event {
  resultIndex: number;
  results: SpeechRecognitionResultList;
}

export interface SpeechRecognitionResultList {
  length: number;
  item(index: number): SpeechRecognitionResult;
  [index: number]: SpeechRecognitionResult;
}

export interface SpeechRecognitionResult {
  isFinal: boolean;
  length: number;
  item(index: number): SpeechRecognitionAlternative;
  [index: number]: SpeechRecognitionAlternative;
}

export interface SpeechRecognitionAlternative {
  transcript: string;
  confidence: number;
}

export interface SpeechRecognitionErrorEvent extends Event {
  error: string;
}

interface SpeechRecognition extends EventTarget {
  continuous: boolean;
  interimResults: boolean;
  lang?: string;
  onresult: ((event: SpeechRecognitionEvent) => void) | null;
  onerror: ((event: SpeechRecognitionErrorEvent) => void) | null;
  start(): void;
  stop(): void;
}

export class WebSpeechSttProvider implements SttProvider {
  private recognition: SpeechRecognition | null = null;
  private listening = false;

  constructor() {
    if ('webkitSpeechRecognition' in window) {
      const rec = new (window as any).webkitSpeechRecognition();
      rec.continuous = true;
      rec.interimResults = true;
      this.recognition = rec;
    }
  }

  startListening(onChunk: (text: string, isFinal: boolean) => void): void {
    if (!this.recognition) {
      console.error('Speech recognition not supported');
      return;
    }
    this.listening = true;

    this.recognition.onresult = (event: SpeechRecognitionEvent) => {
      for (let i = event.resultIndex; i < event.results.length; i++) {
        const result = event.results[i];
        onChunk(result[0].transcript, result.isFinal);
      }
    };

    this.recognition.onerror = (event: SpeechRecognitionErrorEvent) => {
      console.error('Speech recognition error:', event.error);
      this.listening = false;
    };

    this.recognition.start();
  }

  stopListening(): void {
    if (this.recognition) {
      this.recognition.stop();
      this.listening = false;
    }
  }

  isListening(): boolean {
    return this.listening;
  }
}

export function createSttProvider(provider: string): SttProvider {
  switch (provider) {
    case 'webspeech':
      return new WebSpeechSttProvider();
    default:
      return new WebSpeechSttProvider();
  }
}