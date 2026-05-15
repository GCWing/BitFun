import { useCallback, useEffect, useRef, useState } from 'react';

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
}): { hotspotVisible: boolean; revealHotspot: () => void; hideHotspot: () => void } {
  const { enabled, onOpen } = params;
  const [hotspotVisible, setHotspotVisible] = useState(false);
  const hideTimerRef = useRef<number | null>(null);
  const onOpenRef = useRef(onOpen);
  onOpenRef.current = onOpen;

  const clearHideTimer = useCallback(() => {
    if (hideTimerRef.current !== null) {
      window.clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
  }, []);

  const hideHotspot = useCallback(() => {
    clearHideTimer();
    setHotspotVisible(false);
  }, [clearHideTimer]);

  const revealHotspot = useCallback(() => {
    if (!enabled) {
      return;
    }
    clearHideTimer();
    setHotspotVisible(true);
    hideTimerRef.current = window.setTimeout(() => {
      setHotspotVisible(false);
      hideTimerRef.current = null;
    }, 2400);
  }, [clearHideTimer, enabled]);

  useEffect(() => {
    if (!enabled) {
      setHotspotVisible(false);
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!shouldHandleCustomizeShortcut(event)) {
        return;
      }
      event.preventDefault();
      onOpenRef.current();
    };

    const handleMouseMove = (event: MouseEvent) => {
      const nearTopRight = event.clientY <= 96 && event.clientX >= window.innerWidth - 80;
      if (nearTopRight) {
        revealHotspot();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('mousemove', handleMouseMove);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('mousemove', handleMouseMove);
      clearHideTimer();
    };
  }, [clearHideTimer, enabled, revealHotspot]);

  return { hotspotVisible, revealHotspot, hideHotspot };
}
