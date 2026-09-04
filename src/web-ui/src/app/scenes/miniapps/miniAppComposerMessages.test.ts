import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  completeMiniAppComposerMessage,
  postMiniAppComposerMessage,
  rejectPendingMiniAppComposerMessages,
  requestMiniAppComposerMessage,
} from './miniAppComposerMessages';
import { MINIAPP_COMPOSER_MESSAGE_EVENT } from './miniAppStore';

class TestCustomEvent<T> extends Event {
  readonly detail: T;

  constructor(type: string, init?: CustomEventInit<T>) {
    super(type);
    this.detail = init?.detail as T;
  }
}

describe('MiniApp composer message lifecycle', () => {
  beforeEach(() => {
    const eventTarget = new EventTarget();
    Object.assign(eventTarget, {
      setTimeout: globalThis.setTimeout.bind(globalThis),
      clearTimeout: globalThis.clearTimeout.bind(globalThis),
    });
    vi.stubGlobal('window', eventTarget);
    vi.stubGlobal('CustomEvent', TestCustomEvent);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('adds a unique request id and source to ordinary composer submissions', () => {
    let received: Record<string, unknown> | undefined;
    window.addEventListener(MINIAPP_COMPOSER_MESSAGE_EVENT, event => {
      received = (event as CustomEvent<Record<string, unknown>>).detail;
    });

    const requestId = postMiniAppComposerMessage({ token: 'ppt#1', text: 'build slides' });

    expect(received).toMatchObject({
      token: 'ppt#1',
      text: 'build slides',
      source: 'composer',
      requestId,
    });
  });

  it('settles a voice submission only from the runner that owns its token', async () => {
    let requestId = '';
    window.addEventListener(MINIAPP_COMPOSER_MESSAGE_EVENT, event => {
      requestId = (event as CustomEvent<{ requestId: string }>).detail.requestId;
    });

    const completion = requestMiniAppComposerMessage({
      token: 'ppt#1',
      source: 'realtime_voice',
      text: 'make page three visual',
    });

    expect(completeMiniAppComposerMessage('ppt#2', requestId)).toBe(false);
    expect(completeMiniAppComposerMessage('ppt#1', requestId)).toBe(true);
    await expect(completion).resolves.toBeUndefined();
  });

  it('propagates MiniApp callback failures to realtime voice', async () => {
    let requestId = '';
    window.addEventListener(MINIAPP_COMPOSER_MESSAGE_EVENT, event => {
      requestId = (event as CustomEvent<{ requestId: string }>).detail.requestId;
    });
    const completion = requestMiniAppComposerMessage({ token: 'ppt#1', text: 'build' });

    completeMiniAppComposerMessage('ppt#1', requestId, 'Deck generation failed');

    await expect(completion).rejects.toThrow('Deck generation failed');
  });

  it('stops waiting when the voice task is cancelled', async () => {
    const controller = new AbortController();
    const completion = requestMiniAppComposerMessage(
      { token: 'ppt#1', text: 'build' },
      controller.signal,
    );

    controller.abort();

    await expect(completion).rejects.toThrow('MiniApp message was cancelled');
  });

  it('fails pending work when the owning MiniApp releases its composer', async () => {
    const completion = requestMiniAppComposerMessage({ token: 'ppt#1', text: 'build' });

    rejectPendingMiniAppComposerMessages('ppt#1', 'MiniApp closed');

    await expect(completion).rejects.toThrow('MiniApp closed');
  });
});
