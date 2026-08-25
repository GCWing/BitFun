/**
 * How many rows a sidebar section shows before "N more".
 * Each tap adds one batch instead of dumping the full device list.
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
}
