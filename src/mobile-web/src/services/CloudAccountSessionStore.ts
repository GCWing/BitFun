import type { CloudAccountSession } from './CloudAccountClient';
import { fromB64, toB64 } from './E2EEncryption';
import { normalizeRelayUrl } from './pairingLink';

const ACCOUNT_SESSION_STORAGE_KEY = 'openbitfun.mobile.account_session.v1';
const ACCOUNT_SESSION_VERSION = 1;

interface PersistedAccountSessionV1 {
  version: 1;
  relay_url: string;
  username: string;
  token: string;
  user_id: string;
  master_key: string;
  controller_device_id: string;
}

/**
 * Account proof retained for the lifetime of the current browser tab.
 *
 * Passwords are never stored. The relay token and account master key are kept
 * in sessionStorage so a same-tab QR scan or reload can reuse an authenticated
 * account, while closing the tab still drops the browser-side credential.
 */
export interface StoredCloudAccountSession {
  relayUrl: string;
  username: string;
  controllerDeviceId: string;
  session: CloudAccountSession;
}

type StorageLike = Pick<Storage, 'getItem' | 'setItem'>;

function stringField(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function storageOrNull(): StorageLike | null {
  try {
    return typeof window === 'undefined' ? null : window.sessionStorage;
  } catch {
    return null;
  }
}

export function serializeCloudAccountSession(value: StoredCloudAccountSession): string {
  const record: PersistedAccountSessionV1 = {
    version: ACCOUNT_SESSION_VERSION,
    relay_url: value.relayUrl,
    username: value.username,
    token: value.session.token,
    user_id: value.session.userId,
    master_key: toB64(value.session.masterKey),
    controller_device_id: value.controllerDeviceId,
  };
  return JSON.stringify(record);
}

/**
 * Read the current shape and the short-lived unversioned camelCase shape used
 * by development builds. Unknown or incomplete records are ignored in place;
 * callers must not delete data merely because a newer build cannot read it.
 */
export function deserializeCloudAccountSession(raw: string): StoredCloudAccountSession | null {
  try {
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const version = parsed.version;
    if (version !== undefined && version !== ACCOUNT_SESSION_VERSION) return null;

    const relayUrl = normalizeRelayUrl(stringField(parsed.relay_url ?? parsed.relayUrl));
    const username = stringField(parsed.username);
    const token = stringField(parsed.token);
    const userId = stringField(parsed.user_id ?? parsed.userId);
    const masterKeyBase64 = stringField(parsed.master_key ?? parsed.masterKey);
    const controllerDeviceId = stringField(
      parsed.controller_device_id ?? parsed.controllerDeviceId,
    );
    if (!relayUrl || !token || !userId || !masterKeyBase64 || !controllerDeviceId) return null;

    const masterKey = fromB64(masterKeyBase64);
    if (masterKey.length !== 32) {
      masterKey.fill(0);
      return null;
    }
    return {
      relayUrl,
      username,
      controllerDeviceId,
      session: { token, userId, masterKey },
    };
  } catch {
    return null;
  }
}

export function saveCloudAccountSession(
  value: StoredCloudAccountSession,
  storage: StorageLike | null = storageOrNull(),
): void {
  if (!storage) return;
  try {
    storage.setItem(ACCOUNT_SESSION_STORAGE_KEY, serializeCloudAccountSession(value));
  } catch {
    // Private browsing and constrained webviews may reject browser storage.
  }
}

export function loadMatchingCloudAccountSession(
  relayUrl: string,
  qrUsername: string,
  controllerDeviceId: string,
  storage: StorageLike | null = storageOrNull(),
): StoredCloudAccountSession | null {
  if (!storage) return null;
  try {
    const raw = storage.getItem(ACCOUNT_SESSION_STORAGE_KEY);
    if (!raw) return null;
    const restored = deserializeCloudAccountSession(raw);
    if (!restored) return null;

    const normalizedRelayUrl = normalizeRelayUrl(relayUrl);
    if (!normalizedRelayUrl || restored.relayUrl !== normalizedRelayUrl) return null;
    if (restored.controllerDeviceId !== controllerDeviceId.trim()) return null;
    const expectedUsername = qrUsername.trim();
    if (expectedUsername && restored.username !== expectedUsername) return null;
    return restored;
  } catch {
    return null;
  }
}
