/**
 * Small helpers shared by driver implementations.
 */

import { generateTempTitle } from '../utils/titleUtils';
import type { FlowChatContext } from '../services/flow-chat-manager/types';

/**
 * Show a readable placeholder title immediately; the backend later confirms
 * the authoritative title via AI or local fallback generation.
 */
export function applyGeneratingTitlePlaceholder(
  context: FlowChatContext,
  sessionId: string,
  message: string,
): void {
  const tempTitle = generateTempTitle(message, 20);
  context.flowChatStore.updateSessionTitle(sessionId, tempTitle, 'generating');
}
