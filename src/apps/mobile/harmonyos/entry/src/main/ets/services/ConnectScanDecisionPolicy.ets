export class DetectedUrlAction {
  static readonly INVALID: string = 'invalid';
  static readonly PAIR_NOW: string = 'pair_now';
  static readonly PROMPT_ACCOUNT_PASSWORD: string = 'prompt_account_password';
  static readonly USE_CLOUD_DEVICE: string = 'use_cloud_device';
  static readonly SHOW_CLOUD_DEVICES: string = 'show_cloud_devices';
}

export class DetectedRemoteUrlResult {
  readonly action: string;
  readonly cloudDeviceId: string;

  constructor(action: string, cloudDeviceId: string = '') {
    this.action = action;
    this.cloudDeviceId = cloudDeviceId;
  }
}

/**
 * What a scanned pair URL should do next. Applying the URL and starting a
 * connect are separate steps: the detector never connects by itself.
 *
 * An OpenBitFun cloud session on this phone is already the account proof. The
 * account device directory is the authoritative membership check for a QR
 * target; display usernames are not, because aliases and restored legacy
 * records can legitimately differ. Reuse is still limited to the same relay,
 * then device selection verifies the scanned desktop id through the
 * authenticated directory instead of asking for the password based on a
 * string mismatch.
 */
export class ConnectScanDecisionPolicy {
  static decide(
    accountAuth: boolean,
    hasCloudAccountSession: boolean,
    _cloudUsername: string,
    _qrUsername: string,
    desktopId: string,
    cloudRelayUrl: string = '',
    qrRelayUrl: string = ''
  ): string {
    if (!accountAuth) {
      return DetectedUrlAction.PAIR_NOW;
    }
    if (hasCloudAccountSession &&
      ConnectScanDecisionPolicy.relayEndpointsCompatible(cloudRelayUrl, qrRelayUrl)) {
      return desktopId.trim().length > 0 ?
        DetectedUrlAction.USE_CLOUD_DEVICE :
        DetectedUrlAction.SHOW_CLOUD_DEVICES;
    }
    return DetectedUrlAction.PROMPT_ACCOUNT_PASSWORD;
  }

  static relayEndpointsCompatible(cloudRelayUrl: string, qrRelayUrl: string): boolean {
    const cloud = ConnectScanDecisionPolicy.normalizedRelayEndpoint(cloudRelayUrl);
    const qr = ConnectScanDecisionPolicy.normalizedRelayEndpoint(qrRelayUrl);
    return cloud.length > 0 && cloud === qr;
  }

  private static normalizedRelayEndpoint(value: string): string {
    let normalized = value.trim().toLowerCase();
    if (normalized.indexOf('wss://') === 0) {
      normalized = `https://${normalized.slice(6)}`;
    } else if (normalized.indexOf('ws://') === 0) {
      normalized = `http://${normalized.slice(5)}`;
    }
    while (normalized.length > 0 && normalized.charAt(normalized.length - 1) === '/') {
      normalized = normalized.slice(0, normalized.length - 1);
    }
    if (normalized.endsWith('/ws')) {
      normalized = normalized.slice(0, normalized.length - 3);
    }
    return normalized;
  }
}
