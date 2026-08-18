// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PopupCloseButton } from './PopupCloseButton';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('PopupCloseButton', () => {
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

  it('keeps the canonical size and preserves the owning surface metadata', () => {
    const onClick = vi.fn();
    act(() => {
      root.render(
        <PopupCloseButton
          aria-label="Close dialog"
          onClick={onClick}
          data-bf-component="modal"
          data-bf-part="close"
        />,
      );
    });

    const button = container.querySelector<HTMLButtonElement>('button');
    expect(button).not.toBeNull();
    expect(button?.type).toBe('button');
    expect(button?.getAttribute('aria-label')).toBe('Close dialog');
    expect(button?.getAttribute('data-bf-component')).toBe('modal');
    expect(button?.getAttribute('data-bf-part')).toBe('close');
    expect(button?.getAttribute('data-bf-role')).toBe('popup-close');
    expect(button?.getAttribute('data-bf-size')).toBe('medium');
    expect(button?.classList.contains('popup-close-button')).toBe(true);
    expect(button?.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');

    act(() => button?.click());
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
