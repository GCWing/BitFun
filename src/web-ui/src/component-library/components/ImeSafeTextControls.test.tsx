// @vitest-environment jsdom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { Input } from '@bitfun/ui';

import { isImeOwnedKeyboardEvent } from '@/shared/utils/ime';
import { Textarea } from './Textarea';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('IME-safe text controls', () => {
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

  it.each([
    ['Input', <Input onKeyDown={vi.fn()} />],
    ['Textarea', <Textarea onKeyDown={vi.fn()} />],
  ])('does not submit, cancel, or bubble while %s is composing', (_name, control) => {
    const onKeyDown = control.props.onKeyDown as ReturnType<typeof vi.fn>;
    const bubbledKeyDown = vi.fn();
    document.addEventListener('keydown', bubbledKeyDown);

    act(() => root.render(control));
    const textControl = container.querySelector('input, textarea');
    expect(textControl).not.toBeNull();

    act(() => {
      textControl?.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
      textControl?.dispatchEvent(new KeyboardEvent('keydown', {
        bubbles: true,
        key: 'Enter',
      }));
      textControl?.dispatchEvent(new KeyboardEvent('keydown', {
        bubbles: true,
        key: 'Escape',
      }));
    });

    expect(onKeyDown).not.toHaveBeenCalled();
    expect(bubbledKeyDown).not.toHaveBeenCalled();

    act(() => {
      textControl?.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
      textControl?.dispatchEvent(new KeyboardEvent('keydown', {
        bubbles: true,
        key: 'Enter',
      }));
    });

    expect(onKeyDown).toHaveBeenCalledTimes(1);
    expect(bubbledKeyDown).toHaveBeenCalledTimes(1);
    document.removeEventListener('keydown', bubbledKeyDown);
  });

  it('recognizes the WebKit keyCode 229 fallback', () => {
    expect(isImeOwnedKeyboardEvent({ isComposing: true })).toBe(true);
    expect(isImeOwnedKeyboardEvent({ keyCode: 229 })).toBe(true);
    expect(isImeOwnedKeyboardEvent({ nativeEvent: { keyCode: 229 } })).toBe(true);
    expect(isImeOwnedKeyboardEvent({ keyCode: 13 })).toBe(false);
  });
});
