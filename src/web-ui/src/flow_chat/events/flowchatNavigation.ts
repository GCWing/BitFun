/**
 * Shared navigation events for FlowChat viewport movement and focus.
 */

export const FLOWCHAT_FOCUS_ITEM_EVENT = 'flowchat:focus-item';

export type FlowChatFocusItemSource =
  | 'btw-back'
  | 'usage-report'
  | 'background-activity'
  | 'global-search';

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
 * A message has been submitted to a Session from the composer.
 *
 * The transcript may be showing a history window the reader navigated to, and
 * the Turn they just sent is not in it. Session growth alone cannot ask for
 * that window to be given up — a Turn arriving from anywhere else must leave a
 * reader where they are — so the submission says so itself.
 */
export const FLOWCHAT_MESSAGE_SUBMITTED_EVENT = 'flowchat:message-submitted';

export interface FlowChatMessageSubmittedRequest {
  sessionId: string;
}

/**
 * Turns have been rolled back out of a Session, and the transcript now ends
 * somewhere it did not a moment ago.
 *
 * The ledger cannot say this by itself. A shorter `dialogTurns` is also what a
 * history window re-cut and a hydration merge look like, and there are two
 * dozen writers of that array — so, as with a submission, the actor announces
 * it rather than leaving the viewport to infer an action from a count.
 */
export const FLOWCHAT_TURNS_ROLLED_BACK_EVENT = 'flowchat:turns-rolled-back';

export interface FlowChatTurnsRolledBackRequest {
  sessionId: string;
  /** First Turn index removed; everything from here on is gone. */
  fromTurnIndex: number;
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
