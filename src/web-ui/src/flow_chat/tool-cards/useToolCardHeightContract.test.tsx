// @vitest-environment jsdom

import React, { useLayoutEffect } from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useToolCardHeightContract } from './useToolCardHeightContract';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

function Harness({ height, collapse }: { height: number; collapse: boolean }) {
  const { cardRootRef, dispatchCollapseIntent } = useToolCardHeightContract({
    toolId: 'tool-a',
    toolName: 'Write',
  });

  useLayoutEffect(() => {
    if (collapse) {
      dispatchCollapseIntent('auto');
    }
  }, [collapse, dispatchCollapseIntent]);

  return <div ref={cardRootRef} data-height={height} />;
}

function CustomAnchorHarness({ collapse }: { collapse: boolean }) {
  const anchorRef = React.useRef<HTMLDivElement>(null);
  const { dispatchCollapseIntent } = useToolCardHeightContract({
    toolId: 'thinking-a',
    toolName: 'thinking',
    getAnchorElement: () => anchorRef.current,
    getCardHeight: () => 240,
  });

  useLayoutEffect(() => {
    if (collapse) {
      dispatchCollapseIntent('auto');
    }
  }, [collapse, dispatchCollapseIntent]);

  return <div ref={anchorRef} data-testid="custom-anchor" />;
}

describe('useToolCardHeightContract', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.append(container);
    root = createRoot(container);
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function () {
      const height = Number((this as HTMLElement).dataset.height ?? 0);
      return {
        bottom: height,
        height,
        left: 0,
        right: 300,
        top: 0,
        width: 300,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      };
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });

  it('reports the pre-collapse height and semantic anchor after a state-driven shrink', () => {
    let receivedDetail: Record<string, unknown> | null = null;
    const handleIntent = (event: Event) => {
      receivedDetail = (event as CustomEvent<Record<string, unknown>>).detail;
    };
    window.addEventListener('flowchat:tool-card-collapse-intent', handleIntent);

    try {
      act(() => root.render(<Harness height={320} collapse={false} />));
      act(() => root.render(<Harness height={64} collapse />));

      expect(receivedDetail).toMatchObject({
        toolId: 'tool-a',
        toolName: 'Write',
        cardHeight: 320,
        reason: 'auto',
      });
      expect(receivedDetail?.anchorElement).toBe(container.firstElementChild);
    } finally {
      window.removeEventListener('flowchat:tool-card-collapse-intent', handleIntent);
    }
  });

  it('supports a semantic anchor owned by a non-card wrapper', () => {
    let receivedDetail: Record<string, unknown> | null = null;
    const handleIntent = (event: Event) => {
      receivedDetail = (event as CustomEvent<Record<string, unknown>>).detail;
    };
    window.addEventListener('flowchat:tool-card-collapse-intent', handleIntent);

    try {
      act(() => root.render(<CustomAnchorHarness collapse />));
      expect(receivedDetail).toMatchObject({
        toolId: 'thinking-a',
        toolName: 'thinking',
        cardHeight: 240,
      });
      expect(receivedDetail?.anchorElement).toBe(
        container.querySelector('[data-testid="custom-anchor"]'),
      );
    } finally {
      window.removeEventListener('flowchat:tool-card-collapse-intent', handleIntent);
    }
  });
});
