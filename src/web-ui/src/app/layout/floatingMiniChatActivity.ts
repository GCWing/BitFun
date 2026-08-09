import type { Session } from '@/flow_chat/types/flow-chat';
import { isTransientTurnStatus } from '@/flow_chat/utils/dialogTurnStability';

/**
 * The floating entry remains active for the full non-terminal turn lifecycle,
 * including queueing and cancellation, rather than only while text streams.
 */
export function isFloatingMiniChatSessionExecuting(
  session: Pick<Session, 'dialogTurns'> | undefined,
): boolean {
  const lastTurn = session?.dialogTurns.at(-1);
  return Boolean(lastTurn && isTransientTurnStatus(lastTurn.status));
}
