/** Merges the active control target into the account-backed device directory. */
export class SidebarDeviceProjectionPolicy {
  static shouldPrependControlTarget(
    directoryDeviceIds: string[],
    controlTargetType: string,
    controlTargetDeviceId: string,
    connectionVisible: boolean,
    hasRemoteContent: boolean
  ): boolean {
    const targetId = controlTargetDeviceId.trim();
    if (controlTargetType === 'none' || targetId.length === 0) {
      return false;
    }
    if (directoryDeviceIds.some((deviceId: string): boolean => deviceId === targetId)) {
      return false;
    }
    return connectionVisible || hasRemoteContent;
  }
}
