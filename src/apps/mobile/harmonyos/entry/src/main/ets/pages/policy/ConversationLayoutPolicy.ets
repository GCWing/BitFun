export class ConversationLayoutCrease {
  readonly left: number;
  readonly width: number;

  constructor(left: number, width: number) {
    this.left = left;
    this.width = width;
  }
}

export class ConversationLayoutRect {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;

  constructor(left: number, top: number, width: number, height: number) {
    this.left = left;
    this.top = top;
    this.width = width;
    this.height = height;
  }
}

export class ConversationLayoutGeometry {
  readonly masterPaneWidth: number;
  readonly masterDetailGap: number;
  readonly isExtraWide: boolean;
  readonly detailContentOffset: number;
  readonly detailContentWidth: number;
  readonly collapsedDetailContentOffset: number;
  readonly collapsedDetailContentWidth: number;

  constructor(
    masterPaneWidth: number,
    masterDetailGap: number,
    isExtraWide: boolean,
    detailContentOffset: number,
    detailContentWidth: number,
    collapsedDetailContentOffset: number,
    collapsedDetailContentWidth: number
  ) {
    this.masterPaneWidth = masterPaneWidth;
    this.masterDetailGap = masterDetailGap;
    this.isExtraWide = isExtraWide;
    this.detailContentOffset = detailContentOffset;
    this.detailContentWidth = detailContentWidth;
    this.collapsedDetailContentOffset = collapsedDetailContentOffset;
    this.collapsedDetailContentWidth = collapsedDetailContentWidth;
  }
}

class ConversationLayoutSegment {
  readonly left: number;
  readonly width: number;

  constructor(left: number, width: number) {
    this.left = left;
    this.width = width;
  }
}

export class ConversationLayoutPolicy {
  /** Official GridRow sm|md boundary: 320 / 600 / 840 / 1440 vp. */
  static readonly MD_MIN_WIDTH: number = 600;
  static readonly LG_MIN_WIDTH: number = 840;
  static readonly XL_MIN_WIDTH: number = 1440;
  static readonly WIDE_LAYOUT_MIN_WIDTH: number = 600;
  static readonly EXTRA_WIDE_MIN_WIDTH: number = 840;
  static readonly FALLBACK_MASTER_PANE_WIDTH: number = 344;
  static readonly MIN_MASTER_PANE_WIDTH: number = 280;
  static readonly MIN_DETAIL_PANE_WIDTH: number = 360;
  static readonly SYNTHETIC_HINGE_WIDTH: number = 16;
  static readonly FOLD_STATUS_UNKNOWN: number = 0;
  static readonly FOLD_STATUS_EXPANDED: number = 1;
  static readonly FOLD_STATUS_FOLDED: number = 2;
  static readonly FOLD_STATUS_HALF_FOLDED: number = 3;
  static readonly FOLD_STATUS_EXPANDED_WITH_SECOND_EXPANDED: number = 11;
  static readonly FOLD_STATUS_FOLDED_WITH_SECOND_EXPANDED: number = 12;
  static readonly FOLD_STATUS_HALF_FOLDED_WITH_SECOND_EXPANDED: number = 13;
  static readonly FOLD_STATUS_EXPANDED_WITH_SECOND_HALF_FOLDED: number = 21;
  static readonly FOLD_STATUS_FOLDED_WITH_SECOND_HALF_FOLDED: number = 22;
  static readonly FOLD_STATUS_HALF_FOLDED_WITH_SECOND_HALF_FOLDED: number = 23;

  static isFoldedStatus(status: number): boolean {
    return status === ConversationLayoutPolicy.FOLD_STATUS_FOLDED;
  }

  static isExpandedFoldableStatus(status: number): boolean {
    return status !== ConversationLayoutPolicy.FOLD_STATUS_UNKNOWN &&
      status !== ConversationLayoutPolicy.FOLD_STATUS_FOLDED;
  }

  static isHoverStatus(status: number): boolean {
    return status === ConversationLayoutPolicy.FOLD_STATUS_HALF_FOLDED ||
      status === ConversationLayoutPolicy.FOLD_STATUS_HALF_FOLDED_WITH_SECOND_HALF_FOLDED;
  }

  static useHoverOperate(
    status: number,
    viewportWidth: number,
    viewportHeight: number
  ): boolean {
    if (!ConversationLayoutPolicy.isHoverStatus(status)) {
      return false;
    }
    if (viewportWidth >= ConversationLayoutPolicy.MD_MIN_WIDTH &&
      viewportWidth >= viewportHeight) {
      return false;
    }
    return true;
  }

  static isLandscapeRotation(rotation: number): boolean {
    return rotation === 1 || rotation === 3 || rotation === 90 || rotation === 270;
  }

  static resolveLayoutViewportWidth(
    areaWidth: number,
    areaHeight: number,
    displayWidth: number,
    displayHeight: number,
    rotation: number = 0
  ): number {
    const landscape = ConversationLayoutPolicy.isLandscapeRotation(rotation) ||
      (displayWidth > 0 && displayWidth >= displayHeight);
    const layoutWidth = landscape ? Math.max(displayWidth, displayHeight) : displayWidth;
    if (layoutWidth > 0 && areaWidth > 0 && areaWidth < layoutWidth * 0.8) {
      return layoutWidth;
    }
    return areaWidth > 0 ? areaWidth : layoutWidth;
  }

  static verticalCreasesFromRects(
    viewportWidth: number,
    viewportHeight: number,
    rects: ConversationLayoutRect[]
  ): ConversationLayoutCrease[] {
    const mapped: ConversationLayoutCrease[] = [];
    rects.forEach((rect: ConversationLayoutRect): void => {
      if (rect.width < 0 || rect.height < 0) {
        return;
      }
      if (rect.height > rect.width) {
        mapped.push(new ConversationLayoutCrease(rect.left, rect.width));
        return;
      }
      if (viewportWidth <= 0 || viewportHeight <= 0) {
        return;
      }
      const spansViewportWidth = rect.width >= viewportWidth * 0.8;
      const spansViewportHeight = rect.width >= viewportHeight * 0.8;
      if (!spansViewportWidth && spansViewportHeight) {
        mapped.push(new ConversationLayoutCrease(rect.top, rect.height));
      }
    });
    return ConversationLayoutPolicy.visibleCreases(viewportWidth, mapped);
  }

  static useMasterDetail(
    viewportWidth: number,
    mediaQueryMatched: boolean,
    isFolded: boolean,
    creases: ConversationLayoutCrease[],
    isExpandedFoldable: boolean = false,
    isHover: boolean = false
  ): boolean {
    if (isFolded || isHover) {
      return false;
    }
    if (!ConversationLayoutPolicy.hasWideViewport(viewportWidth, mediaQueryMatched)) {
      return false;
    }
    const visibleCreases = ConversationLayoutPolicy.effectiveVerticalCreases(
      viewportWidth,
      creases,
      isExpandedFoldable
    );
    if (visibleCreases.length > 0) {
      return ConversationLayoutPolicy.canFitMasterDetail(viewportWidth, visibleCreases);
    }
    return ConversationLayoutPolicy.canFitMasterDetail(viewportWidth, []);
  }

  static resolveWideGeometry(
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): ConversationLayoutGeometry {
    const visibleCreases = ConversationLayoutPolicy.visibleCreases(viewportWidth, creases);
    const firstCrease = ConversationLayoutPolicy.firstUsableCrease(viewportWidth, visibleCreases);
    const masterPaneWidth = firstCrease ? firstCrease.left :
      ConversationLayoutPolicy.fallbackMasterPaneWidth(viewportWidth);
    const masterDetailGap = firstCrease ? Math.max(0, firstCrease.width) : 0;
    const detailStart = masterPaneWidth + masterDetailGap;
    const detailWidth = Math.max(0, viewportWidth - detailStart);
    const collapsedContentSegment = ConversationLayoutPolicy.collapsedContentSegment(
      viewportWidth,
      visibleCreases,
      detailStart,
      detailWidth
    );
    return new ConversationLayoutGeometry(
      masterPaneWidth,
      masterDetailGap,
      visibleCreases.length > 1 || viewportWidth >= ConversationLayoutPolicy.LG_MIN_WIDTH,
      0,
      detailWidth,
      collapsedContentSegment.left,
      collapsedContentSegment.width
    );
  }

  private static collapsedContentSegment(
    viewportWidth: number,
    visibleCreases: ConversationLayoutCrease[],
    detailStart: number,
    detailWidth: number
  ): ConversationLayoutSegment {
    if (visibleCreases.length === 0) {
      return new ConversationLayoutSegment(0, Math.max(0, viewportWidth));
    }
    if (visibleCreases.length === 1) {
      const widest = ConversationLayoutPolicy.widestSegment(
        ConversationLayoutPolicy.detailSegments(viewportWidth, 0, visibleCreases)
      );
      if (widest && widest.width <= viewportWidth * 0.55) {
        return new ConversationLayoutSegment(0, Math.max(0, viewportWidth));
      }
    }
    return new ConversationLayoutSegment(detailStart, detailWidth);
  }

  static effectiveVerticalCreases(
    viewportWidth: number,
    creases: ConversationLayoutCrease[],
    synthesizeCenterHinge: boolean
  ): ConversationLayoutCrease[] {
    const visibleCreases = ConversationLayoutPolicy.visibleCreases(viewportWidth, creases);
    if (visibleCreases.length > 0) {
      return visibleCreases;
    }
    if (!synthesizeCenterHinge) {
      return [];
    }
    const synthetic = ConversationLayoutPolicy.syntheticCenterCrease(viewportWidth);
    return synthetic ? [synthetic] : [];
  }

  static syntheticCenterCrease(viewportWidth: number): ConversationLayoutCrease | undefined {
    const hinge = ConversationLayoutPolicy.SYNTHETIC_HINGE_WIDTH;
    const minimum = ConversationLayoutPolicy.MIN_MASTER_PANE_WIDTH +
      ConversationLayoutPolicy.MIN_DETAIL_PANE_WIDTH + hinge;
    if (viewportWidth < minimum) {
      return undefined;
    }
    const centered = Math.floor((viewportWidth - hinge) / 2);
    const maximumMaster = viewportWidth - hinge - ConversationLayoutPolicy.MIN_DETAIL_PANE_WIDTH;
    const masterPaneWidth = Math.max(
      ConversationLayoutPolicy.MIN_MASTER_PANE_WIDTH,
      Math.min(centered, maximumMaster)
    );
    return new ConversationLayoutCrease(masterPaneWidth, hinge);
  }

  private static detailSegments(
    viewportWidth: number,
    detailStart: number,
    visibleCreases: ConversationLayoutCrease[]
  ): ConversationLayoutSegment[] {
    if (viewportWidth <= detailStart) {
      return [];
    }
    const detailCreases = visibleCreases.filter((crease: ConversationLayoutCrease): boolean => {
      return crease.left >= detailStart;
    });
    const segments: ConversationLayoutSegment[] = [];
    let segmentStart = detailStart;
    detailCreases.forEach((crease: ConversationLayoutCrease): void => {
      if (crease.left > segmentStart) {
        segments.push(new ConversationLayoutSegment(segmentStart, crease.left - segmentStart));
      }
      segmentStart = Math.max(segmentStart, crease.left + crease.width);
    });
    segments.push(new ConversationLayoutSegment(segmentStart, viewportWidth - segmentStart));
    return segments;
  }

  private static widestSegment(segments: ConversationLayoutSegment[]): ConversationLayoutSegment | undefined {
    return segments.reduce((widest: ConversationLayoutSegment | undefined, segment: ConversationLayoutSegment) => {
      if (!widest || segment.width >= widest.width) {
        return segment;
      }
      return widest;
    }, undefined);
  }

  private static canFitMasterDetail(
    viewportWidth: number,
    visibleCreases: ConversationLayoutCrease[]
  ): boolean {
    const firstCrease = ConversationLayoutPolicy.firstUsableCrease(viewportWidth, visibleCreases);
    const masterPaneWidth = firstCrease ? firstCrease.left :
      ConversationLayoutPolicy.fallbackMasterPaneWidth(viewportWidth);
    const masterDetailGap = firstCrease ? Math.max(0, firstCrease.width) : 0;
    return masterPaneWidth >= ConversationLayoutPolicy.MIN_MASTER_PANE_WIDTH &&
      viewportWidth - masterPaneWidth - masterDetailGap >= ConversationLayoutPolicy.MIN_DETAIL_PANE_WIDTH;
  }

  private static fallbackMasterPaneWidth(viewportWidth: number): number {
    const maximumMaster = viewportWidth - ConversationLayoutPolicy.MIN_DETAIL_PANE_WIDTH;
    if (maximumMaster < ConversationLayoutPolicy.MIN_MASTER_PANE_WIDTH) {
      return ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH;
    }
    return Math.min(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH, maximumMaster);
  }

  private static firstUsableCrease(
    viewportWidth: number,
    visibleCreases: ConversationLayoutCrease[]
  ): ConversationLayoutCrease | undefined {
    return visibleCreases.find((crease: ConversationLayoutCrease): boolean => {
      return crease.left >= ConversationLayoutPolicy.MIN_MASTER_PANE_WIDTH &&
        crease.left + crease.width <= viewportWidth - ConversationLayoutPolicy.MIN_DETAIL_PANE_WIDTH;
    });
  }

  private static hasWideViewport(viewportWidth: number, mediaQueryMatched: boolean): boolean {
    return mediaQueryMatched || viewportWidth >= ConversationLayoutPolicy.MD_MIN_WIDTH;
  }

  private static visibleCreases(
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): ConversationLayoutCrease[] {
    return creases
      .filter((crease: ConversationLayoutCrease): boolean => {
        return crease.left > 0 && crease.width >= 0 && crease.left + crease.width < viewportWidth;
      })
      .sort((left: ConversationLayoutCrease, right: ConversationLayoutCrease): number => left.left - right.left);
  }
}
