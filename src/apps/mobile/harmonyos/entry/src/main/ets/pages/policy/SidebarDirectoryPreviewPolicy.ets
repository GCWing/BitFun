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
   * A device header means the user just opened that desktop. Keep its
   * workspaces folded so the next tap opens one workspace's session preview.
   * A flattened unpaired row has no device tap, so workspaces start open.
   */
  static workspaceExpandedByDefault(deviceHeaderVisible: boolean): boolean {
    return !deviceHeaderVisible;
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
