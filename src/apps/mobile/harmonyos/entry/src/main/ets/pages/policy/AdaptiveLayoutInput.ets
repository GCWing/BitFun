import {
  ConversationLayoutCrease,
  ConversationLayoutHorizontalCrease
} from './ConversationLayoutPolicy';

/**
 * Device geometry for placement policies. Presentation constructs this once
 * and never injects it into components.
 */
export class AdaptiveLayoutInput {
  readonly viewportWidth: number;
  readonly viewportHeight: number;
  readonly foldStatus: number;
  readonly isFolded: boolean;
  readonly isExpandedFoldable: boolean;
  readonly isHoverOperate: boolean;
  readonly wideLayoutMatched: boolean;
  readonly verticalCreases: ConversationLayoutCrease[];
  readonly horizontalCreases: ConversationLayoutHorizontalCrease[];

  constructor(
    viewportWidth: number,
    viewportHeight: number,
    foldStatus: number,
    isFolded: boolean,
    isExpandedFoldable: boolean,
    isHoverOperate: boolean,
    wideLayoutMatched: boolean,
    verticalCreases: ConversationLayoutCrease[],
    horizontalCreases: ConversationLayoutHorizontalCrease[]
  ) {
    this.viewportWidth = viewportWidth;
    this.viewportHeight = viewportHeight;
    this.foldStatus = foldStatus;
    this.isFolded = isFolded;
    this.isExpandedFoldable = isExpandedFoldable;
    this.isHoverOperate = isHoverOperate;
    this.wideLayoutMatched = wideLayoutMatched;
    this.verticalCreases = verticalCreases;
    this.horizontalCreases = horizontalCreases;
  }

  static empty(): AdaptiveLayoutInput {
    return new AdaptiveLayoutInput(0, 0, 0, false, false, false, false, [], []);
  }
}
