/**
 * Map a conversation hierarchy level entry to a human-readable label.
 *
 * The `level` field carried by ConversationLevelEntry is mixed-semantics:
 * - ancestor chain entries (isDescendant === false): real depth index, 0 = root
 * - descendant entries (isDescendant === true): BFS peer sequence number
 *
 * Rendering raw `L${level}` misleads: three sibling sessions show as
 * L1/L2/L3, which reads like a nested hierarchy. Ancestor chain entries use
 * the plain child-session label; descendant entries stay on the same label
 * with the level value as a peer sequence number, so siblings are never
 * misread as deeper nesting.
 *
 * Localization: labels are resolved through the required `t` function
 * (react-i18next TFunction, `flow-chat` namespace,
 * `chatInput.conversationLevel.*`). The fallback returns the i18n key itself
 * so this module never embeds localized user-facing literals; the key must
 * exist in every locale resource (enforced by i18n key-parity audits).
 */

export interface ConversationLevelLabelEntry {
  level: number;
  isDescendant: boolean;
}

export type ConversationLevelT = (key: string, options?: Record<string, unknown>) => string;

const MAIN_KEY = 'chatInput.conversationLevel.main';
const CHILD_KEY = 'chatInput.conversationLevel.child';
const SENIOR_KEY = 'chatInput.conversationLevel.senior';
const CHILD_WITH_SEQ_KEY = 'chatInput.conversationLevel.childWithSeq';

export function conversationLevelLabel(
  entry: ConversationLevelLabelEntry,
  t: ConversationLevelT,
): string {
  if (entry.isDescendant) {
    return t(CHILD_WITH_SEQ_KEY, { seq: entry.level });
  }
  if (entry.level < 0) {
    // Negative level is a caller bug, keep the raw value visible instead of
    // inventing a label.
    return `L${entry.level}`;
  }
  if (entry.level === 0) {
    return t(MAIN_KEY);
  }
  if (entry.level >= 4) {
    // Depth 4 and beyond collapses into the senior label.
    return t(SENIOR_KEY);
  }
  return t(CHILD_KEY);
}
