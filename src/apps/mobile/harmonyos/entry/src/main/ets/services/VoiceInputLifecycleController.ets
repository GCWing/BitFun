import { Context } from '@kit.AbilityKit';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from './ConnectionErrorPolicy';
import { VoiceInputCallbacks, VoiceInputService, VoiceInputState } from './VoiceInputService';

export interface VoiceInputRouteSnapshot {
  routeId: string;
  isListening: boolean;
  isBusy: boolean;
  inputText: string;
  selectedImageCount: number;
}

export interface VoiceInputLifecycleCallbacks {
  currentInputText: () => string;
  currentStatusText: () => string;
  onInputText: (routeId: string, text: string) => void;
  onListening: (routeId: string, isListening: boolean) => void;
  onStatusText: (statusText: string) => void;
  onError: (message: string) => void;
}

export class VoiceInputLifecycleController {
  private readonly service: VoiceInputService;
  private readonly callbacks: VoiceInputLifecycleCallbacks;

  constructor(service: VoiceInputService, callbacks: VoiceInputLifecycleCallbacks) {
    this.service = service;
    this.callbacks = callbacks;
  }

  async toggle(context: Context, snapshot: VoiceInputRouteSnapshot): Promise<boolean> {
    if (snapshot.isListening || this.service.isListening()) {
      return this.stop(snapshot, true);
    }
    if (snapshot.isBusy) {
      return false;
    }
    // Intermediate recognizer results replace one another. Merge every result
    // against the immutable draft captured at start so partial callbacks do
    // not duplicate speech and existing text or attachments never block a
    // user from continuing the prompt by voice.
    const baseDraft = snapshot.inputText;
    try {
      this.callbacks.onStatusText(RemoteI18n.t('status.voiceStarting'));
      await this.service.start(context, {
        onText: (text: string, isFinal: boolean): void => {
          this.callbacks.onInputText(snapshot.routeId, this.mergeDraft(baseDraft, text));
          if (isFinal) {
            this.callbacks.onStatusText(RemoteI18n.t('status.voiceRecognized'));
          }
        },
        onState: (state: VoiceInputState): void => {
          this.handleState(snapshot.routeId, state);
        },
        onError: (message: string): void => {
          this.callbacks.onListening(snapshot.routeId, false);
          this.callbacks.onError(message || RemoteI18n.t('errors.voiceInputUnavailable'));
        }
      });
      return true;
    } catch (err) {
      this.callbacks.onListening(snapshot.routeId, false);
      this.callbacks.onError(ConnectionErrorPolicy.errorText(err));
      return false;
    }
  }

  private mergeDraft(baseDraft: string, transcript: string): string {
    const spoken = transcript.trim();
    if (baseDraft.length === 0 || spoken.length === 0) {
      return baseDraft.length === 0 ? spoken : baseDraft;
    }
    const last = baseDraft.charAt(baseDraft.length - 1);
    const first = spoken.charAt(0);
    const needsSpace = !/\s/.test(last) && /[\x21-\x7E]/.test(last) && /[A-Za-z0-9]/.test(first);
    return `${baseDraft}${needsSpace ? ' ' : ''}${spoken}`;
  }

  async stop(snapshot: VoiceInputRouteSnapshot, showStatus: boolean): Promise<boolean> {
    if (!snapshot.isListening && !this.service.isListening()) {
      return false;
    }
    this.callbacks.onListening(snapshot.routeId, false);
    try {
      await this.service.stop();
      if (showStatus) {
        this.callbacks.onStatusText(this.callbacks.currentInputText().trim().length > 0 ?
          RemoteI18n.t('status.voiceRecognized') :
          RemoteI18n.t('status.voiceStopped'));
      }
      return true;
    } catch (err) {
      this.callbacks.onError(ConnectionErrorPolicy.errorText(err));
      return false;
    }
  }

  async cancel(routeId: string, clearAllListening: () => void): Promise<void> {
    await this.service.cancel();
    clearAllListening();
    this.callbacks.onListening(routeId, false);
  }

  private handleState(routeId: string, state: VoiceInputState): void {
    this.callbacks.onListening(routeId, state === 'listening');
    if (state === 'listening') {
      this.callbacks.onStatusText(RemoteI18n.t('status.voiceListening'));
      return;
    }
    const statusText = this.callbacks.currentStatusText();
    if (statusText === RemoteI18n.t('status.voiceListening') ||
      statusText === RemoteI18n.t('status.voiceStarting')) {
      this.callbacks.onStatusText(this.callbacks.currentInputText().trim().length > 0 ?
        RemoteI18n.t('status.voiceRecognized') :
        RemoteI18n.t('status.voiceStopped'));
    }
  }
}
