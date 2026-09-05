import React, { useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { composerMenuPlacement } from './composerMenuPlacement';

interface Props {
  anchorRef: React.RefObject<HTMLDivElement>;
  id: string;
  label: string;
  width: number;
  onClose: () => void;
  children: React.ReactNode;
}

/** Keep anchored menus outside the composer and inside the visible viewport. */
export default function ComposerAnchoredMenu({ anchorRef, id, label, width, onClose, children }: Props) {
  const menuRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const [position, setPosition] = useState<ReturnType<typeof composerMenuPlacement> | null>(null);

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    const menu = menuRef.current;
    if (!anchor || !menu) return;
    const viewport = window.visualViewport;
    const updatePosition = () => {
      const rect = anchor.getBoundingClientRect();
      const next = composerMenuPlacement(rect, {
        left: viewport?.offsetLeft ?? 0,
        top: viewport?.offsetTop ?? 0,
        width: viewport?.width ?? window.innerWidth,
        height: viewport?.height ?? window.innerHeight,
      }, width, menu.getBoundingClientRect().height);
      setPosition(current => current && Object.keys(next).every(
        key => current[key as keyof typeof next] === next[key as keyof typeof next],
      ) ? current : next);
    };
    const onOutside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!anchor.contains(target) && !menu.contains(target)) onCloseRef.current();
    };
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      onCloseRef.current();
      anchor.querySelector<HTMLButtonElement>('button')?.focus({ preventScroll: true });
    };
    updatePosition();
    menu.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus({ preventScroll: true });
    const observer = new ResizeObserver(updatePosition);
    observer.observe(anchor);
    observer.observe(menu);
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    viewport?.addEventListener('resize', updatePosition);
    viewport?.addEventListener('scroll', updatePosition);
    document.addEventListener('pointerdown', onOutside);
    document.addEventListener('keydown', onEscape);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
      viewport?.removeEventListener('resize', updatePosition);
      viewport?.removeEventListener('scroll', updatePosition);
      document.removeEventListener('pointerdown', onOutside);
      document.removeEventListener('keydown', onEscape);
    };
  }, [anchorRef, width]);

  return createPortal(
    <div
      ref={menuRef}
      id={id}
      role="dialog"
      aria-label={label}
      data-composer-popover="true"
      className="chat-composer-popover"
      style={position ?? { width, visibility: 'hidden' }}
    >
      {children}
    </div>,
    document.body,
  );
}
