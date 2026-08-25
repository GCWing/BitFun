/** Presentation rules for connection actions in the signed-out sidebar. */
export class SidebarConnectionActionPolicy {
  static shouldShowScan(connectionState: string): boolean {
    return connectionState !== 'connected';
  }
}
