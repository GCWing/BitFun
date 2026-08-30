import React, { useCallback, useId, useRef, useState } from 'react';
import { ChevronRight } from 'lucide-react';
import { useSubmenuIntent } from '@/shared/utils/useSubmenuIntent';

interface ChatInputBoostSubmenuProps {
  label: string;
  icon: React.ReactNode;
  children: React.ReactNode;
  estimatedPanelWidth?: number;
  estimatedPanelHeight?: number;
  testId?: string;
}

/** Shared second-level disclosure used by ChatInput's add menu. */
export const ChatInputBoostSubmenu: React.FC<ChatInputBoostSubmenuProps> = ({
  label,
  icon,
  children,
  estimatedPanelWidth = 260,
  estimatedPanelHeight = 200,
  testId,
}) => {
  const [open, setOpen] = useState(false);
  const [openLeft, setOpenLeft] = useState(false);
  const [openUp, setOpenUp] = useState(false);
  const hostRef = useRef<HTMLDivElement>(null);
  const submenuRef = useRef<HTMLDivElement>(null);
  const panelId = useId();

  const setActiveSubmenu = useCallback((id: 'submenu' | null) => {
    if (id !== null) {
      const host = hostRef.current;
      if (host) {
        const bounds = host.getBoundingClientRect();
        setOpenLeft(bounds.right + estimatedPanelWidth > window.innerWidth - 8);
        setOpenUp(bounds.top + estimatedPanelHeight > window.innerHeight - 8);
      }
    }
    setOpen(id !== null);
  }, [estimatedPanelHeight, estimatedPanelWidth]);

  const {
    requestChange,
    requestClose,
    keepOpen,
    openNow,
    closeNow: closeImmediately,
  } = useSubmenuIntent<'submenu'>({
    activeId: open ? 'submenu' : null,
    onActiveIdChange: setActiveSubmenu,
    parentRef: hostRef,
    submenuRef,
    openDelayMs: 0,
    closeDelayMs: 180,
  });

  const openFlyout = useCallback(() => {
    openNow('submenu');
  }, [openNow]);

  return (
    <div
      ref={hostRef}
      className="bitfun-chat-input__boost-submenu-host"
      onPointerEnter={event => requestChange('submenu', event)}
      onPointerLeave={requestClose}
      data-testid={testId}
    >
      <div
        role="menuitem"
        tabIndex={0}
        className="bitfun-chat-input__boost-submenu-trigger"
        data-bf-component="chat-input"
        data-bf-part="boostSubmenuTrigger"
        data-bf-state={open ? 'open' : undefined}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={(event) => {
          event.stopPropagation();
          openFlyout();
        }}
        onKeyDown={(event) => {
          if (event.key === 'Escape' || event.key === 'ArrowLeft') {
            if (!open) return;
            event.preventDefault();
            event.stopPropagation();
            closeImmediately();
            return;
          }
          if (event.key !== 'Enter' && event.key !== ' ' && event.key !== 'ArrowRight') return;
          event.preventDefault();
          event.stopPropagation();
          openFlyout();
        }}
      >
        <span className="bitfun-chat-input__boost-submenu-trigger-main">
          {icon}
          <span>{label}</span>
        </span>
        <ChevronRight
          size={14}
          className="bitfun-chat-input__boost-submenu-chevron"
          aria-hidden
        />
      </div>
      <div
        ref={submenuRef}
        onPointerEnter={keepOpen}
        onPointerLeave={requestClose}
        className={[
          'bitfun-chat-input__boost-submenu-shell',
          open ? 'bitfun-chat-input__boost-submenu-shell--open' : '',
          openLeft ? 'bitfun-chat-input__boost-submenu-shell--left' : '',
          openUp ? 'bitfun-chat-input__boost-submenu-shell--up' : '',
        ].filter(Boolean).join(' ')}
      >
        <div
          id={panelId}
          role="menu"
          className="bitfun-chat-input__boost-submenu-panel"
          data-bf-component="chat-input"
          data-bf-part="boostSubmenuPanel"
          data-bf-state={open ? 'open' : undefined}
          aria-hidden={!open}
          {...(!open ? { inert: '' } : {})}
        >
          {children}
        </div>
      </div>
    </div>
  );
};

export default ChatInputBoostSubmenu;
