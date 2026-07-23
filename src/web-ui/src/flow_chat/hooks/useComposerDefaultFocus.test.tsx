// @vitest-environment jsdom
import React, { act, useRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useComposerDefaultFocus } from './useComposerDefaultFocus';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

interface ProbeProps {
  sessionId: string | null;
  isSceneActive: boolean;
}

function Probe({ sessionId, isSceneActive }: ProbeProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  useComposerDefaultFocus({ editorRef, sessionId, isSceneActive });

  return <div ref={editorRef} contentEditable tabIndex={0} data-testid="composer" />;
}

function markRendered(element: HTMLElement): void {
  vi.spyOn(element, 'getClientRects').mockReturnValue([
    { width: 100, height: 20 } as DOMRect,
  ] as unknown as DOMRectList);
}

describe('useComposerDefaultFocus', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', () => {});
    window.requestAnimationFrame = globalThis.requestAnimationFrame;
    window.cancelAnimationFrame = globalThis.cancelAnimationFrame;

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document.body.replaceChildren();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  function renderProbe(props: ProbeProps): HTMLDivElement {
    act(() => root.render(<Probe {...props} />));
    return container.querySelector('[data-testid="composer"]') as HTMLDivElement;
  }

  it('focuses the composer when the active session has no input owner', () => {
    const composer = renderProbe({ sessionId: 'session-a', isSceneActive: true });

    expect(document.activeElement).toBe(composer);
  });

  it('does not focus an inactive scene', () => {
    const composer = renderProbe({ sessionId: 'session-a', isSceneActive: false });

    expect(document.activeElement).not.toBe(composer);
  });

  it('preserves a visible text input selected by the user', () => {
    const alternateInput = document.createElement('textarea');
    document.body.appendChild(alternateInput);
    markRendered(alternateInput);
    alternateInput.focus();

    const composer = renderProbe({ sessionId: 'session-a', isSceneActive: true });
    act(() => window.dispatchEvent(new FocusEvent('focus')));

    expect(document.activeElement).toBe(alternateInput);
    expect(document.activeElement).not.toBe(composer);
  });

  it('focuses the composer when the previous input owner is no longer rendered', () => {
    const hiddenInput = document.createElement('textarea');
    hiddenInput.hidden = true;
    document.body.appendChild(hiddenInput);
    hiddenInput.focus();

    const composer = renderProbe({ sessionId: 'session-a', isSceneActive: true });

    expect(document.activeElement).toBe(composer);
  });

  it('focuses the composer on window activation when only a button was focused', () => {
    const composer = renderProbe({ sessionId: 'session-a', isSceneActive: true });
    const button = document.createElement('button');
    document.body.appendChild(button);
    button.focus();

    act(() => window.dispatchEvent(new FocusEvent('focus')));

    expect(document.activeElement).toBe(composer);
  });

  it('does not refocus the composer when it already owns focus', () => {
    const composer = renderProbe({ sessionId: 'session-a', isSceneActive: true });
    const focusSpy = vi.spyOn(composer, 'focus');

    act(() => {
      window.dispatchEvent(new FocusEvent('focus'));
      window.dispatchEvent(new FocusEvent('focus'));
    });

    expect(document.activeElement).toBe(composer);
    expect(focusSpy).not.toHaveBeenCalled();
  });

  it('does not move focus behind a modal dialog', () => {
    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');
    const button = document.createElement('button');
    dialog.appendChild(button);
    document.body.appendChild(dialog);
    button.focus();

    const composer = renderProbe({ sessionId: 'session-a', isSceneActive: true });

    expect(document.activeElement).toBe(button);
    expect(document.activeElement).not.toBe(composer);
  });
});
