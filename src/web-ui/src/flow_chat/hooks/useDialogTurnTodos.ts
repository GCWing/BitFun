/**
 * Subscribe to DialogTurn.todos for a given session + turn.
 * Returns the latest todos array, or empty array if unavailable.
 *
 * Uses FlowChatStore.subscribe() with shallow-diff to avoid
 * re-renders on unrelated state changes.
 */

import { useState, useEffect } from 'react';
import { flowChatStore } from '../store/FlowChatStore';
import type { TodoItem } from '../types/flow-chat';

function todosEqual(a: TodoItem[], b: TodoItem[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i].id !== b[i].id || a[i].content !== b[i].content || a[i].status !== b[i].status) {
      return false;
    }
  }
  return true;
}

export function useDialogTurnTodos(
  sessionId: string | undefined,
  turnId: string | undefined
): TodoItem[] {
  const [todos, setTodos] = useState<TodoItem[]>(() => {
    if (!sessionId || !turnId) return [];
    return flowChatStore.getDialogTurnTodos(sessionId, turnId);
  });

  useEffect(() => {
    if (!sessionId || !turnId) {
      setTodos([]);
      return;
    }

    // Initial read
    const initial = flowChatStore.getDialogTurnTodos(sessionId, turnId);
    setTodos(initial);

    const unsubscribe = flowChatStore.subscribe((state) => {
      const session = state.sessions.get(sessionId);
      if (!session) return;

      const turn = session.dialogTurns.find((t) => t.id === turnId);
      const nextTodos = turn?.todos ?? [];

      setTodos((prev) => {
        if (todosEqual(prev, nextTodos)) return prev;
        return nextTodos;
      });
    });

    return unsubscribe;
  }, [sessionId, turnId]);

  return todos;
}
