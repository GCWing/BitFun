// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const sceneHarness = vi.hoisted(() => {
  let resolveAgents: (() => void) | null = null;
  let agentsAreReady = false;
  const agentsReady = new Promise<void>((resolve) => {
    resolveAgents = () => {
      agentsAreReady = true;
      resolve();
    };
  });

  return {
    state: {
      openTabs: [{ id: 'session', openedAt: 0, lastUsed: 0 }],
      activeTabId: 'session',
      navigationMotion: 'instant',
      navigationSequence: 0,
    },
    agentsReady,
    agentsAreReady: () => agentsAreReady,
    resolveAgents: () => resolveAgents?.(),
  };
});

vi.mock('../hooks/useSceneManager', () => ({
  useSceneManager: () => sceneHarness.state,
}));

vi.mock('../hooks/useDialogCompletionNotify', () => ({
  useDialogCompletionNotify: () => undefined,
}));

vi.mock('@/infrastructure/i18n/hooks/useI18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('@/component-library', () => ({
  DotMatrixLoader: () => <div data-testid="scene-loader" />,
}));

vi.mock('./session/SessionScene', () => ({
  default: () => <div data-testid="session-scene-content" />,
}));

vi.mock('./settings/SettingsScene', () => ({
  default: () => <div data-testid="settings-scene-content" />,
}));

vi.mock('./assistant/AssistantScene', () => ({
  default: () => <div data-testid="assistant-scene-content" />,
}));

vi.mock('./agents/AgentsScene', () => ({
  default: () => {
    if (!sceneHarness.agentsAreReady()) {
      throw sceneHarness.agentsReady;
    }
    return <div data-testid="agents-scene-content" />;
  },
}));

import SceneViewport from './SceneViewport';

describe('SceneViewport transitions', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => (
      window.setTimeout(() => callback(performance.now()), 16)
    ));
    vi.stubGlobal('cancelAnimationFrame', (handle: number) => window.clearTimeout(handle));
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  function visibleScenes(): Element[] {
    return Array.from(container.querySelectorAll('[data-testid="scene-viewport-scene"]'))
      .filter(scene => !scene.hasAttribute('hidden'));
  }

  it('keeps one scene visible while a lazy pointer target becomes ready', async () => {
    act(() => root.render(<SceneViewport />));
    expect(visibleScenes().map(scene => scene.getAttribute('data-scene-id'))).toEqual(['session']);

    sceneHarness.state = {
      openTabs: [
        { id: 'session', openedAt: 0, lastUsed: 0 },
        { id: 'agents', openedAt: 1, lastUsed: 1 },
      ],
      activeTabId: 'agents',
      navigationMotion: 'pointer',
      navigationSequence: 1,
    };
    await act(async () => {
      root.render(<SceneViewport />);
      await Promise.resolve();
    });

    expect(visibleScenes().map(scene => scene.getAttribute('data-scene-id'))).toEqual(['session']);

    await act(async () => {
      sceneHarness.resolveAgents();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(visibleScenes().map(scene => scene.getAttribute('data-scene-id'))).toEqual(['agents']);
    expect(container.querySelector('[data-scene-id="session"]')?.hasAttribute('hidden')).toBe(true);
    expect(container.querySelector('[data-scene-id="agents"]')?.classList.contains(
      'bitfun-scene-viewport__scene--incoming',
    )).toBe(true);

    act(() => vi.advanceTimersByTime(32));
    expect(container.querySelector('[data-scene-id="agents"]')?.classList.contains(
      'bitfun-scene-viewport__scene--incoming',
    )).toBe(true);

    act(() => vi.advanceTimersByTime(479));
    expect(container.querySelector('[data-scene-id="agents"]')?.classList.contains(
      'bitfun-scene-viewport__scene--incoming',
    )).toBe(true);

    act(() => vi.advanceTimersByTime(1));
    expect(container.querySelector('[data-scene-id="agents"]')?.classList.contains(
      'bitfun-scene-viewport__scene--incoming',
    )).toBe(false);

    sceneHarness.state = {
      ...sceneHarness.state,
      activeTabId: 'session',
      navigationSequence: 2,
    };
    act(() => root.render(<SceneViewport />));

    expect(visibleScenes().map(scene => scene.getAttribute('data-scene-id'))).toEqual(['session']);
    expect(container.querySelector('[data-scene-id="agents"]')?.hasAttribute('hidden')).toBe(true);
  });
});
