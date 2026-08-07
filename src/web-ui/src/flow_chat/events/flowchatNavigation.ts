/**
 * Shared navigation events for FlowChat viewport movement and focus.
 */

export const FLOWCHAT_FOCUS_ITEM_EVENT = 'flowchat:focus-item';

export type FlowChatFocusItemSource = 'btw-back' | 'usage-report' | 'background-activity';

export interface FlowChatFocusItemRequest {
  sessionId: string;
  /** Stable Turn identity. When present, this is authoritative over `turnIndex`. */
  turnId?: string;
  /** Absolute, one-based visible Turn index within the Session. */
  turnIndex?: number;
  itemId?: string;
  source?: FlowChatFocusItemSource;
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
