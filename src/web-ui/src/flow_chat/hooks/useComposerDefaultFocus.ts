import { useCallback, useEffect, useLayoutEffect, useRef, type RefObject } from 'react';

const NON_TEXT_INPUT_TYPES = new Set([
  'button',
  'checkbox',
  'color',
  'file',
  'hidden',
  'image',
  'radio',
  'range',
  'reset',
  'submit',
]);

function isTextInput(element: Element | null): element is HTMLElement {
  if (!(element instanceof HTMLElement)) {
    return false;
  }

  if (element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) {
    return !element.disabled;
  }

  if (element instanceof HTMLInputElement) {
    return !element.disabled
      && !element.readOnly
      && !NON_TEXT_INPUT_TYPES.has((element.type || 'text').toLowerCase());
  }

  return element.isContentEditable;
}

function isRendered(element: HTMLElement): boolean {
  if (element.closest('[hidden], [aria-hidden="true"], [inert]')) {
    return false;
  }

  return element.getClientRects().length > 0;
}

function hasVisibleTextInputFocus(): boolean {
  return isTextInput(document.activeElement) && isRendered(document.activeElement);
}

function hasBlockingModal(): boolean {
  return document.querySelector('[role="dialog"][aria-modal="true"]') !== null;
}

interface UseComposerDefaultFocusOptions {
  editorRef: RefObject<HTMLElement | null>;
  sessionId: string | null;
  isSceneActive: boolean;
}

export function useComposerDefaultFocus({
  editorRef,
  sessionId,
  isSceneActive,
}: UseComposerDefaultFocusOptions): void {
  const sessionIdRef = useRef(sessionId);
  const sceneActiveRef = useRef(isSceneActive);
  const pendingFrameRef = useRef<number | null>(null);
  sessionIdRef.current = sessionId;
  sceneActiveRef.current = isSceneActive;

  const focusComposerIfUnowned = useCallback(() => {
    if (!sceneActiveRef.current || !sessionIdRef.current) {
      return;
    }

    if (pendingFrameRef.current !== null) {
      window.cancelAnimationFrame(pendingFrameRef.current);
    }

    pendingFrameRef.current = window.requestAnimationFrame(() => {
      pendingFrameRef.current = null;
      const editor = editorRef.current;

      if (
        !editor
        || !sceneActiveRef.current
        || !sessionIdRef.current
        || document.activeElement === editor
        || hasVisibleTextInputFocus()
        || hasBlockingModal()
      ) {
        return;
      }

      editor.focus({ preventScroll: true });
    });
  }, [editorRef]);

  useLayoutEffect(() => {
    focusComposerIfUnowned();
  }, [focusComposerIfUnowned, isSceneActive, sessionId]);

  useEffect(() => {
    window.addEventListener('focus', focusComposerIfUnowned);
    return () => {
      window.removeEventListener('focus', focusComposerIfUnowned);
      if (pendingFrameRef.current !== null) {
        window.cancelAnimationFrame(pendingFrameRef.current);
        pendingFrameRef.current = null;
      }
    };
  }, [focusComposerIfUnowned]);
}
