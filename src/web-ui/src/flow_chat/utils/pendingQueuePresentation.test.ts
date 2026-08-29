import { describe, expect, it } from 'vitest';
import { getQueuedMessageAttachmentCount } from './pendingQueuePresentation';

describe('getQueuedMessageAttachmentCount', () => {
  it('counts the image payload on the individual queued message', () => {
    expect(getQueuedMessageAttachmentCount({
      imageContexts: [{ id: 'one' }, { id: 'two' }, { id: 'three' }],
      imageDisplayData: [{ id: 'one' }, { id: 'two' }, { id: 'three' }],
    })).toBe(3);
  });

  it('does not confuse the attachment count with queue position or queue length', () => {
    expect(getQueuedMessageAttachmentCount({
      imageContexts: [{ id: 'only-attachment' }],
      imageDisplayData: undefined,
    })).toBe(1);
  });

  it('falls back to legacy display data when the transport payload is absent', () => {
    expect(getQueuedMessageAttachmentCount({
      imageContexts: undefined,
      imageDisplayData: [{ id: 'one' }, { id: 'two' }],
    })).toBe(2);
  });

  it('returns zero when the message has no image attachments', () => {
    expect(getQueuedMessageAttachmentCount({
      imageContexts: undefined,
      imageDisplayData: undefined,
    })).toBe(0);
  });
});
