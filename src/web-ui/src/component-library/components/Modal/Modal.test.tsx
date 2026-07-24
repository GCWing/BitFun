// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Modal } from './Modal';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/infrastructure/i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

describe('Modal motion presence', () => {
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
  });

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

    act(() => {
      vi.advanceTimersByTime(179);
    });
    expect(document.body.querySelector('.modal')).not.toBeNull();

    act(() => {
      vi.advanceTimersByTime(1);
    });
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
    act(() => {
      vi.advanceTimersByTime(180);
    });

    expect(document.body.querySelector('.modal')).not.toBeNull();
    expect(document.body.querySelector('.modal--exiting')).toBeNull();
  });
});
