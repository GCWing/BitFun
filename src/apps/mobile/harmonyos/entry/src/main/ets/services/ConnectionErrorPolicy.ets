import { RemoteI18n } from '../i18n/RemoteI18n';
import { MobileIdentityStore } from './MobileIdentityStore';

const MAX_FAILED_USER_ID_ATTEMPTS = 3;
const USER_ID_LOCKOUT_MS = 60000;

export interface UserIdProtectionState {
  failureCount: number;
  lockUntil: number;
}

export interface ConnectionErrorResult {
  message: string;
  failureKind: string;
  failureCount: number;
  lockUntil: number;
  shouldShowRemoteUrlInput: boolean;
}

export class ConnectionErrorPolicy {
  static isLocked(lockUntil: number): boolean {
    return lockUntil > Date.now();
  }

  static remainingLockSeconds(lockUntil: number): number {
    if (!ConnectionErrorPolicy.isLocked(lockUntil)) {
      return 0;
    }
    return Math.max(1, Math.ceil((lockUntil - Date.now()) / 1000));
  }

  static protectedUserIdError(message: string): boolean {
    return message.indexOf('This remote URL is already protected') >= 0 ||
      message.indexOf('This mobile device must continue using the previously confirmed user ID') >= 0;
  }

  static async connectionErrorResult(
    err: Object,
    autoReconnect: boolean,
    protectionState: UserIdProtectionState,
    identityStore: MobileIdentityStore
  ): Promise<ConnectionErrorResult> {
    const raw = err instanceof Error ? err.message : JSON.stringify(err);
    let message = ConnectionErrorPolicy.errorText(err);
    let failureCount = protectionState.failureCount;
    let lockUntil = protectionState.lockUntil;

    if (!autoReconnect && ConnectionErrorPolicy.protectedUserIdError(raw)) {
      failureCount += 1;
      const shouldLock = failureCount >= MAX_FAILED_USER_ID_ATTEMPTS;
      lockUntil = shouldLock ? Date.now() + USER_ID_LOCKOUT_MS : 0;
      await identityStore.saveUserIdProtection(failureCount, lockUntil);
      if (shouldLock) {
        message = RemoteI18n.f('errors.tooManyAttempts', `${Math.ceil(USER_ID_LOCKOUT_MS / 1000)}`);
      }
    }

    const failureKind = ConnectionErrorPolicy.failureKindFor(message);
    return {
      message,
      failureKind,
      failureCount,
      lockUntil,
      shouldShowRemoteUrlInput: failureKind === 'expired_room' || failureKind === 'invalid_link'
    };
  }

  static failureKindFor(message: string): string {
    const text = message.toLowerCase();
    if (ConnectionErrorPolicy.protectedUserIdError(message) ||
      message.indexOf(RemoteI18n.t('errors.tooManyAttempts').split('{0}')[0]) >= 0 ||
      (message.indexOf('user id') >= 0 && message.indexOf('protected') >= 0)) {
      return 'protected_user';
    }
    if (message === RemoteI18n.t('errors.roomNotFound') ||
      message === RemoteI18n.t('errors.pairRejected') ||
      text.indexOf('room') >= 0 ||
      text.indexOf('qr') >= 0) {
      return 'expired_room';
    }
    if (message === RemoteI18n.t('errors.remoteUrlInvalid') ||
      message === RemoteI18n.t('errors.remoteUrlMissingParams') ||
      text.indexOf('malformed') >= 0 ||
      text.indexOf('invalid') >= 0) {
      return 'invalid_link';
    }
    if (message === RemoteI18n.t('errors.relayNetwork') ||
      message === RemoteI18n.t('errors.relayTimeout') ||
      message === RemoteI18n.t('errors.relayTimeoutDetail') ||
      text.indexOf('network') >= 0 || text.indexOf('dns') >= 0 || text.indexOf('timeout') >= 0) {
      return 'network';
    }
    return '';
  }

  static errorText(err: Object): string {
    const raw = err instanceof Error ? err.message : JSON.stringify(err);
    const text = raw.toLowerCase();
    if (raw === RemoteI18n.t('errors.remoteUrlRequired') ||
      raw === RemoteI18n.t('errors.remoteUrlMissingParams') ||
      raw === RemoteI18n.t('errors.userIdRequired')) {
      return raw;
    }
    if (text.indexOf('uri malformed') >= 0 || text.indexOf('malformed') >= 0 || text.indexOf('invalid') >= 0) {
      return RemoteI18n.t('errors.remoteUrlInvalid');
    }
    if (text.indexOf('permission') >= 0 || text.indexOf('denied') >= 0) {
      return RemoteI18n.t('errors.permissionDenied');
    }
    return raw || RemoteI18n.t('errors.operationFailed');
  }
}
