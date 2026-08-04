// @vitest-environment jsdom

import React, { act, useState } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Modal } from './Modal';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

describe('Modal behavior', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  const pressEscape = () => {
    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      cancelable: true,
    }));
  };

  it('keeps the dialog mounted until its exit animation completes', () => {
    act(() => {
      root.render(
        <Modal isOpen onClose={vi.fn()} title="Motion test">
          Content
        </Modal>,
      );
    });

    expect(document.body.querySelector('.modal')).not.toBeNull();

    act(() => {
      root.render(
        <Modal isOpen={false} onClose={vi.fn()} title="Motion test">
          Content
        </Modal>,
      );
    });

    expect(document.body.querySelector('.modal-overlay--exiting')).not.toBeNull();
    expect(document.body.querySelector('.modal--exiting')).not.toBeNull();
    expect(document.body.querySelector('[role="dialog"]')?.getAttribute('aria-hidden')).toBe('true');

    act(() => vi.advanceTimersByTime(179));
    expect(document.body.querySelector('.modal')).not.toBeNull();

    act(() => vi.advanceTimersByTime(1));
    expect(document.body.querySelector('.modal')).toBeNull();
  });

  it('cancels the exit when the dialog reopens', () => {
    const renderModal = (isOpen: boolean) => {
      root.render(
        <Modal isOpen={isOpen} onClose={vi.fn()} title="Motion test">
          Content
        </Modal>,
      );
    };

    act(() => renderModal(true));
    act(() => renderModal(false));
    act(() => {
      vi.advanceTimersByTime(80);
      renderModal(true);
    });
    act(() => vi.advanceTimersByTime(180));

    expect(document.body.querySelector('.modal')).not.toBeNull();
    expect(document.body.querySelector('.modal--exiting')).toBeNull();
  });

  it('closes a standalone modal on Escape', () => {
    const onClose = vi.fn();
    act(() => {
      root.render(
        <Modal isOpen onClose={onClose} title="Standalone">
          Content
        </Modal>,
      );
    });

    act(pressEscape);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('closes only the topmost modal for each Escape press', () => {
    const parentClosed = vi.fn();
    const childClosed = vi.fn();

    const NestedModals = () => {
      const [parentOpen, setParentOpen] = useState(true);
      const [childOpen, setChildOpen] = useState(true);
      return (
        <>
          <Modal isOpen={parentOpen} onClose={() => {
            parentClosed();
            setParentOpen(false);
          }} title="Parent">
            Parent content
          </Modal>
          <Modal isOpen={childOpen} onClose={() => {
            childClosed();
            setChildOpen(false);
          }} title="Child">
            Child content
          </Modal>
        </>
      );
    };

    act(() => root.render(<NestedModals />));
    act(pressEscape);

    expect(childClosed).toHaveBeenCalledTimes(1);
    expect(parentClosed).not.toHaveBeenCalled();

    act(pressEscape);

    expect(childClosed).toHaveBeenCalledTimes(1);
    expect(parentClosed).toHaveBeenCalledTimes(1);
  });

  it('removes a closed child from the stack before a reopened child is handled', () => {
    const parentClosed = vi.fn();

    const ReopenedChild = () => {
      const [childOpen, setChildOpen] = useState(true);
      return (
        <>
          <Modal isOpen onClose={parentClosed} title="Parent">
            Parent content
          </Modal>
          <Modal isOpen={childOpen} onClose={() => setChildOpen(false)} title="Child">
            Child content
          </Modal>
          {!childOpen ? <button onClick={() => setChildOpen(true)}>Reopen child</button> : null}
        </>
      );
    };

    act(() => root.render(<ReopenedChild />));
    act(pressEscape);
    const reopen = Array.from(document.body.querySelectorAll('button'))
      .find(button => button.textContent === 'Reopen child');
    expect(reopen).toBeTruthy();

    act(() => reopen?.click());
    act(pressEscape);
    expect(parentClosed).not.toHaveBeenCalled();

    act(pressEscape);
    expect(parentClosed).toHaveBeenCalledTimes(1);
  });
});
