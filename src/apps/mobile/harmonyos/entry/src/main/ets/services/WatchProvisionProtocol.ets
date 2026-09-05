/**
 * Wire format for the watch → phone → desktop device-provisioning handoff.
 *
 * The watch has no keyboard, so it cannot type an account password. Instead it
 * writes a provisioning request into the distributed KV store both apps share
 * (same bundle name, same store id), the phone asks its owner to confirm, and
 * the phone relays the request to the paired desktop over the already
 * encrypted room channel. The credential comes back through the same two keys.
 *
 * Only the credential itself is secret, and it is sealed to the watch's
 * ephemeral X25519 public key before it is ever written, so neither the relay
 * nor the KV replication layer sees the account master key in plaintext.
 * Errors are written unsealed on purpose: the watch must be able to render
 * them even when the failure was in key agreement.
 */

import { HarmonyUpgradeIdentityContract } from './HarmonyUpgradeIdentityContract';

export const WATCH_PROVISION_REQUEST_KEY: string = HarmonyUpgradeIdentityContract.WATCH_PROVISION_REQUEST_KEY;
export const WATCH_PROVISION_RESPONSE_KEY: string = HarmonyUpgradeIdentityContract.WATCH_PROVISION_RESPONSE_KEY;
export const WATCH_PROVISION_PROTOCOL_VERSION: number = 1;

/**
 * A request older than this is treated as abandoned. Without it, a request the
 * watch never got to clear would re-open the confirmation card on every phone
 * launch, long after whoever asked for it walked away.
 */
export const WATCH_PROVISION_REQUEST_TTL_MS: number = 5 * 60 * 1000;

export const WATCH_PROVISION_STATUS_OK: string = 'ok';
export const WATCH_PROVISION_STATUS_ERROR: string = 'error';

/** As read off the KV store, before any field has been trusted. */
export interface WatchProvisionRequestPayload {
  v?: number;
  request_id?: string;
  device_id?: string;
  device_name?: string;
  public_key?: string;
  created_ms?: number;
}

/** A request that passed every shape check the relay would apply later. */
export interface WatchProvisionRequest {
  requestId: string;
  deviceId: string;
  deviceName: string;
  publicKeyBase64: string;
  createdMs: number;
}

export interface WatchProvisionResponsePayload {
  v: number;
  request_id: string;
  status: string;
  message?: string;
  public_key?: string;
  nonce?: string;
  encrypted_data?: string;
}

/** The plaintext sealed into `encrypted_data` of a successful response. */
export interface WatchProvisionCredential {
  relay_url: string;
  token: string;
  user_id: string;
  master_key: string;
  device_id: string;
}

/** Longest device name we will forward; the watch picks it, so bound it. */
const MAX_DEVICE_NAME_LENGTH: number = 64;

export class WatchProvisionProtocol {
  /**
   * Parse and fully validate a request. Returns `undefined` for anything we
   * would not be able to act on, because a malformed request has no usable
   * `request_id` to answer and silently ignoring it is the only option.
   *
   * The `device_id` rule mirrors the relay's exactly (32 lowercase hex): a
   * request that would be refused at the far end is refused here, before the
   * owner is asked to approve something that cannot succeed.
   */
  static parseRequest(payload: string): WatchProvisionRequest | undefined {
    const text = payload.trim();
    if (text.length === 0 || !text.startsWith('{')) {
      return undefined;
    }
    let value: WatchProvisionRequestPayload;
    try {
      value = JSON.parse(text) as WatchProvisionRequestPayload;
    } catch (_err) {
      return undefined;
    }
    if ((value.v || 0) !== WATCH_PROVISION_PROTOCOL_VERSION) {
      return undefined;
    }
    const requestId = (value.request_id || '').trim();
    const deviceId = (value.device_id || '').trim();
    const deviceName = (value.device_name || '').trim();
    const publicKey = (value.public_key || '').trim();
    if (!WatchProvisionProtocol.isUuid(requestId) || !WatchProvisionProtocol.isRelayDeviceId(deviceId)) {
      return undefined;
    }
    if (deviceName.length === 0 || deviceName.length > MAX_DEVICE_NAME_LENGTH || publicKey.length === 0) {
      return undefined;
    }
    return {
      requestId,
      deviceId,
      deviceName,
      publicKeyBase64: publicKey,
      createdMs: value.created_ms || 0
    };
  }

  /**
   * A request is stale once it is older than the TTL. A watch clock running
   * ahead of the phone yields a negative age and is accepted: clock skew
   * between two devices on one account is not the user's problem to solve.
   */
  static isExpired(request: WatchProvisionRequest, nowMs: number): boolean {
    if (request.createdMs <= 0) {
      return true;
    }
    return nowMs - request.createdMs > WATCH_PROVISION_REQUEST_TTL_MS;
  }

  static successResponse(
    requestId: string,
    phonePublicKeyBase64: string,
    encryptedData: string,
    nonce: string
  ): string {
    const payload: WatchProvisionResponsePayload = {
      v: WATCH_PROVISION_PROTOCOL_VERSION,
      request_id: requestId,
      status: WATCH_PROVISION_STATUS_OK,
      public_key: phonePublicKeyBase64,
      encrypted_data: encryptedData,
      nonce
    };
    return JSON.stringify(payload);
  }

  static errorResponse(requestId: string, message: string): string {
    const payload: WatchProvisionResponsePayload = {
      v: WATCH_PROVISION_PROTOCOL_VERSION,
      request_id: requestId,
      status: WATCH_PROVISION_STATUS_ERROR,
      message
    };
    return JSON.stringify(payload);
  }

  /** The relay rejects any `device_id` that is not 32 lowercase hex digits. */
  static isRelayDeviceId(value: string): boolean {
    return /^[0-9a-f]{32}$/.test(value);
  }

  /** The desktop parses `request_id` as a UUID before it calls the relay. */
  static isUuid(value: string): boolean {
    return /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(value);
  }

  /** Short form for the confirmation card; the full id means nothing to a reader. */
  static shortDeviceId(deviceId: string): string {
    return deviceId.length <= 8 ? deviceId : deviceId.slice(0, 4) + '…' + deviceId.slice(deviceId.length - 4);
  }
}
