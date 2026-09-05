/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ConversationModeSurface } from './ConversationModeSurface';
import type { VoiceMiniAppCallTarget } from './voiceClientContext';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  controller: {
    enabled: true,
    phase: 'idle',
    start: vi.fn(),
    end: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('./RealtimeVoiceCallContext', () => ({
  useRealtimeVoiceCall: () => mocks.controller,
}));

vi.mock('./RealtimeVoiceCallPanel', () => ({
  RealtimeVoiceCallPanel: () => <div data-testid="voice-panel" />,
}));

const miniAppTarget: VoiceMiniAppCallTarget = {
  kind: 'miniapp',
  appId: 'builtin-ppt-live',
  appName: 'PPT Live',
  claimToken: 'builtin-ppt-live#1',
  sessionId: 'ppt-session',
};

describe('ConversationModeSurface', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.controller.enabled = true;
    mocks.controller.phase = 'idle';
    mocks.controller.start.mockReset();
    mocks.controller.end.mockReset();
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    container.remove();
  });

  it('shows the supplied chat surface and starts voice with its captured route', async () => {
    await act(async () => {
      root.render(
        <ConversationModeSurface voiceTarget={miniAppTarget}>
          <div data-testid="chat-surface" />
        </ConversationModeSurface>,
      );
    });

    expect(container.querySelector('[data-testid="chat-surface"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="voice-panel"]')).toBeNull();

    await act(async () => {
      container.querySelector('button')?.click();
    });
    expect(mocks.controller.start).toHaveBeenCalledWith(miniAppTarget);
  });

  it('renders the same voice panel for every host and switches back through one action', async () => {
    mocks.controller.phase = 'live';
    await act(async () => {
      root.render(
        <ConversationModeSurface>
          <div data-testid="chat-surface" />
        </ConversationModeSurface>,
      );
    });

    expect(container.querySelector('[data-testid="voice-panel"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="chat-surface"]')).toBeNull();

    await act(async () => {
      container.querySelector('button')?.click();
    });
    expect(mocks.controller.end).toHaveBeenCalledOnce();
  });

  it('cannot silently fall back to workspace voice while a MiniApp route is unavailable', async () => {
    await act(async () => {
      root.render(
        <ConversationModeSurface voiceStartDisabled>
          <div data-testid="chat-surface" />
        </ConversationModeSurface>,
      );
    });

    const button = container.querySelector('button');
    expect(button?.disabled).toBe(true);
    button?.click();
    expect(mocks.controller.start).not.toHaveBeenCalled();
  });

  it('hides the realtime voice switch until the client voice assistant is enabled', async () => {
    mocks.controller.enabled = false;
    await act(async () => {
      root.render(
        <ConversationModeSurface>
          <div data-testid="chat-surface" />
        </ConversationModeSurface>,
      );
    });

    expect(container.querySelector('[data-testid="chat-surface"]')).not.toBeNull();
    expect(container.querySelector('[data-openbitfun-part="modeSwitch"]')).toBeNull();
    expect(container.querySelector('button')).toBeNull();
  });

  it('keeps the hang-up switch while a call is already live after the assistant is disabled', async () => {
    mocks.controller.enabled = false;
    mocks.controller.phase = 'live';
    await act(async () => {
      root.render(
        <ConversationModeSurface>
          <div data-testid="chat-surface" />
        </ConversationModeSurface>,
      );
    });

    expect(container.querySelector('[data-openbitfun-part="modeSwitch"]')).not.toBeNull();
    await act(async () => {
      container.querySelector('button')?.click();
    });
    expect(mocks.controller.end).toHaveBeenCalledOnce();
  });
});
