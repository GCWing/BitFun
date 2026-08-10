/**
 * Build the conversation hierarchy levels (L0..LN) around a session:
 * the ancestor chain from the root conversation down to the current session,
 * then all descendant child sessions (BFS, ordered by createdAt then depth).
 *
 * Kept outside of ChatInput.tsx so that file only exports components
 * (react-refresh/only-export-components).
 */

import type { Session } from '../types/flow-chat';
import { resolveSessionRelationship } from './sessionMetadata';

export interface ConversationLevelEntry {
  sessionId: string;
  level: number;
  session?: Session;
  isDescendant: boolean;
}

/**
 * Build the conversation hierarchy levels (L0..LN) around the current session:
 * the ancestor chain from the root conversation down to the current session,
 * then all descendant child sessions (BFS, ordered by createdAt then depth).
 */
export function buildConversationHierarchy(
  sessions: ReadonlyMap<string, Session>,
  currentSessionId?: string | null,
): ConversationLevelEntry[] {
  if (!currentSessionId) {
    return [];
  }

  const chain: Session[] = [];
  let cursorId: string | undefined = currentSessionId;
  let guard = 0;
  while (cursorId && guard++ < 128) {
    const session = sessions.get(cursorId);
    if (!session) {
      break;
    }
    chain.unshift(session);
    cursorId = resolveSessionRelationship(session).parentSessionId;
  }
  if (chain.length === 0) {
    return [];
  }

  const childrenByParent = new Map<string, Session[]>();
  for (const session of sessions.values()) {
    const parentId = resolveSessionRelationship(session).parentSessionId;
    if (!parentId) {
      continue;
    }
    const list = childrenByParent.get(parentId);
    if (list) {
      list.push(session);
    } else {
      childrenByParent.set(parentId, [session]);
    }
  }
  for (const list of childrenByParent.values()) {
    list.sort(
      (a, b) => (a.createdAt ?? 0) - (b.createdAt ?? 0) || (a.depth ?? 0) - (b.depth ?? 0),
    );
  }

  const descendants: Session[] = [];
  const queue: string[] = [currentSessionId];
  const visited = new Set<string>([currentSessionId]);
  while (queue.length > 0) {
    const parentId = queue.shift()!;
    for (const child of childrenByParent.get(parentId) ?? []) {
      if (visited.has(child.sessionId)) {
        continue;
      }
      visited.add(child.sessionId);
      descendants.push(child);
      queue.push(child.sessionId);
    }
  }

  const levels: ConversationLevelEntry[] = chain.map((session, index) => ({
    sessionId: session.sessionId,
    level: index,
    session,
    isDescendant: false,
  }));
  let nextLevel = chain.length;
  for (const session of descendants) {
    levels.push({
      sessionId: session.sessionId,
      level: nextLevel,
      session,
      isDescendant: true,
    });
    nextLevel += 1;
  }
  return levels;
}
