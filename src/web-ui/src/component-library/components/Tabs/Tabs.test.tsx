// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { recordInteractionModality } from '@/shared/utils/motionPreference';
import { Tabs, TabPane } from './Tabs';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('Tabs view transitions', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => (
      window.setTimeout(() => callback(performance.now()), 16)
    ));
    vi.stubGlobal('cancelAnimationFrame', (handle: number) => window.clearTimeout(handle));
    recordInteractionModality('programmatic');
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    recordInteractionModality('programmatic');
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  const renderTabs = () => {
    root.render(
      <Tabs defaultActiveKey="first">
        <TabPane tabKey="first" label="First"><div data-testid="first-pane" /></TabPane>
        <TabPane tabKey="second" label="Second"><div data-testid="second-pane" /></TabPane>
      </Tabs>,
    );
  };

  it('retains the outgoing panel for a pointer-selected tab', () => {
    act(renderTabs);
    const secondTab = container.querySelectorAll<HTMLButtonElement>('[role="tab"]')[1];
    recordInteractionModality('pointer');
    act(() => secondTab.click());

    expect(container.querySelector('[data-testid="first-pane"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="second-pane"]')).not.toBeNull();
    expect(
      container.querySelector('.bitfun-view-transition-boundary__view--outgoing')?.hasAttribute('inert'),
    ).toBe(true);
  });

  it('switches a keyboard-selected tab without retaining the outgoing panel', () => {
    act(renderTabs);
    const firstTab = container.querySelectorAll<HTMLButtonElement>('[role="tab"]')[0];
    recordInteractionModality('keyboard');
    act(() => {
      firstTab.dispatchEvent(new KeyboardEvent('keydown', {
        bubbles: true,
        key: 'ArrowRight',
      }));
    });

    expect(container.querySelector('[data-testid="first-pane"]')).toBeNull();
    expect(container.querySelector('[data-testid="second-pane"]')).not.toBeNull();
    expect(container.querySelectorAll('.bitfun-view-transition-boundary__view')).toHaveLength(1);
  });
});
