/**
 * Shared navigation events for FlowChat viewport movement and focus.
 */

export const FLOWCHAT_FOCUS_ITEM_EVENT = 'flowchat:focus-item';
export const FLOWCHAT_PIN_TURN_TO_TOP_EVENT = 'flowchat:pin-turn-to-top';

export type FlowChatFocusItemSource = 'btw-back';
export type FlowChatPinTurnToTopSource = 'send-message';
export type FlowChatPinTurnToTopMode = 'transient' | 'sticky-latest';

export interface FlowChatFocusItemRequest {
  sessionId: string;
  turnIndex?: number;
  itemId?: string;
  source?: FlowChatFocusItemSource;
}

export interface FlowChatPinTurnToTopRequest {
  sessionId: string;
  turnId: string;
  behavior?: ScrollBehavior;
  source?: FlowChatPinTurnToTopSource;
  pinMode?: FlowChatPinTurnToTopMode;
}

/**
 * Event for scrolling from the review action bar to a specific remediation
 * item in the CodeReviewToolCard report.
 */
export const DEEP_REVIEW_SCROLL_TO_EVENT = 'deep-review:scroll-to';

export interface DeepReviewScrollToRequest {
  groupId: string;
  groupIndex: number;
  /** Index of the best-matching issue in the report's `issues` array, or -1. */
  issueIndex: number;
}
