/**
 * Session menu shared by the floating chat surfaces (floating window mode and
 * the floating chat bubble).
 *
 * One "+" trigger opening one dropdown: one unified new session action, then
 * the recent sessions to switch to. Both surfaces mount this component
 * rather than each growing its own header affordances, so the interaction stays
 * identical and there is a single place to change it.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { Menu, MenuItem, MenuSeparator } from '@openbitfun/ui';
import { Tooltip, Icon } from '@openbitfun/ui';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
import { activateMainSession } from '../../services/sessionActivation';
import { useFlowChatSessions, resolveDisplayTitle } from './useFlowChatSessions';
import './SessionMenu.scss';

interface SessionMenuProps {
  /** Notified when the dropdown opens/closes, so hosts can close their own menus. */
  onOpenChange?: (open: boolean) => void;
}

export const SessionMenu: React.FC<SessionMenuProps> = ({ onOpenChange }) => {
  const { t } = useTranslation('flow-chat');
  const { activeSessionId, sessions } = useFlowChatSessions();
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const dropdownLayout = useAnchoredPopoverPosition({
    open: isMenuOpen,
    anchorRef: triggerRef,
    popoverRef: dropdownRef,
    preferredPlacement: 'bottom',
    gap: 4,
    layoutRevision: sessions.length,
  });

  const setOpen = useCallback((open: boolean) => {
    setIsMenuOpen(open);
    onOpenChange?.(open);
  }, [onOpenChange]);

  const toggleMenu = useCallback(() => setOpen(!isMenuOpen), [isMenuOpen, setOpen]);

  const createSession = useCallback(() => {
    window.dispatchEvent(new CustomEvent('toolbar-create-session'));
    setOpen(false);
  }, [setOpen]);

  const switchSession = useCallback((e: React.MouseEvent, sessionId: string) => {
    e.stopPropagation();
    e.preventDefault();
    void activateMainSession(sessionId).then((activated) => {
      if (activated) setOpen(false);
    });
  }, [setOpen]);

  useEffect(() => {
    if (!isMenuOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as HTMLElement | null;
      if (!target) return;
      if (rootRef.current?.contains(target)) return;
      if (dropdownRef.current?.contains(target)) return;
      setOpen(false);
    };

    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClickOutside);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isMenuOpen, setOpen]);

  // Escape closes the menu rather than the host surface behind it.
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key !== 'Escape' || !isMenuOpen) return;
    e.preventDefault();
    e.stopPropagation();
    setOpen(false);
  }, [isMenuOpen, setOpen]);

  return (
    <div
      ref={rootRef}
      className="openbitfun-session-menu"
      data-openbitfun-component="session-menu"
      data-openbitfun-part="root"
      data-openbitfun-state={isMenuOpen ? 'open' : undefined}
      onKeyDown={handleKeyDown}
    >
      <Tooltip content={t('toolCards.toolbar.openSessionMenu')}>
        <button
          ref={triggerRef}
          type="button"
          className={[
            'openbitfun-session-menu__trigger',
            isMenuOpen ? 'openbitfun-session-menu__trigger--open' : '',
          ].filter(Boolean).join(' ')}
          data-openbitfun-component="session-menu"
          data-openbitfun-part="trigger"
          onClick={toggleMenu}
          aria-expanded={isMenuOpen}
          aria-haspopup="listbox"
        >
          <Icon name="plus" size="sm" />
        </button>
      </Tooltip>

      {isMenuOpen && createPortal(
        <Menu
          className="openbitfun-session-menu__dropdown"
          data-openbitfun-component="session-menu"
          data-openbitfun-part="dropdown"
          data-openbitfun-placement={dropdownLayout?.placement ?? 'bottom'}
          ref={dropdownRef}
          style={{
            top: `${dropdownLayout?.top ?? 0}px`,
            left: `${dropdownLayout?.left ?? 0}px`,
            visibility: dropdownLayout ? 'visible' : 'hidden',
          }}
          onMouseDown={(e) => e.stopPropagation()}
        >
          <div className="openbitfun-session-menu__actions" data-openbitfun-component="session-menu" data-openbitfun-part="actions">
            <MenuItem
              type="button"
              className="openbitfun-session-menu__item-row openbitfun-session-menu__item-row--new openbitfun-session-menu__item--new"
              data-openbitfun-component="session-menu"
              data-openbitfun-part="item"
              data-openbitfun-item-kind="create"
              data-openbitfun-session-kind="unified"
              onMouseDown={(e) => {
                e.preventDefault();
                e.stopPropagation();
                createSession();
              }}
              leading={(
                <span className="openbitfun-session-menu__item-icon" data-openbitfun-component="session-menu" data-openbitfun-part="itemIcon">
                  <Icon name="plus" size="lg" style={{ width: 13, height: 13 }} />
                </span>
              )}
            >
              <span className="openbitfun-session-menu__item-label" data-openbitfun-component="session-menu" data-openbitfun-part="itemLabel">
                {t('toolCards.toolbar.newSessionItem')}
              </span>
            </MenuItem>
            <MenuSeparator className="openbitfun-session-menu__divider" data-openbitfun-component="session-menu" data-openbitfun-part="divider" />
          </div>

          <div
            className="openbitfun-session-menu__scroll"
            data-openbitfun-component="session-menu"
            data-openbitfun-part="scroll"
            aria-label={t('session.switchSession')}
          >
            {sessions.map((session) => (
              <MenuItem
                key={session.sessionId}
                type="button"
                role="menuitemradio"
                checked={session.sessionId === activeSessionId}
                className={[
                  'openbitfun-session-menu__item-row',
                  session.sessionId === activeSessionId ? 'openbitfun-session-menu__item-row--active' : '',
                ].filter(Boolean).join(' ')}
                data-openbitfun-component="session-menu"
                data-openbitfun-part="item"
                data-openbitfun-item-kind="session"
                data-openbitfun-state={session.sessionId === activeSessionId ? 'active' : undefined}
                onMouseDown={(e) => switchSession(e, session.sessionId)}
              >
                {resolveDisplayTitle(session)}
              </MenuItem>
            ))}
          </div>
        </Menu>,
        getAppearanceOverlayHost(),
      )}
    </div>
  );
};

export default SessionMenu;
