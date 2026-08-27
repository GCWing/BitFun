/**
 * How many children a sidebar directory row shows before "N more".
 *
 * Device → workspaces and workspace → sessions use the same batch so
 * opening a group never dumps the full list. Each "more" tap adds one batch.
 */
export class SidebarDirectoryPreviewPolicy {
  static readonly PREVIEW_COUNT: number = 3;

  static visibleSlice<T>(items: T[], visibleCount: number): T[] {
    const limit = Math.max(0, visibleCount);
    return items.length <= limit ? items : items.slice(0, limit);
  }

  static hiddenCount(total: number, visibleCount: number): number {
    return Math.max(0, total - Math.max(0, visibleCount));
  }

  static nextVisibleCount(visibleCount: number, total: number): number {
    return Math.min(total, Math.max(0, visibleCount) + SidebarDirectoryPreviewPolicy.PREVIEW_COUNT);
  }

  /**
   * Session trees are lazy at every surface: a workspace starts folded and
   * its session RPC begins only after the user explicitly opens that row.
   */
  static workspaceExpandedByDefault(_deviceHeaderVisible: boolean): boolean {
    return false;
  }

  /**
   * ArkUI keeps a ForEach child's local state by key. Use the device plus a
   * canonical remote path so two devices never share disclosure state and a
   * harmless trailing slash does not destroy and recreate the row.
   */
  static workspaceKey(deviceId: string, path: string): string {
    let normalizedPath = path.trim();
    while (normalizedPath.length > 1 &&
      (normalizedPath.endsWith('/') || normalizedPath.endsWith('\\'))) {
      normalizedPath = normalizedPath.slice(0, normalizedPath.length - 1);
    }
    return `${deviceId.trim()}:${normalizedPath}`;
  }
}

export class SidebarPreviewCursor {
  readonly path: string;
  readonly visibleCount: number;

  constructor(path: string, visibleCount: number) {
    this.path = path;
    this.visibleCount = visibleCount;
  }
}
