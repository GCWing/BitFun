// @vitest-environment jsdom

import React from 'react';
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useControlTargetEpoch } from '../../../../../mobile-web/src/hooks/useControlTargetEpoch';
import type { RemoteSessionManager } from '../../../../../mobile-web/src/services/RemoteSessionManager';

function Harness({
  manager,
  onRender,
}: {
  manager: RemoteSessionManager;
  onRender: (epoch: number) => void;
}) {
  onRender(useControlTargetEpoch(manager));
  return null;
}

describe('mobile control-target React subscription', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('observes an epoch change even when no other component state changes', async () => {
    let epoch = 0;
    let listener: (() => void) | undefined;
    const unsubscribe = vi.fn();
    const manager = {
      get controlTargetEpoch() { return epoch; },
      onControlTargetChange: (next: () => void) => {
        listener = next;
        return unsubscribe;
      },
    } as unknown as RemoteSessionManager;
    const renders: number[] = [];

    await act(async () => {
      root.render(<Harness manager={manager} onRender={(value) => renders.push(value)} />);
    });
    expect(renders.at(-1)).toBe(0);

    await act(async () => {
      epoch = 1;
      listener?.();
    });
    expect(renders.at(-1)).toBe(1);
  });

  it('detects a target change that occurs while subscribing', async () => {
    let epoch = 0;
    const manager = {
      get controlTargetEpoch() { return epoch; },
      onControlTargetChange: () => {
        epoch = 1;
        return () => {};
      },
    } as unknown as RemoteSessionManager;
    const renders: number[] = [];

    await act(async () => {
      root.render(<Harness manager={manager} onRender={(value) => renders.push(value)} />);
    });

    expect(renders.at(-1)).toBe(1);
  });
});
