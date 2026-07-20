import { RemoteI18n } from '../i18n/RemoteI18n';

export class ConnectionStatusPresenter {
  static label(connectionState: string): string {
    if (connectionState === 'connected') {
      return RemoteI18n.t('connection.connected');
    }
    if (connectionState === 'reconnecting') {
      return RemoteI18n.t('connection.reconnecting');
    }
    if (connectionState === 'pairing' || connectionState === 'parsing') {
      return RemoteI18n.t('connection.connecting');
    }
    if (connectionState === 'failed') {
      return RemoteI18n.t('connection.error');
    }
    if (connectionState === 'disconnected') {
      return RemoteI18n.t('connection.disconnected');
    }
    return RemoteI18n.t('connection.waiting');
  }

  static detail(connectionState: string, statusText: string, desktopName: string, userId: string): string {
    const status = statusText || ConnectionStatusPresenter.label(connectionState);
    if (connectionState === 'connected') {
      const desktopSuffix = desktopName.length > 0 ? ` · ${desktopName}` : '';
      const userSuffix = userId.length > 0 ? ` · ${RemoteI18n.f('connection.user', userId)}` : '';
      return `${status || RemoteI18n.t('connection.connected')}${desktopSuffix}${userSuffix}`;
    }
    if (connectionState === 'reconnecting') {
      return statusText || RemoteI18n.t('connection.reconnectingDesktop');
    }
    if (connectionState === 'failed') {
      return statusText || RemoteI18n.t('connection.unavailableReconnect');
    }
    if (connectionState === 'disconnected') {
      return statusText || RemoteI18n.t('connection.desktopDisconnected');
    }
    return status;
  }

  static tone(connectionState: string): string {
    if (connectionState === 'connected') {
      return 'ok';
    }
    if (connectionState === 'reconnecting' || connectionState === 'pairing' || connectionState === 'parsing') {
      return 'busy';
    }
    if (connectionState === 'failed') {
      return 'error';
    }
    return 'muted';
  }
}
