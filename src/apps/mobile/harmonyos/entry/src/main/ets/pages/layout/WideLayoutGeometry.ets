import { FilePreviewLayout, FilePreviewPlacement } from '../policy/FilePreviewPlacementPolicy';

/** Pure geometry helpers shared by wide conversation presentation paths. */
export class WideLayoutGeometry {
  static masterPaneWidth(layout: FilePreviewLayout, fallback: number): number {
    return layout.placement === FilePreviewPlacement.WideTriplePane ? layout.masterPaneWidth : fallback;
  }

  static detailOffset(collapsed: boolean, expandedOffset: number, collapsedOffset: number): number {
    return collapsed ? collapsedOffset : expandedOffset;
  }

  static detailWidth(collapsed: boolean, expandedWidth: number, collapsedWidth: number): number {
    return collapsed ? collapsedWidth : expandedWidth;
  }

  static collapsedVisualBias(
    collapsed: boolean,
    collapsedOffset: number,
    collapsedWidth: number,
    maxContentWidth: number,
    maximumBias: number
  ): number {
    if (!collapsed || collapsedOffset > 0) {
      return 0;
    }
    const availableMargin = (collapsedWidth - maxContentWidth) / 2;
    return Math.min(maximumBias, Math.max(0, availableMargin));
  }

  static areaLength(value: Object): number {
    const parsed = Number.parseFloat(`${value}`);
    return Number.isNaN(parsed) ? 0 : parsed;
  }
}
