import { useEffect, useRef } from 'react';

type ShortcutEventLike = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'target'>;

function isEditableTarget(target: EventTarget | null): boolean {
  if (!target) {
    return false;
  }

  const elementLike = target as {
    tagName?: string;
    isContentEditable?: boolean;
  };
  const isHtmlElement = typeof HTMLElement !== 'undefined' && target instanceof HTMLElement;
  if (!isHtmlElement && !elementLike.tagName) {
    return false;
  }

  const tagName = (elementLike.tagName ?? '').toLowerCase();
  return (
    elementLike.isContentEditable === true ||
    tagName === 'input' ||
    tagName === 'textarea' ||
    tagName === 'select'
  );
}

export function shouldHandleCustomizeShortcut(event: ShortcutEventLike): boolean {
  if (isEditableTarget(event.target)) {
    return false;
  }

  return event.key.toLowerCase() === 'e' && event.shiftKey && (event.ctrlKey || event.metaKey);
}

export function useMiniAppCustomizeHotspot(params: {
  enabled: boolean;
  onOpen: () => void;
}): void {
  const { enabled, onOpen } = params;
  const onOpenRef = useRef(onOpen);
  onOpenRef.current = onOpen;

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!shouldHandleCustomizeShortcut(event)) {
        return;
      }
      event.preventDefault();
      onOpenRef.current();
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [enabled]);
}
