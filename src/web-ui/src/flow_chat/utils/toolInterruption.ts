import type { TFunction } from 'i18next';
import type { FlowToolItem } from '../types/flow-chat';

export function getToolInterruptionNote(
  toolItem: Pick<FlowToolItem, 'status' | 'interruptionReason'>,
  t: TFunction<'flow-chat'>,
): string | null {
  if (toolItem.status === 'cancelled' && toolItem.interruptionReason === 'app_restart') {
    return t('toolCards.common.interruptedByRestart');
  }

  return null;
}
