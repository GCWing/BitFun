import { ConversationLayoutCrease } from './ConversationLayoutPolicy';

export enum FilePreviewPlacement {
  Hidden = 'hidden',
  CompactFullPage = 'compact_full_page',
  WideFocusSplit = 'wide_focus_split',
  WideMasterPreviewFocus = 'wide_master_preview_focus',
  WideTriplePane = 'wide_triple_pane'
}

export class FilePreviewLayout {
  readonly placement: FilePreviewPlacement;
  readonly masterPaneWidth: number;
  readonly masterConversationGap: number;
  readonly conversationPaneWidth: number;
  readonly conversationPreviewGap: number;
  readonly previewPaneWidth: number;
  readonly masterConversationGapIsCrease: boolean;
  readonly conversationPreviewGapIsCrease: boolean;

  constructor(
    placement: FilePreviewPlacement,
    masterPaneWidth: number = 0,
    masterConversationGap: number = 0,
    conversationPaneWidth: number = 0,
    conversationPreviewGap: number = 0,
    previewPaneWidth: number = 0,
    masterConversationGapIsCrease: boolean = false,
    conversationPreviewGapIsCrease: boolean = false
  ) {
    this.placement = placement;
    this.masterPaneWidth = masterPaneWidth;
    this.masterConversationGap = masterConversationGap;
    this.conversationPaneWidth = conversationPaneWidth;
    this.conversationPreviewGap = conversationPreviewGap;
    this.previewPaneWidth = previewPaneWidth;
    this.masterConversationGapIsCrease = masterConversationGapIsCrease;
    this.conversationPreviewGapIsCrease = conversationPreviewGapIsCrease;
  }
}

export class FilePreviewPlacementPolicy {
  static readonly MIN_MASTER_WIDTH: number = 280;
  static readonly MIN_CONVERSATION_WIDTH: number = 360;
  static readonly MIN_PREVIEW_WIDTH: number = 360;
  // A physical tri-fold panel is a stronger boundary than the generic tablet
  // minimum. Current tri-fold right panels are about 290vp: that is still the
  // complete screen and must not make preview hide the master pane.
  static readonly MIN_CREASE_ALIGNED_PREVIEW_WIDTH: number = 280;
  static readonly PANE_DIVIDER_WIDTH: number = 1;

  static resolve(
    previewVisible: boolean,
    largeScreenLayout: boolean,
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): FilePreviewPlacement {
    return FilePreviewPlacementPolicy.resolveLayout(
      previewVisible,
      largeScreenLayout,
      viewportWidth,
      creases
    ).placement;
  }

  static resolveLayout(
    previewVisible: boolean,
    largeScreenLayout: boolean,
    viewportWidth: number,
    creases: ConversationLayoutCrease[],
    preferredMasterWidth: number = FilePreviewPlacementPolicy.MIN_MASTER_WIDTH,
    previewExpanded: boolean = false
  ): FilePreviewLayout {
    if (!previewVisible) {
      return new FilePreviewLayout(FilePreviewPlacement.Hidden);
    }
    if (!largeScreenLayout) {
      return new FilePreviewLayout(
        FilePreviewPlacement.CompactFullPage,
        0,
        0,
        Math.max(0, viewportWidth),
        0,
        Math.max(0, viewportWidth)
      );
    }
    if (previewExpanded) {
      const triFoldPreviewFocus = FilePreviewPlacementPolicy.creaseAlignedMasterPreviewFocus(
        viewportWidth,
        creases
      );
      if (triFoldPreviewFocus) {
        return triFoldPreviewFocus;
      }
    }
    const creaseLayout = FilePreviewPlacementPolicy.creaseAlignedTriplePane(viewportWidth, creases);
    if (creaseLayout) {
      return creaseLayout;
    }
    const creaseFocusLayout = FilePreviewPlacementPolicy.creaseAlignedFocusSplit(viewportWidth, creases);
    if (creaseFocusLayout) {
      return creaseFocusLayout;
    }
    const lastCreaseFocusLayout = FilePreviewPlacementPolicy.lastCreaseFocusSplit(viewportWidth, creases);
    if (lastCreaseFocusLayout) {
      return lastCreaseFocusLayout;
    }
    const flatLayout = FilePreviewPlacementPolicy.flatTriplePane(
      viewportWidth,
      creases,
      preferredMasterWidth
    );
    if (flatLayout) {
      return flatLayout;
    }
    const focusGap = FilePreviewPlacementPolicy.PANE_DIVIDER_WIDTH;
    const minimumFocusWidth = FilePreviewPlacementPolicy.MIN_CONVERSATION_WIDTH +
      FilePreviewPlacementPolicy.MIN_PREVIEW_WIDTH + focusGap;
    if (viewportWidth < minimumFocusWidth) {
      return new FilePreviewLayout(
        FilePreviewPlacement.CompactFullPage,
        0,
        0,
        Math.max(0, viewportWidth),
        0,
        Math.max(0, viewportWidth)
      );
    }
    const focusContentWidth = Math.max(0, viewportWidth - focusGap);
    const conversationWidth = Math.floor(focusContentWidth / 2);
    return new FilePreviewLayout(
      FilePreviewPlacement.WideFocusSplit,
      0,
      0,
      conversationWidth,
      focusGap,
      focusContentWidth - conversationWidth
    );
  }

  private static flatTriplePane(
    viewportWidth: number,
    creases: ConversationLayoutCrease[],
    preferredMasterWidth: number
  ): FilePreviewLayout | undefined {
    if (FilePreviewPlacementPolicy.visibleCreases(viewportWidth, creases).length > 0) {
      return undefined;
    }
    const minimum = FilePreviewPlacementPolicy.MIN_MASTER_WIDTH +
      FilePreviewPlacementPolicy.MIN_CONVERSATION_WIDTH +
      FilePreviewPlacementPolicy.MIN_PREVIEW_WIDTH +
      FilePreviewPlacementPolicy.PANE_DIVIDER_WIDTH * 2;
    if (viewportWidth < minimum) {
      return undefined;
    }
    const dividerWidth = FilePreviewPlacementPolicy.PANE_DIVIDER_WIDTH;
    const maximumMasterWidth = viewportWidth - dividerWidth * 2 -
      FilePreviewPlacementPolicy.MIN_CONVERSATION_WIDTH -
      FilePreviewPlacementPolicy.MIN_PREVIEW_WIDTH;
    const masterWidth = Math.max(
      FilePreviewPlacementPolicy.MIN_MASTER_WIDTH,
      Math.min(preferredMasterWidth, maximumMasterWidth)
    );
    const detailWidth = viewportWidth - masterWidth - dividerWidth * 2;
    const conversationWidth = Math.floor(detailWidth / 2);
    return new FilePreviewLayout(
      FilePreviewPlacement.WideTriplePane,
      masterWidth,
      dividerWidth,
      conversationWidth,
      dividerWidth,
      detailWidth - conversationWidth
    );
  }

  private static creaseAlignedFocusSplit(
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): FilePreviewLayout | undefined {
    const visible = FilePreviewPlacementPolicy.visibleCreases(viewportWidth, creases);
    if (visible.length !== 1) {
      return undefined;
    }
    const crease = visible[0];
    const conversationWidth = crease.left;
    const previewWidth = viewportWidth - crease.left - crease.width;
    if (conversationWidth < FilePreviewPlacementPolicy.MIN_MASTER_WIDTH ||
      previewWidth < FilePreviewPlacementPolicy.MIN_PREVIEW_WIDTH) {
      return undefined;
    }
    return new FilePreviewLayout(
      FilePreviewPlacement.WideFocusSplit,
      0,
      0,
      conversationWidth,
      crease.width,
      previewWidth,
      false,
      true
    );
  }

  private static lastCreaseFocusSplit(
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): FilePreviewLayout | undefined {
    const visible = FilePreviewPlacementPolicy.visibleCreases(viewportWidth, creases);
    if (visible.length < 2) {
      return undefined;
    }
    const crease = visible[visible.length - 1];
    const conversationWidth = crease.left;
    const previewWidth = viewportWidth - crease.left - crease.width;
    if (conversationWidth < FilePreviewPlacementPolicy.MIN_CONVERSATION_WIDTH ||
      previewWidth < FilePreviewPlacementPolicy.MIN_MASTER_WIDTH) {
      return undefined;
    }
    return new FilePreviewLayout(
      FilePreviewPlacement.WideFocusSplit,
      0,
      0,
      conversationWidth,
      crease.width,
      previewWidth,
      false,
      true
    );
  }

  private static creaseAlignedTriplePane(
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): FilePreviewLayout | undefined {
    const visible = FilePreviewPlacementPolicy.visibleCreases(viewportWidth, creases);
    if (visible.length < 2) {
      return undefined;
    }
    const first = visible[0];
    const second = visible[1];
    const masterWidth = first.left;
    const conversationWidth = second.left - first.left - first.width;
    const previewWidth = viewportWidth - second.left - second.width;
    if (masterWidth < FilePreviewPlacementPolicy.MIN_MASTER_WIDTH ||
      conversationWidth < FilePreviewPlacementPolicy.MIN_CONVERSATION_WIDTH ||
      previewWidth < FilePreviewPlacementPolicy.MIN_CREASE_ALIGNED_PREVIEW_WIDTH) {
      return undefined;
    }
    return new FilePreviewLayout(
      FilePreviewPlacement.WideTriplePane,
      masterWidth,
      first.width,
      conversationWidth,
      second.width,
      previewWidth,
      true,
      true
    );
  }

  /**
   * When requested by the user, preserve navigation on the first panel and
   * promote a narrow physical right-panel preview to the two-panel detail canvas.
   * This is deliberately limited to geometry where the middle conversation
   * panel was otherwise valid: malformed fold reports still use conservative
   * focus fallbacks instead of being mistaken for a tri-fold device.
   */
  private static creaseAlignedMasterPreviewFocus(
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): FilePreviewLayout | undefined {
    const visible = FilePreviewPlacementPolicy.visibleCreases(viewportWidth, creases);
    if (visible.length < 2) return undefined;
    const first = visible[0];
    const second = visible[1];
    const masterWidth = first.left;
    const middleWidth = second.left - first.left - first.width;
    const rightWidth = viewportWidth - second.left - second.width;
    const previewWidth = viewportWidth - first.left - first.width;
    if (masterWidth < FilePreviewPlacementPolicy.MIN_MASTER_WIDTH ||
      middleWidth < FilePreviewPlacementPolicy.MIN_CONVERSATION_WIDTH ||
      rightWidth < FilePreviewPlacementPolicy.MIN_CREASE_ALIGNED_PREVIEW_WIDTH ||
      rightWidth >= FilePreviewPlacementPolicy.MIN_PREVIEW_WIDTH) {
      return undefined;
    }
    return new FilePreviewLayout(
      FilePreviewPlacement.WideMasterPreviewFocus,
      masterWidth,
      first.width,
      0,
      0,
      previewWidth,
      true,
      false
    );
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
