export type SessionModelSelectionTarget = {
  isTransient?: boolean;
  agentBackedTransient?: boolean;
  sessionKind?: string;
};

/** Whether the visible selector has a real runtime session to update. */
export function shouldSyncSessionModelSelection<T extends SessionModelSelectionTarget>(
  session: T | undefined,
): session is T {
  return Boolean(session && (!session.isTransient || session.agentBackedTransient));
}

/** Whether restoring the target requires access to internal runtime sessions. */
export function shouldIncludeInternalModelSession(
  session: SessionModelSelectionTarget | undefined,
): boolean {
  return Boolean(session?.sessionKind === 'subagent' || session?.agentBackedTransient);
}
