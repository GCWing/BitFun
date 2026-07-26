import { RemoteI18n } from '../i18n/RemoteI18n';
import { RemoteDescriptor } from '../model/RemoteModels';
import { PairIdentity } from './RelayHttpClient';
import { RemoteDescriptorParser } from './RemoteDescriptorParser';

export interface RemotePairingProjection {
  requiresAccountAuth: boolean;
  accountUsername: string;
}

export interface RemotePairingReconnectState {
  autoReconnectAttempted: boolean;
  remoteUrl: string;
  userId: string;
  requiresAccountAuth: boolean;
}

export class RemotePairingPolicy {
  projection(remoteUrl: string): RemotePairingProjection {
    if (remoteUrl.trim().length === 0) {
      return {
        requiresAccountAuth: false,
        accountUsername: ''
      };
    }
    return {
      requiresAccountAuth: RemoteDescriptorParser.accountAuthRequired(remoteUrl),
      accountUsername: RemoteDescriptorParser.accountUsername(remoteUrl)
    };
  }

  userIdAfterProjection(currentUserId: string, deviceId: string, projection: RemotePairingProjection): string {
    if (!projection.requiresAccountAuth || projection.accountUsername.length === 0) {
      return currentUserId;
    }
    return this.isDefaultUserId(currentUserId, deviceId) ? projection.accountUsername : currentUserId;
  }

  shouldAutoReconnect(state: RemotePairingReconnectState): boolean {
    return !state.autoReconnectAttempted &&
      !state.requiresAccountAuth &&
      state.remoteUrl.trim().length > 0 &&
      state.userId.trim().length > 0;
  }

  identityForConnect(
    descriptor: RemoteDescriptor,
    currentUserId: string,
    deviceId: string,
    accountPassword: string,
    autoReconnect: boolean
  ): PairIdentity {
    if (descriptor.accountAuth) {
      if (autoReconnect) {
        throw new Error(RemoteI18n.t('errors.accountPasswordRequired'));
      }
      const username = currentUserId.trim() || descriptor.accountUsername.trim();
      if (username.length === 0) {
        throw new Error(RemoteI18n.t('errors.accountUsernameRequired'));
      }
      if (accountPassword.length === 0) {
        throw new Error(RemoteI18n.t('errors.accountPasswordRequired'));
      }
      return {
        userId: username,
        password: accountPassword
      };
    }

    return {
      userId: currentUserId.trim() || deviceId
    };
  }

  shouldPromptForAccount(descriptor: RemoteDescriptor): boolean {
    return descriptor.accountAuth;
  }

  private isDefaultUserId(userId: string, deviceId: string): boolean {
    const value = userId.trim();
    return value.length === 0 || value === deviceId || value.indexOf('harmony-') === 0;
  }
}
