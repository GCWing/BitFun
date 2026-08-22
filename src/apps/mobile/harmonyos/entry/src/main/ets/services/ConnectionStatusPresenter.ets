import { RemoteI18n } from '../i18n/RemoteI18n';

export class ConnectionStatusPresenter {
  static labelKey(connectionState: string): string {
    if (connectionState === 'connected') {
      return 'connection.connected';
    }
    if (connectionState === 'reconnecting') {
      return 'connection.reconnecting';
    }
    if (connectionState === 'pairing' || connectionState === 'parsing') {
      return 'connection.connecting';
    }
    if (connectionState === 'failed') {
      return 'connection.error';
    }
    if (connectionState === 'disconnected') {
      return 'connection.disconnected';
    }
    return 'connection.waiting';
  }

  static label(connectionState: string): string {
    return RemoteI18n.t(ConnectionStatusPresenter.labelKey(connectionState));
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
