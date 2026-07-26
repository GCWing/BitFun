import { RemoteI18n } from '../../i18n/RemoteI18n';

export enum GeneralChatServiceState {
  Unconfigured = 'unconfigured',
  Ready = 'ready',
  Sending = 'sending',
  Streaming = 'streaming',
  Offline = 'offline',
  AuthExpired = 'authExpired',
  Failed = 'failed'
}

export class GeneralChatServiceStatus {
  static fromConfiguration(apiUrl: string, modelName: string, hasApiKey: boolean): GeneralChatServiceState {
    return apiUrl.trim().length > 0 && modelName.trim().length > 0 && hasApiKey ?
      GeneralChatServiceState.Ready : GeneralChatServiceState.Unconfigured;
  }

  static fromError(message: string): GeneralChatServiceState {
    const normalized = message.trim().toLowerCase();
    if (message === RemoteI18n.t('generalChat.authenticationFailed') ||
      normalized.indexOf('authentication') >= 0 || normalized.indexOf('api key') >= 0) {
      return GeneralChatServiceState.AuthExpired;
    }
    if (normalized.indexOf('network') >= 0 || normalized.indexOf('dns') >= 0 ||
      normalized.indexOf('timeout') >= 0 || normalized.indexOf('socket') >= 0 ||
      normalized.indexOf('connect') >= 0) {
      return GeneralChatServiceState.Offline;
    }
    return GeneralChatServiceState.Failed;
  }

  static connectionState(state: GeneralChatServiceState): string {
    if (state === GeneralChatServiceState.Offline ||
      state === GeneralChatServiceState.AuthExpired || state === GeneralChatServiceState.Failed ||
      state === GeneralChatServiceState.Unconfigured) {
      return 'failed';
    }
    return 'connected';
  }

  static userMessage(state: GeneralChatServiceState, fallback: string = ''): string {
    if (state === GeneralChatServiceState.Unconfigured) {
      return RemoteI18n.t('generalChat.modelNotConfigured');
    }
    if (state === GeneralChatServiceState.Offline) {
      return RemoteI18n.t('generalChat.offline');
    }
    if (state === GeneralChatServiceState.AuthExpired) {
      return RemoteI18n.t('generalChat.authenticationFailed');
    }
    if (state === GeneralChatServiceState.Failed) {
      return fallback.length > 0 ? fallback : RemoteI18n.t('generalChat.serviceUnavailable');
    }
    if (state === GeneralChatServiceState.Sending) {
      return RemoteI18n.t('generalChat.sending');
    }
    if (state === GeneralChatServiceState.Streaming) {
      return RemoteI18n.t('generalChat.generating');
    }
    return RemoteI18n.t('generalChat.ready');
  }
}
