import { abilityAccessCtrl, Context, Permissions } from '@kit.AbilityKit';
import { audio } from '@kit.AudioKit';
import { speechRecognizer } from '@kit.CoreSpeechKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { hilog } from '@kit.PerformanceAnalysisKit';

const VOICE_DOMAIN: number = 0x0000;
const VOICE_TAG: string = 'BitFunVoiceInput';
const MICROPHONE_PERMISSION: Permissions = 'ohos.permission.MICROPHONE';
const AUDIO_CHUNK_BYTES = 1280;
const HALF_AUDIO_CHUNK_BYTES = 640;
const DEFAULT_LANGUAGE = 'zh-CN';
const DEFAULT_SAMPLE_RATE = 16000;
const DEFAULT_MAX_RECORDING_SECONDS = 60;
const ENGINE_VAD_BEGIN = 2000;
const ENGINE_VAD_END = 3000;
const ENGINE_MAX_AUDIO_DURATION = 55000;

interface VoiceStartRequest {
  language?: string;
  sampleRate?: number;
  maxRecordingSeconds?: number;
  modelId?: string;
}

interface VoiceSessionInfo {
  sessionId: string;
  modelId: string;
  language: string;
  sampleRate: number;
  maxRecordingSeconds: number;
}

interface VoiceTranscriptionResult {
  text: string;
  language: string;
  durationMs: number;
  audioDurationSeconds: number;
}

interface VoiceAppendRequest {
  sessionId: string;
  pcm16Base64: string;
}

interface VoiceAppendResult {
  receivedBytes: number;
  receivedSeconds: number;
  limitReached: boolean;
}

/**
 * HarmonyOS system speech input service.
 *
 * Wraps the on-device `speechRecognizer` engine and `AudioCapturer` into the
 * start/append/finish/cancel Promise contract used by the desktop speech_*
 * Tauri commands. The ArkTS side owns audio capture and recognition; the
 * frontend-supplied PCM chunks in `append` are intentionally discarded because
 * the system engine captures audio itself.
 */
export class VoiceInputService {
  private context?: Context;
  private engine?: speechRecognizer.SpeechRecognitionEngine;
  private capturer?: audio.AudioCapturer;
  private sessionId: string = '';
  private language: string = DEFAULT_LANGUAGE;
  private sampleRate: number = DEFAULT_SAMPLE_RATE;
  private maxRecordingSeconds: number = DEFAULT_MAX_RECORDING_SECONDS;
  private listening: boolean = false;
  private reading: boolean = false;
  private idCounter: number = 0;
  private startedAt: number = 0;
  private capturedSeconds: number = 0;
  private finalText: string = '';
  private finishedResult: string = '';
  private hasFinishedResult: boolean = false;
  private finishResolver: ((result: string) => void) | null = null;
  private finishRejecter: ((error: Error) => void) | null = null;

  async start(arg: string): Promise<string> {
    await this.cancelInternal(false);
    hilog.info(VOICE_DOMAIN, VOICE_TAG, 'start begin: arg=%{public}s', arg);
    const context = this.context;
    if (!context) {
      throw new Error('Voice input service is not bound to a context');
    }
    await ensureMicrophonePermission(context);
    const req: VoiceStartRequest = JSON.parse(arg);
    this.language = this.resolveLanguage(req.language);
    this.sampleRate = req.sampleRate ?? DEFAULT_SAMPLE_RATE;
    this.maxRecordingSeconds = req.maxRecordingSeconds ?? DEFAULT_MAX_RECORDING_SECONDS;
    const modelId: string = req.modelId ?? 'ohos-system';
    this.sessionId = this.newSessionId();
    this.finalText = '';
    this.finishedResult = '';
    this.hasFinishedResult = false;
    this.finishResolver = null;
    this.finishRejecter = null;
    this.capturedSeconds = 0;
    this.startedAt = Date.now();
    try {
      this.engine = await speechRecognizer.createEngine({
        language: this.language,
        online: 1,
        extraParams: {
          locate: 'CN',
          recognizerMode: 'short'
        }
      });
    this.engine.setListener(this.buildRecognitionListener());
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
          vadBegin: ENGINE_VAD_BEGIN,
          vadEnd: ENGINE_VAD_END,
          maxAudioDuration: ENGINE_MAX_AUDIO_DURATION
        }
      });
    await this.capturer.start();
    this.listening = true;
      this.reading = true;
      this.captureLoop(this.sessionId);
    } catch (err) {
      const failErr: Error = this.toError(err);
      hilog.error(VOICE_DOMAIN, VOICE_TAG, 'voice input start failed: %{public}s', failErr.message);
      await this.cancelInternal(true);
      throw failErr;
    }
    const info: VoiceSessionInfo = {
      sessionId: this.sessionId,
      modelId: modelId,
      language: this.language,
      sampleRate: this.sampleRate,
      maxRecordingSeconds: this.maxRecordingSeconds
    };
    return JSON.stringify(info);
  }

  async append(arg: string): Promise<string> {
    const req: VoiceAppendRequest = JSON.parse(arg);
    const receivedBytes: number = this.estimateBase64Bytes(req.pcm16Base64);
    const receivedSeconds: number = this.sampleRate > 0
      ? receivedBytes / (this.sampleRate * 2)
      : 0;
    this.capturedSeconds += receivedSeconds;
    const limitReached: boolean = this.capturedSeconds >= this.maxRecordingSeconds;
    const result: VoiceAppendResult = {
      receivedBytes: receivedBytes,
      receivedSeconds: receivedSeconds,
      limitReached: limitReached
    };
    return JSON.stringify(result);
  }

  async finish(arg: string): Promise<string> {
    const sessionId: string = JSON.parse(arg).sessionId as string;
    if (this.hasFinishedResult) {
      const prebuilt: string = this.finishedResult;
      this.hasFinishedResult = false;
      this.finishedResult = '';
      await this.releaseCapturer();
      this.releaseEngine();
      return prebuilt;
    }
    const engine = this.engine;
    if (!engine || sessionId !== this.sessionId) {
      throw new Error('Voice input session is not active');
    }
    await this.releaseCapturer();
    try {
      engine.finish(this.sessionId);
    } catch (err) {
      const failErr: Error = this.toError(err);
      hilog.error(VOICE_DOMAIN, VOICE_TAG, 'voice input finish failed: %{public}s', failErr.message);
      this.releaseEngine();
      throw failErr;
    }
    const resultJson: string = await new Promise<string>((resolve, reject): void => {
      this.finishResolver = resolve;
      this.finishRejecter = reject;
    });
    this.releaseEngine();
    return resultJson;
  }

  async cancel(_arg: string): Promise<string> {
    await this.cancelInternal(false);
    return '';
  }

  private buildRecognitionListener(): speechRecognizer.RecognitionListener {
    return {
      onStart: (_sessionId: string, _eventMessage: string): void => {
      },
      onEvent: (_sessionId: string, _eventCode: number, _eventMessage: string): void => {
      },
      onResult: (_sessionId: string, result: speechRecognizer.SpeechRecognitionResult): void => {
        const text: string = result.result.trim();
        if (text.length > 0) {
          this.finalText = text;
        }
        if (result.isLast) {
          this.resolveFinish();
        }
      },
      onComplete: (_sessionId: string, _eventMessage: string): void => {
        this.resolveFinish();
      },
      onError: (_sessionId: string, errorCode: number, errorMessage: string): void => {
        const error: Error = new Error(errorMessage.length > 0
          ? errorMessage
          : `Voice input failed (${errorCode})`);
        this.listening = false;
        this.reading = false;
        if (this.finishRejecter) {
          const rejecter = this.finishRejecter;
          this.finishRejecter = null;
          this.finishResolver = null;
          rejecter(error);
        } else {
          this.hasFinishedResult = true;
          this.finishedResult = '';
        }
        this.releaseCapturer();
        this.releaseEngine();
      }
    };
  }

  private resolveFinish(): void {
    this.listening = false;
    this.reading = false;
    const result: VoiceTranscriptionResult = {
      text: this.finalText,
      language: this.language,
      durationMs: Date.now() - this.startedAt,
      audioDurationSeconds: this.capturedSeconds
    };
    const resultJson: string = JSON.stringify(result);
    this.finishedResult = resultJson;
    this.hasFinishedResult = true;
    if (this.finishResolver) {
      const resolver = this.finishResolver;
      this.finishResolver = null;
      this.finishRejecter = null;
      resolver(resultJson);
    }
    this.releaseCapturer();
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
        this.reading = false;
        if (this.finishRejecter) {
          const rejecter = this.finishRejecter;
          this.finishRejecter = null;
          this.finishResolver = null;
          rejecter(this.toError(err));
        }
        await this.cancelInternal(true);
        break;
      }
    }
  }

  private writeAudioBuffer(sessionId: string, buffer: ArrayBuffer): void {
    const engine = this.engine;
    if (!engine || buffer.byteLength === 0) {
      return;
    }
    const data: Uint8Array = new Uint8Array(buffer);
    let offset = 0;
    while (offset + AUDIO_CHUNK_BYTES <= data.length) {
      engine.writeAudio(sessionId, data.subarray(offset, offset + AUDIO_CHUNK_BYTES));
      offset += AUDIO_CHUNK_BYTES;
    }
    const remaining: number = data.length - offset;
    if (remaining === HALF_AUDIO_CHUNK_BYTES) {
      engine.writeAudio(sessionId, data.subarray(offset, offset + HALF_AUDIO_CHUNK_BYTES));
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
        source: source,
        capturerFlags: 0
      }
    };
  }

  private async cancelInternal(notifyError: boolean): Promise<void> {
    const engine = this.engine;
    const sessionId = this.sessionId;
    this.listening = false;
    this.reading = false;
    const rejecter = this.finishRejecter;
    this.finishResolver = null;
    this.finishRejecter = null;
    await this.releaseCapturer();
    if (engine && sessionId.length > 0) {
      try {
        engine.cancel(sessionId);
      } catch (_err) {
      }
    }
    this.releaseEngine();
    if (notifyError && rejecter) {
      rejecter(new Error('Voice input cancelled'));
    }
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

  setContext(context: Context): void {
    this.context = context;
  }

  /**
   * Map the frontend-provided language to a value accepted by the system
   * speechRecognizer. The engine rejects "auto" (and other non-BCP-47 values)
   * with code 1002200001 "Asr CreateEngineParams is wrong!", so normalize to
   * the default concrete language.
   */
  private resolveLanguage(language?: string): string {
    if (language === undefined || language === null || language.length === 0 || language === 'auto') {
      return DEFAULT_LANGUAGE;
    }
    return language;
  }

  private newSessionId(): string {
    this.idCounter += 1;
    return `voice-${Date.now()}-${this.idCounter}`;
  }

  private estimateBase64Bytes(base64: string): number {
    if (base64.length === 0) {
      return 0;
    }
    let padding = 0;
    if (base64.endsWith('==')) {
      padding = 2;
    } else if (base64.endsWith('=')) {
      padding = 1;
    }
    const decoded: number = Math.floor((base64.length * 3) / 4);
    return decoded > padding ? decoded - padding : 0;
  }

  private toError(err: Object): Error {
    if (err instanceof Error && err.message.length > 0) {
      return err;
    }
    const business = err as BusinessError;
    if (business && business.message && business.message.length > 0) {
      return new Error(business.message);
    }
    try {
      return new Error(JSON.stringify(err));
    } catch (_jsonErr) {
      return new Error(`${err}`);
    }
  }
}

/**
 * Request microphone permission from the user. Must be called from an
 * ability context (the EntryAbility registers the ArkTS bridge with the
 * ability context attached so the service can request permissions on demand).
 */
export async function ensureMicrophonePermission(context: Context): Promise<void> {
  const atManager = abilityAccessCtrl.createAtManager();
  const result = await atManager.requestPermissionsFromUser(context, [MICROPHONE_PERMISSION]);
  if (result.authResults.length === 0 ||
    result.authResults[0] !== abilityAccessCtrl.GrantStatus.PERMISSION_GRANTED) {
    throw new Error('Microphone permission denied');
  }
}
