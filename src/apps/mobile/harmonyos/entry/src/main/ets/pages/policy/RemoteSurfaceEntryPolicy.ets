export class RemoteConnectEntryKind {
  static readonly EnterSurface: string = 'enter_surface';
  static readonly OpenConnect: string = 'open_connect';
  static readonly OpenScan: string = 'open_scan';
}

/**
 * Switching to Remote is a change of context. Opening the connect sheet is a
 * command to pair or pick a desktop. Those two must not share an entry point.
 */
export class RemoteSurfaceEntryPolicy {
  static shouldOpenConnectSheet(kind: string): boolean {
    return kind === RemoteConnectEntryKind.OpenConnect ||
      kind === RemoteConnectEntryKind.OpenScan;
  }
}
