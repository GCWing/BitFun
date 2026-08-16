export class ConversationLayoutCrease {
  readonly left: number;
  readonly width: number;

  constructor(left: number, width: number) {
    this.left = left;
    this.width = width;
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
    const detailSegments = ConversationLayoutPolicy.detailSegments(viewportWidth, detailStart, visibleCreases);
    const contentSegment = ConversationLayoutPolicy.widestSegment(detailSegments);
    const collapsedContentSegment = ConversationLayoutPolicy.collapsedContentSegment(
      viewportWidth,
      visibleCreases
    );
    return new ConversationLayoutGeometry(
      masterPaneWidth,
      masterDetailGap,
      visibleCreases.length > 1 || viewportWidth >= ConversationLayoutPolicy.LG_MIN_WIDTH,
      contentSegment ? contentSegment.left - detailStart : 0,
      contentSegment ? contentSegment.width : 0,
      collapsedContentSegment.left,
      collapsedContentSegment.width
    );
  }

  private static collapsedContentSegment(
    viewportWidth: number,
    visibleCreases: ConversationLayoutCrease[]
  ): ConversationLayoutSegment {
    if (visibleCreases.length <= 1) {
      return new ConversationLayoutSegment(0, Math.max(0, viewportWidth));
    }
    const widest = ConversationLayoutPolicy.widestSegment(
      ConversationLayoutPolicy.detailSegments(viewportWidth, 0, visibleCreases)
    );
    return widest ? widest : new ConversationLayoutSegment(0, Math.max(0, viewportWidth));
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
