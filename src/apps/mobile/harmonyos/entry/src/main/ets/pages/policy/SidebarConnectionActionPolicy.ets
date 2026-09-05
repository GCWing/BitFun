/** Presentation rules for account-independent sidebar entry and navigation. */
export class SidebarConnectionActionPolicy {
  static shouldShowScan(connectionState: string): boolean {
    return connectionState !== 'connected';
  }

  /**
   * Account identity is optional once a remote control target exists. A QR
   * room therefore unlocks the same chat/settings shell as an account device,
   * including while that remembered target is temporarily reconnecting.
   */
  static shouldShowPrimaryNavigation(
    accountUserId: string,
    controlTargetType: string,
    connectionState: string
  ): boolean {
    return accountUserId.trim().length > 0 ||
      controlTargetType === 'room' ||
      controlTargetType === 'account_device' ||
      connectionState === 'connected' ||
      connectionState === 'reconnecting';
  }
}
