import type { QueuedMessage } from '../types/flow-chat';

type QueuedMessageAttachmentSource = Pick<
  QueuedMessage,
  'imageContexts' | 'imageDisplayData'
>;

/**
 * Returns the image attachments carried by one queued message.
 *
 * The transport payload is authoritative because it is what the Agent will
 * receive. Display data remains a compatibility fallback for older persisted
 * queue entries that did not retain the transport-side array.
 */
export function getQueuedMessageAttachmentCount(
  message: QueuedMessageAttachmentSource,
): number {
  const payloadCount = Array.isArray(message.imageContexts)
    ? message.imageContexts.length
    : 0;
  if (payloadCount > 0) {
    return payloadCount;
  }

  return Array.isArray(message.imageDisplayData)
    ? message.imageDisplayData.length
    : 0;
}
