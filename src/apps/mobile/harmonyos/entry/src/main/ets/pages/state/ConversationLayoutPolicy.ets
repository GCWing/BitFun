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
  static readonly TABLET_DEVICE_TYPE: string = 'tablet';
  static readonly WIDE_LAYOUT_MIN_WIDTH: number = 720;
  static readonly FALLBACK_MASTER_PANE_WIDTH: number = 344;
  static readonly MIN_MASTER_PANE_WIDTH: number = 280;
  static readonly MIN_DETAIL_PANE_WIDTH: number = 360;
  static readonly EXTRA_WIDE_MIN_WIDTH: number = 1080;

  static useMasterDetail(
    viewportWidth: number,
    mediaQueryMatched: boolean,
    isFolded: boolean,
    deviceType: string,
    creases: ConversationLayoutCrease[]
  ): boolean {
    if (isFolded) {
      return false;
    }
    if (!ConversationLayoutPolicy.hasWideViewport(viewportWidth, mediaQueryMatched)) {
      return false;
    }
    const visibleCreases = ConversationLayoutPolicy.visibleCreases(viewportWidth, creases);
    if (visibleCreases.length > 0) {
      return visibleCreases.length >= 2;
    }
    return ConversationLayoutPolicy.isTabletDevice(deviceType);
  }

  static resolveWideGeometry(
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): ConversationLayoutGeometry {
    const visibleCreases = ConversationLayoutPolicy.visibleCreases(viewportWidth, creases);
    const firstCrease = visibleCreases.find((crease: ConversationLayoutCrease): boolean => {
      return crease.left >= ConversationLayoutPolicy.MIN_MASTER_PANE_WIDTH &&
        crease.left + crease.width <= viewportWidth - ConversationLayoutPolicy.MIN_DETAIL_PANE_WIDTH;
    });
    const masterPaneWidth = firstCrease ? firstCrease.left :
      ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH;
    const masterDetailGap = firstCrease ? Math.max(0, firstCrease.width) : 0;
    const detailStart = masterPaneWidth + masterDetailGap;
    const detailSegments = ConversationLayoutPolicy.detailSegments(viewportWidth, detailStart, visibleCreases);
    const contentSegment = ConversationLayoutPolicy.widestSegment(detailSegments);
    const collapsedContentSegment = ConversationLayoutPolicy.widestSegment(
      ConversationLayoutPolicy.detailSegments(viewportWidth, 0, visibleCreases)
    );
    return new ConversationLayoutGeometry(
      masterPaneWidth,
      masterDetailGap,
      visibleCreases.length > 1 || viewportWidth >= ConversationLayoutPolicy.EXTRA_WIDE_MIN_WIDTH,
      contentSegment ? contentSegment.left - detailStart : 0,
      contentSegment ? contentSegment.width : 0,
      collapsedContentSegment ? collapsedContentSegment.left : 0,
      collapsedContentSegment ? collapsedContentSegment.width : 0
    );
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

  private static hasWideViewport(viewportWidth: number, mediaQueryMatched: boolean): boolean {
    return mediaQueryMatched || viewportWidth >= ConversationLayoutPolicy.WIDE_LAYOUT_MIN_WIDTH;
  }

  private static isTabletDevice(deviceType: string): boolean {
    return deviceType.toLowerCase() === ConversationLayoutPolicy.TABLET_DEVICE_TYPE;
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
