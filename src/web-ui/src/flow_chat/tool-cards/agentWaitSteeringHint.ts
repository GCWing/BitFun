import type { Session, ToolCardProps, ToolCardDisplayContext } from '../types/flow-chat';
import { isAcpFlowSession } from '../utils/acpSession';

export const RUNNING_STATUSES = new Set(['pending', 'preparing', 'running', 'streaming', 'receiving']);

export function shouldShowAgentWaitSteeringHint(
  status: ToolCardProps['toolItem']['status'],
  displayContext: ToolCardDisplayContext | undefined,
  session: Session | null | undefined,
): boolean {
  if (!RUNNING_STATUSES.has(status) || displayContext === 'subagent-projection' || !session) {
    return false;
  }

  return session.sessionKind !== 'subagent'
    && !session.parentToolCallId
    && !session.isHistorical
    && !isAcpFlowSession(session);
}
