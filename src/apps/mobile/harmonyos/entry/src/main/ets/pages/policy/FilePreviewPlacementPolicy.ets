import { ConversationLayoutCrease } from './ConversationLayoutPolicy';

export enum FilePreviewPlacement {
  Hidden = 'hidden',
  CompactFullPage = 'compact_full_page',
  WideFocusSplit = 'wide_focus_split',
  WideTriplePane = 'wide_triple_pane'
}

export class FilePreviewLayout {
  readonly placement: FilePreviewPlacement;
  readonly masterPaneWidth: number;
  readonly masterConversationGap: number;
  readonly conversationPaneWidth: number;
  readonly conversationPreviewGap: number;
  readonly previewPaneWidth: number;

  constructor(
    placement: FilePreviewPlacement,
    masterPaneWidth: number = 0,
    masterConversationGap: number = 0,
    conversationPaneWidth: number = 0,
    conversationPreviewGap: number = 0,
    previewPaneWidth: number = 0
  ) {
    this.placement = placement;
    this.masterPaneWidth = masterPaneWidth;
    this.masterConversationGap = masterConversationGap;
    this.conversationPaneWidth = conversationPaneWidth;
    this.conversationPreviewGap = conversationPreviewGap;
    this.previewPaneWidth = previewPaneWidth;
  }
}

export class FilePreviewPlacementPolicy {
  static readonly MIN_MASTER_WIDTH: number = 280;
  static readonly MIN_CONVERSATION_WIDTH: number = 360;
  static readonly MIN_PREVIEW_WIDTH: number = 360;
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
    preferredMasterWidth: number = FilePreviewPlacementPolicy.MIN_MASTER_WIDTH
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
    const creaseLayout = FilePreviewPlacementPolicy.creaseAlignedTriplePane(viewportWidth, creases);
    if (creaseLayout) {
      return creaseLayout;
    }
    const creaseFocusLayout = FilePreviewPlacementPolicy.creaseAlignedFocusSplit(viewportWidth, creases);
    if (creaseFocusLayout) {
      return creaseFocusLayout;
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
    if (conversationWidth < FilePreviewPlacementPolicy.MIN_CONVERSATION_WIDTH ||
      previewWidth < FilePreviewPlacementPolicy.MIN_PREVIEW_WIDTH) {
      return undefined;
    }
    return new FilePreviewLayout(
      FilePreviewPlacement.WideFocusSplit,
      0,
      0,
      conversationWidth,
      crease.width,
      previewWidth
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
      previewWidth < FilePreviewPlacementPolicy.MIN_PREVIEW_WIDTH) {
      return undefined;
    }
    return new FilePreviewLayout(
      FilePreviewPlacement.WideTriplePane,
      masterWidth,
      first.width,
      conversationWidth,
      second.width,
      previewWidth
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
