/**
 * Map a conversation hierarchy level entry to a human-readable Chinese label.
 *
 * The `level` field carried by ConversationLevelEntry is mixed-semantics:
 * - ancestor chain entries (isDescendant === false): real depth index, 0 = root
 * - descendant entries (isDescendant === true): BFS peer sequence number
 *
 * Rendering raw `L${level}` misleads: three sibling sessions show as
 * L1/L2/L3, which reads like a nested hierarchy. Ancestor chain entries map
 * to military ranks; descendant entries stay on the same rank (副官) with the
 * level value as a peer sequence number, so siblings are never misread as
 * deeper nesting.
 */

const ANCESTOR_RANK_LABELS = ['主会话', '副官', '上尉', '少尉'];

export interface ConversationLevelLabelEntry {
  level: number;
  isDescendant: boolean;
}

export function conversationLevelLabel(entry: ConversationLevelLabelEntry): string {
  if (entry.isDescendant) {
    return `副官 ${entry.level}`;
  }
  if (entry.level < 0 || entry.level >= ANCESTOR_RANK_LABELS.length) {
    // level >= 4 falls back to 士官; negative level is a caller bug, keep the
    // raw value visible instead of inventing a rank.
    return entry.level < 0 ? `L${entry.level}` : '士官';
  }
  return ANCESTOR_RANK_LABELS[entry.level];
}
