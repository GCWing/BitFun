import { abilityAccessCtrl, Context, Permissions } from '@kit.AbilityKit';
import { audio } from '@kit.AudioKit';
import { speechRecognizer } from '@kit.CoreSpeechKit';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { Encoding } from './Encoding';

export type VoiceInputState = 'idle' | 'listening';

export interface VoiceInputCallbacks {
  onText: (text: string, isFinal: boolean) => void;
  onState?: (state: VoiceInputState) => void;
  onError?: (message: string) => void;
}

const MICROPHONE_PERMISSION: Permissions = 'ohos.permission.MICROPHONE';
const AUDIO_CHUNK_BYTES = 1280;
const HALF_AUDIO_CHUNK_BYTES = 640;

export class VoiceInputService {
  private engine?: speechRecognizer.SpeechRecognitionEngine;
  private capturer?: audio.AudioCapturer;
  private sessionId: string = '';
  private listening: boolean = false;
  private reading: boolean = false;
  private callbacks?: VoiceInputCallbacks;

  isListening(): boolean {
    return this.listening;
  }

  async start(context: Context, callbacks: VoiceInputCallbacks): Promise<void> {
    await this.cancelInternal(false);
    this.callbacks = callbacks;
    try {
      await this.ensureMicrophonePermission(context);
      this.sessionId = Encoding.randomId('voice');
      this.engine = await speechRecognizer.createEngine({
        language: 'zh-CN',
        online: 1,
        extraParams: {
          locate: 'CN',
          recognizerMode: 'short'
        }
      });
      this.engine.setListener(this.recognitionListener());
      this.capturer = await this.createCapturer();
      this.engine.startListening({
        sessionId: this.sessionId,
        audioInfo: {
          audioType: 'pcm',
          sampleRate: 16000,
          soundChannel: 1,
          sampleBit: 16
        },
        extraParams: {
          recognitionMode: 1,
          vadBegin: 2000,
          vadEnd: 3000,
          maxAudioDuration: 55000
        }
      });
      await this.capturer.start();
      this.listening = true;
      this.reading = true;
      this.callbacks?.onState?.('listening');
      this.captureLoop(this.sessionId);
    } catch (err) {
      await this.cancelInternal(true);
      throw new Error(this.errorMessage(err));
    }
  }

  async stop(): Promise<void> {
    if (!this.listening && !this.reading) {
      return;
    }
    const engine = this.engine;
    const sessionId = this.sessionId;
    this.listening = false;
    this.reading = false;
    this.callbacks?.onState?.('idle');
    await this.releaseCapturer();
    if (engine && sessionId.length > 0) {
      try {
        engine.finish(sessionId);
      } catch (err) {
        this.releaseEngine();
        throw new Error(this.errorMessage(err));
      }
    }
  }

  async cancel(): Promise<void> {
    await this.cancelInternal(true);
  }

  private async ensureMicrophonePermission(context: Context): Promise<void> {
    const atManager = abilityAccessCtrl.createAtManager();
    const result = await atManager.requestPermissionsFromUser(context, [MICROPHONE_PERMISSION]);
    if (result.authResults.length === 0 ||
      result.authResults[0] !== abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
      throw new Error(RemoteI18n.t('errors.voicePermissionDenied'));
    }
  }

  private async createCapturer(): Promise<audio.AudioCapturer> {
    try {
      return await audio.createAudioCapturer(this.capturerOptions(audio.SourceType.SOURCE_TYPE_VOICE_RECOGNITION));
    } catch (_err) {
      return await audio.createAudioCapturer(this.capturerOptions(audio.SourceType.SOURCE_TYPE_MIC));
    }
  }

  private capturerOptions(source: audio.SourceType): audio.AudioCapturerOptions {
    return {
      streamInfo: {
        samplingRate: audio.AudioSamplingRate.SAMPLE_RATE_16000,
        channels: audio.AudioChannel.CHANNEL_1,
        sampleFormat: audio.AudioSampleFormat.SAMPLE_FORMAT_S16LE,
        encodingType: audio.AudioEncodingType.ENCODING_TYPE_RAW
      },
      capturerInfo: {
        source,
        capturerFlags: 0
      }
    };
  }

  private recognitionListener(): speechRecognizer.RecognitionListener {
    return {
      onStart: (_sessionId: string, _eventMessage: string): void => {
      },
      onEvent: (_sessionId: string, _eventCode: number, _eventMessage: string): void => {
      },
      onResult: (_sessionId: string, result: speechRecognizer.SpeechRecognitionResult): void => {
        const text = result.result.trim();
        if (text.length > 0) {
          this.callbacks?.onText(text, result.isFinal || result.isLast);
        }
        if (result.isLast) {
          this.releaseAfterRecognizer(true);
        }
      },
      onComplete: (_sessionId: string, _eventMessage: string): void => {
        this.releaseAfterRecognizer(true);
      },
      onError: (_sessionId: string, errorCode: number, errorMessage: string): void => {
        this.callbacks?.onError?.(errorMessage || RemoteI18n.f('errors.voiceInputFailed', `${errorCode}`));
        this.releaseAfterRecognizer(true);
      }
    };
  }

  private async captureLoop(sessionId: string): Promise<void> {
    while (this.reading && this.sessionId === sessionId) {
      const capturer = this.capturer;
      if (!capturer) {
        break;
      }
      try {
        const buffer = await capturer.read(AUDIO_CHUNK_BYTES, true);
        if (!this.reading || this.sessionId !== sessionId) {
          break;
        }
        this.writeAudioBuffer(sessionId, buffer);
      } catch (err) {
        if (this.reading) {
          this.callbacks?.onError?.(this.errorMessage(err));
          await this.cancelInternal(true);
        }
        break;
      }
    }
  }

  private writeAudioBuffer(sessionId: string, buffer: ArrayBuffer): void {
    const engine = this.engine;
    if (!engine || buffer.byteLength === 0) {
      return;
    }
    const data = new Uint8Array(buffer);
    let offset = 0;
    while (offset + AUDIO_CHUNK_BYTES <= data.length) {
      engine.writeAudio(sessionId, data.subarray(offset, offset + AUDIO_CHUNK_BYTES));
      offset += AUDIO_CHUNK_BYTES;
    }
    const remaining = data.length - offset;
    if (remaining === HALF_AUDIO_CHUNK_BYTES) {
      engine.writeAudio(sessionId, data.subarray(offset, offset + HALF_AUDIO_CHUNK_BYTES));
    }
  }

  private releaseAfterRecognizer(notifyState: boolean): void {
    this.listening = false;
    this.reading = false;
    if (notifyState) {
      this.callbacks?.onState?.('idle');
    }
    this.releaseCapturer();
    this.releaseEngine();
  }

  private async cancelInternal(notifyState: boolean): Promise<void> {
    const engine = this.engine;
    const sessionId = this.sessionId;
    this.listening = false;
    this.reading = false;
    if (notifyState) {
      this.callbacks?.onState?.('idle');
    }
    await this.releaseCapturer();
    if (engine && sessionId.length > 0) {
      try {
        engine.cancel(sessionId);
      } catch (_err) {
      }
    }
    this.releaseEngine();
  }

  private async releaseCapturer(): Promise<void> {
    const capturer = this.capturer;
    this.capturer = undefined;
    if (!capturer) {
      return;
    }
    try {
      await capturer.stop();
    } catch (_err) {
    }
    try {
      await capturer.release();
    } catch (_err) {
    }
  }

  private releaseEngine(): void {
    const engine = this.engine;
    this.engine = undefined;
    this.sessionId = '';
    if (!engine) {
      return;
    }
    try {
      engine.shutdown();
    } catch (_err) {
    }
  }

  private errorMessage(err: Object): string {
    if (err instanceof Error && err.message.length > 0) {
      return err.message;
    }
    const details = this.errorDetails(err);
    if (details.length > 0 && details !== '{}') {
      return RemoteI18n.f('errors.voiceInputFailed', details);
    }
    return RemoteI18n.t('errors.voiceInputUnavailable');
  }

  private errorDetails(err: Object): string {
    if (err instanceof Error) {
      return `${err.name}: ${err.message}`;
    }
    try {
      return JSON.stringify(err);
    } catch (_jsonErr) {
      return `${err}`;
    }
  }
}
