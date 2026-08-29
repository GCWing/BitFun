import { describe, expect, it } from 'vitest';
import {
  canRestoreQueuedMessageToComposer,
  getQueuedMessageComposerDraft,
} from './pendingQueueDraft';

describe('canRestoreQueuedMessageToComposer', () => {
  const emptyComposer = {
    value: '',
    contexts: [],
    pendingLargePastes: {},
    queuedInput: null,
  };

  it('accepts an empty composer', () => {
    expect(canRestoreQueuedMessageToComposer(emptyComposer)).toBe(true);
  });

  it.each([
    { ...emptyComposer, value: 'draft' },
    { ...emptyComposer, value: ' ' },
    {
      ...emptyComposer,
      contexts: [{
        id: 'file-1',
        type: 'file' as const,
        timestamp: 1,
        filePath: '/workspace/file.ts',
        fileName: 'file.ts',
      }],
    },
    { ...emptyComposer, pendingLargePastes: { 'paste-1': 'content' } },
    { ...emptyComposer, queuedInput: 'failed submission' },
  ])('rejects an occupied composer without overwriting it', composer => {
    expect(canRestoreQueuedMessageToComposer(composer)).toBe(false);
  });
});

describe('getQueuedMessageComposerDraft', () => {
  it('restores the original composer snapshot without reusing mutable containers', () => {
    const contexts = [{
      id: 'file-1',
      type: 'file' as const,
      timestamp: 1,
      filePath: '/workspace/file.ts',
      fileName: 'file.ts',
    }];
    const pendingLargePastes = { 'paste-1': 'complete pasted content' };

    const restored = getQueuedMessageComposerDraft({
      id: 'queued-1',
      content: 'transport content',
      displayMessage: 'display content',
      timestamp: 1,
      composerDraft: {
        value: 'original editor value',
        contexts,
        pendingLargePastes,
      },
    });

    expect(restored).toEqual({
      value: 'original editor value',
      contexts,
      pendingLargePastes,
    });
    expect(restored.contexts).not.toBe(contexts);
    expect(restored.pendingLargePastes).not.toBe(pendingLargePastes);
  });

  it('reconstructs image attachments for queues persisted by older builds', () => {
    const restored = getQueuedMessageComposerDraft({
      id: 'queued-legacy',
      content: 'transport content',
      displayMessage: 'edit this message',
      timestamp: 42,
      imageContexts: [{
        id: 'image-1',
        image_path: '/runtime/uploads/image.png',
        mime_type: 'image/png',
        metadata: {
          name: 'image.png',
          width: 120,
          height: 80,
          file_size: 512,
          source: 'clipboard',
        },
      }],
      imageDisplayData: [{
        id: 'image-1',
        name: 'image.png',
        dataUrl: 'data:image/png;base64,abc',
        imagePath: '/runtime/uploads/image.png',
        mimeType: 'image/png',
      }],
    });

    expect(restored.value).toBe('edit this message');
    expect(restored.pendingLargePastes).toEqual({});
    expect(restored.contexts).toEqual([expect.objectContaining({
      id: 'image-1',
      type: 'image',
      imagePath: '/runtime/uploads/image.png',
      imageName: 'image.png',
      width: 120,
      height: 80,
      fileSize: 512,
      mimeType: 'image/png',
      source: 'clipboard',
      isLocal: true,
    })]);
  });
});
