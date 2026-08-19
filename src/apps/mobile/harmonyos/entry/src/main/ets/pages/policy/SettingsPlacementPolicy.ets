import { AdaptiveLayoutInput } from './AdaptiveLayoutInput';
import {
  ConversationLayoutCrease,
  ConversationLayoutHorizontalCrease,
  ConversationLayoutPolicy
} from './ConversationLayoutPolicy';

export enum SettingsPlacementMode {
  Bottom = 'bottom',
  Side = 'side',
  FoldOperate = 'fold_operate'
}

export enum SettingsSheetKind {
  Settings = 'settings',
  Connect = 'connect',
  SessionDetails = 'session_details',
  RemoteViewSettings = 'remote_view_settings'
}

export class SettingsPlacement {
  readonly mode: SettingsPlacementMode;
  readonly width: number;
  readonly height: number;
  readonly maxHeight: number;

  constructor(
    mode: SettingsPlacementMode,
    width: number,
    height: number,
    maxHeight: number
  ) {
    this.mode = mode;
    this.width = width;
    this.height = height;
    this.maxHeight = maxHeight;
  }
}

/**
 * Overlay-sheet placement from fold posture and creases, not a width boolean.
 * Does not import ArkUI types; callers assemble SheetOptions.
 */
export class SettingsPlacementPolicy {
  static readonly TABLET_MIN_WIDTH: number = 400;
  static readonly TABLET_MAX_WIDTH: number = 520;
  static readonly TABLET_WIDTH_RATIO: number = 0.32;
  static readonly LANDSCAPE_WIDTH_RATIO: number = 0.45;
  static readonly SIDE_MAX_HEIGHT: number = 760;
  static readonly SIDE_MIN_HEIGHT: number = 560;
  static readonly SIDE_VERTICAL_MARGIN: number = 80;
  static readonly SESSION_DETAILS_HEIGHT: number = 560;
  static readonly REMOTE_VIEW_SETTINGS_HEIGHT: number = 520;

  static compactBottom(kind: SettingsSheetKind = SettingsSheetKind.Settings): SettingsPlacement {
    return SettingsPlacementPolicy.bottomPlacement(kind);
  }

  static resolve(input: AdaptiveLayoutInput, kind: SettingsSheetKind): SettingsPlacement {
    if (input.isHoverOperate) {
      return SettingsPlacementPolicy.foldOperatePlacement(input, kind);
    }
    if (input.isFolded) {
      return SettingsPlacementPolicy.bottomPlacement(kind);
    }
    const usableCreases = SettingsPlacementPolicy.usableVerticalCreases(
      input.viewportWidth,
      input.verticalCreases
    );
    if (usableCreases.length > 0) {
      return SettingsPlacementPolicy.creaseSidePlacement(input, kind, usableCreases);
    }
    if (input.verticalCreases.length > 0) {
      return SettingsPlacementPolicy.tabletSidePlacement(input, kind);
    }
    if (SettingsPlacementPolicy.isHeightScarceLandscape(input)) {
      return SettingsPlacementPolicy.landscapeSidePlacement(input, kind);
    }
    if (SettingsPlacementPolicy.isWideViewport(input)) {
      return SettingsPlacementPolicy.tabletSidePlacement(input, kind);
    }
    return SettingsPlacementPolicy.bottomPlacement(kind);
  }

  static sheetIntersectsVerticalCrease(
    placement: SettingsPlacement,
    crease: ConversationLayoutCrease,
    viewportWidth: number
  ): boolean {
    if (placement.mode !== SettingsPlacementMode.Side || placement.width <= 0 || viewportWidth <= 0) {
      return false;
    }
    const sheetLeft = viewportWidth - placement.width;
    const creaseRight = crease.left + crease.width;
    return sheetLeft < creaseRight && viewportWidth > crease.left;
  }

  private static creaseSidePlacement(
    input: AdaptiveLayoutInput,
    kind: SettingsSheetKind,
    usableCreases: ConversationLayoutCrease[]
  ): SettingsPlacement {
    const crease = usableCreases[usableCreases.length - 1];
    const width = Math.max(0, input.viewportWidth - crease.left - crease.width);
    const height = SettingsPlacementPolicy.kindSideHeight(input.viewportHeight, kind);
    return new SettingsPlacement(SettingsPlacementMode.Side, width, height, 0);
  }

  private static tabletSidePlacement(
    input: AdaptiveLayoutInput,
    kind: SettingsSheetKind
  ): SettingsPlacement {
    const ratioWidth = Math.round(input.viewportWidth * SettingsPlacementPolicy.TABLET_WIDTH_RATIO);
    const width = Math.min(
      SettingsPlacementPolicy.TABLET_MAX_WIDTH,
      Math.max(SettingsPlacementPolicy.TABLET_MIN_WIDTH, ratioWidth)
    );
    const height = SettingsPlacementPolicy.kindSideHeight(input.viewportHeight, kind);
    return new SettingsPlacement(SettingsPlacementMode.Side, width, height, 0);
  }

  private static landscapeSidePlacement(
    input: AdaptiveLayoutInput,
    kind: SettingsSheetKind
  ): SettingsPlacement {
    const width = Math.round(input.viewportWidth * SettingsPlacementPolicy.LANDSCAPE_WIDTH_RATIO);
    const height = SettingsPlacementPolicy.kindSideHeight(input.viewportHeight, kind);
    return new SettingsPlacement(SettingsPlacementMode.Side, width, height, 0);
  }

  private static foldOperatePlacement(
    input: AdaptiveLayoutInput,
    kind: SettingsSheetKind
  ): SettingsPlacement {
    const maxHeight = SettingsPlacementPolicy.operateMaxHeight(input);
    const height = SettingsPlacementPolicy.kindOperateHeight(maxHeight, kind);
    return new SettingsPlacement(SettingsPlacementMode.FoldOperate, 0, height, maxHeight);
  }

  private static bottomPlacement(kind: SettingsSheetKind): SettingsPlacement {
    const height = kind === SettingsSheetKind.RemoteViewSettings ?
      SettingsPlacementPolicy.REMOTE_VIEW_SETTINGS_HEIGHT : 0;
    return new SettingsPlacement(SettingsPlacementMode.Bottom, 0, height, 0);
  }

  private static kindSideHeight(viewportHeight: number, kind: SettingsSheetKind): number {
    const computed = SettingsPlacementPolicy.sideHeight(viewportHeight);
    if (kind === SettingsSheetKind.SessionDetails) {
      return Math.min(SettingsPlacementPolicy.SESSION_DETAILS_HEIGHT, computed);
    }
    if (kind === SettingsSheetKind.RemoteViewSettings) {
      return Math.min(SettingsPlacementPolicy.REMOTE_VIEW_SETTINGS_HEIGHT, computed);
    }
    return computed;
  }

  private static kindOperateHeight(maxHeight: number, kind: SettingsSheetKind): number {
    if (kind === SettingsSheetKind.RemoteViewSettings) {
      return Math.min(SettingsPlacementPolicy.REMOTE_VIEW_SETTINGS_HEIGHT, maxHeight);
    }
    if (kind === SettingsSheetKind.SessionDetails) {
      return Math.min(SettingsPlacementPolicy.SESSION_DETAILS_HEIGHT, maxHeight);
    }
    return maxHeight;
  }

  private static sideHeight(viewportHeight: number): number {
    if (viewportHeight <= 0) {
      return SettingsPlacementPolicy.SIDE_MAX_HEIGHT;
    }
    return Math.min(
      SettingsPlacementPolicy.SIDE_MAX_HEIGHT,
      Math.max(
        SettingsPlacementPolicy.SIDE_MIN_HEIGHT,
        Math.round(viewportHeight - SettingsPlacementPolicy.SIDE_VERTICAL_MARGIN)
      )
    );
  }

  private static operateMaxHeight(input: AdaptiveLayoutInput): number {
    const crease = SettingsPlacementPolicy.firstUsableHorizontalCrease(
      input.viewportHeight,
      input.horizontalCreases
    );
    if (crease) {
      return Math.max(0, input.viewportHeight - crease.top - crease.height);
    }
    return Math.max(0, Math.floor(input.viewportHeight / 2));
  }

  private static isWideViewport(input: AdaptiveLayoutInput): boolean {
    return input.wideLayoutMatched ||
      input.viewportWidth >= ConversationLayoutPolicy.MD_MIN_WIDTH;
  }

  private static isHeightScarceLandscape(input: AdaptiveLayoutInput): boolean {
    return input.viewportWidth > input.viewportHeight &&
      input.viewportHeight > 0 &&
      input.viewportHeight < ConversationLayoutPolicy.MD_MIN_WIDTH;
  }

  private static usableVerticalCreases(
    viewportWidth: number,
    creases: ConversationLayoutCrease[]
  ): ConversationLayoutCrease[] {
    return creases
      .filter((crease: ConversationLayoutCrease): boolean => {
        return crease.left > 0 &&
          crease.width >= 0 &&
          crease.left + crease.width < viewportWidth;
      })
      .sort((left: ConversationLayoutCrease, right: ConversationLayoutCrease): number => {
        return left.left - right.left;
      });
  }

  private static firstUsableHorizontalCrease(
    viewportHeight: number,
    creases: ConversationLayoutHorizontalCrease[]
  ): ConversationLayoutHorizontalCrease | undefined {
    const usable = creases
      .filter((crease: ConversationLayoutHorizontalCrease): boolean => {
        return crease.top > 0 &&
          crease.height >= 0 &&
          crease.top + crease.height < viewportHeight;
      })
      .sort((left: ConversationLayoutHorizontalCrease, right: ConversationLayoutHorizontalCrease): number => {
        return left.top - right.top;
      });
    return usable.length > 0 ? usable[0] : undefined;
  }
}
