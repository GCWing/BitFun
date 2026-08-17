/**
 * Shared selection rules for the account-device list on Connect and Settings.
 * Online desktops can be selected. The last control target can still be tapped
 * to reconnect after the live link drops, even if presence already reads offline.
 */
export class AccountDeviceSelectionPolicy {
  static canSelectOnline(
    online: boolean,
    deviceId: string,
    phoneDeviceId: string,
    switchingDeviceId: string
  ): boolean {
    return online && deviceId !== phoneDeviceId && switchingDeviceId.length === 0;
  }

  static shouldReconnectLastTarget(
    deviceId: string,
    controlTargetDeviceId: string,
    connectionState: string
  ): boolean {
    return deviceId.length > 0 &&
      deviceId === controlTargetDeviceId &&
      connectionState !== 'connected';
  }

  static canActivate(
    online: boolean,
    deviceId: string,
    phoneDeviceId: string,
    controlTargetDeviceId: string,
    connectionState: string,
    switchingDeviceId: string
  ): boolean {
    if (switchingDeviceId.length > 0) {
      return false;
    }
    return AccountDeviceSelectionPolicy.canSelectOnline(online, deviceId, phoneDeviceId, switchingDeviceId) ||
      AccountDeviceSelectionPolicy.shouldReconnectLastTarget(deviceId, controlTargetDeviceId, connectionState);
  }

  static shouldShowConnectAction(
    online: boolean,
    deviceId: string,
    phoneDeviceId: string,
    controlTargetDeviceId: string,
    connectionState: string
  ): boolean {
    return AccountDeviceSelectionPolicy.canActivate(
      online, deviceId, phoneDeviceId, controlTargetDeviceId, connectionState, ''
    ) && !(deviceId === controlTargetDeviceId && connectionState === 'connected');
  }
}
