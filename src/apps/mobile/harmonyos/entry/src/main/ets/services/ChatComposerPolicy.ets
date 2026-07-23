export class ChatComposerPolicy {
  static canSend(
    text: string,
    attachmentCount: number,
    isBusy: boolean,
    requiresRemoteConnection: boolean,
    connectionState: string
  ): boolean {
    const hasContent = text.trim().length > 0 || attachmentCount > 0;
    const remoteAvailable = !requiresRemoteConnection ||
      connectionState === 'connected' || connectionState === 'reconnecting';
    return hasContent && !isBusy && remoteAvailable;
  }

  static canUseVoice(text: string, attachmentCount: number, isBusy: boolean): boolean {
    return text.trim().length === 0 && attachmentCount === 0 && !isBusy;
  }
}
