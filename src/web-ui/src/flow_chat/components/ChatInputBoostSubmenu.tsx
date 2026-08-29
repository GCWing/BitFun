import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import { ChevronRight } from 'lucide-react';

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
  const closeTimerRef = useRef<number | null>(null);
  const panelId = useId();

  const clearCloseTimer = useCallback(() => {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
  }, []);

  const openFlyout = useCallback(() => {
    clearCloseTimer();
    const host = hostRef.current;
    if (host) {
      const bounds = host.getBoundingClientRect();
      setOpenLeft(bounds.right + estimatedPanelWidth > window.innerWidth - 8);
      setOpenUp(bounds.top + estimatedPanelHeight > window.innerHeight - 8);
    }
    setOpen(true);
  }, [clearCloseTimer, estimatedPanelHeight, estimatedPanelWidth]);

  const closeFlyout = useCallback(() => {
    clearCloseTimer();
    closeTimerRef.current = window.setTimeout(() => {
      closeTimerRef.current = null;
      setOpen(false);
    }, 150);
  }, [clearCloseTimer]);

  const closeImmediately = useCallback(() => {
    clearCloseTimer();
    setOpen(false);
  }, [clearCloseTimer]);

  useEffect(() => clearCloseTimer, [clearCloseTimer]);

  return (
    <div
      ref={hostRef}
      className="bitfun-chat-input__boost-submenu-host"
      onMouseEnter={openFlyout}
      onMouseLeave={closeFlyout}
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
