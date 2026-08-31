import React, { useCallback, useId, useRef } from 'react';
import { createPortal } from 'react-dom';
import { ChevronRight } from 'lucide-react';
import { Menu, MenuItem } from '@bitfun/ui';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useSideAnchoredPopoverPosition } from '@/shared/utils/useSideAnchoredPopoverPosition';

interface ChatInputBoostSubmenuProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  label: string;
  icon: React.ReactNode;
  children: React.ReactNode;
  testId?: string;
}

/** Shared second-level disclosure used by ChatInput's add menu. */
export const ChatInputBoostSubmenu: React.FC<ChatInputBoostSubmenuProps> = ({
  open,
  onOpenChange: setOpen,
  label,
  icon,
  children,
  testId,
}) => {
  const triggerRef = useRef<HTMLButtonElement>(null);
  const submenuRef = useRef<HTMLDivElement>(null);
  const panelId = useId();
  const layout = useSideAnchoredPopoverPosition({
    open,
    anchorRef: triggerRef,
    popoverRef: submenuRef,
    layoutRevision: React.Children.count(children),
  });

  const openFlyout = useCallback((focusFirstItem = false) => {
    setOpen(true);
    if (focusFirstItem) {
      requestAnimationFrame(() => {
        submenuRef.current?.querySelector<HTMLButtonElement>('[data-bf-menu-item]')?.focus();
      });
    }
  }, [setOpen]);

  return (
    <div
      className="bitfun-chat-input__boost-submenu-host"
      data-testid={testId}
    >
      <MenuItem
        ref={triggerRef}
        data-bf-component="chat-input"
        data-bf-part="boostSubmenuTrigger"
        data-bf-state={open ? 'open' : undefined}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={panelId}
        leading={icon}
        metadata={<ChevronRight size={14} aria-hidden />}
        onClick={(event) => {
          event.stopPropagation();
          if (open) setOpen(false);
          else openFlyout();
        }}
        onKeyDown={(event) => {
          if (event.key === 'Escape' || event.key === 'ArrowLeft') {
            if (!open) return;
            event.preventDefault();
            event.stopPropagation();
            setOpen(false);
            return;
          }
          if (event.key !== 'ArrowRight') return;
          event.preventDefault();
          event.stopPropagation();
          openFlyout(true);
        }}
      >
        {label}
      </MenuItem>
      {open ? createPortal(
        <Menu
          ref={submenuRef}
          id={panelId}
          className="bitfun-chat-input__boost-submenu-panel"
          data-bf-component="chat-input"
          data-bf-part="boostSubmenuPanel"
          data-bf-state="open"
          data-bf-placement={layout ? `${layout.placement}-${layout.alignment}` : 'right-start'}
          style={{
            top: layout?.top ?? 0,
            left: layout?.left ?? 0,
            visibility: layout ? 'visible' : 'hidden',
          }}
          onMouseDown={event => event.stopPropagation()}
          onKeyDown={event => {
            if (event.key !== 'ArrowLeft' && event.key !== 'Escape') return;
            event.preventDefault();
            event.stopPropagation();
            setOpen(false);
            requestAnimationFrame(() => triggerRef.current?.focus());
          }}
          >
            {children}
        </Menu>,
        getAppearanceOverlayHost(),
      ) : null}
    </div>
  );
};

export default ChatInputBoostSubmenu;
