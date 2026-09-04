// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { RuntimeStatusSlot } from './RuntimeStatusSlot';
import { useRuntimeStatusStore } from '../../store/runtimeStatusStore';

vi.mock('@openbitfun/ui', () => ({
  Spinner: () => <span data-testid="dot-matrix" />,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: () => ['Working on it'],
  }),
}));

describe('RuntimeStatusSlot', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    useRuntimeStatusStore.getState().reset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('keeps the same fixed slot mounted while visibility changes', () => {
    act(() => {
      root.render(<RuntimeStatusSlot sessionId="session-1" placement="footer" />);
    });
    const slot = container.querySelector<HTMLElement>('.runtime-status-slot');
    expect(slot).not.toBeNull();
    expect(slot?.dataset.runtimeStatusVisible).toBe('false');

    act(() => {
      useRuntimeStatusStore.getState().show({
        sessionId: 'session-1',
        turnId: 'turn-1',
        roundId: 'round-1',
      });
    });
    expect(container.querySelector('.runtime-status-slot')).toBe(slot);
    expect(slot?.dataset.runtimeStatusVisible).toBe('true');
    expect(slot?.textContent).toContain('Working on it');

    act(() => {
      useRuntimeStatusStore.getState().show({
        sessionId: 'session-1',
        turnId: 'dispatch-turn',
        roundId: 'dispatch-transfer:job-1',
        label: 'Transferring workspace',
      });
    });
    expect(slot?.textContent).toContain('Transferring workspace');

    act(() => {
      useRuntimeStatusStore.getState().clear({ sessionId: 'session-1' });
    });
    expect(container.querySelector('.runtime-status-slot')).toBe(slot);
    expect(slot?.dataset.runtimeStatusVisible).toBe('false');
  });
});
