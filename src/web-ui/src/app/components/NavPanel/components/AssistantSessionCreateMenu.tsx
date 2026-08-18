import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown, Plus } from 'lucide-react';
import { Tooltip } from '@/component-library';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import type { WorkspaceInfo } from '@/shared/types';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';

interface AssistantSessionCreateMenuProps {
  assistants: WorkspaceInfo[];
  primaryAssistant: WorkspaceInfo | null;
  onCreatePrimary: () => void | Promise<void>;
  onCreateAssistant: (workspace: WorkspaceInfo) => void | Promise<void>;
}

const getAssistantDisplayName = (workspace: WorkspaceInfo): string =>
  workspace.identity?.name?.trim() || workspace.name;

const AssistantSessionCreateMenu: React.FC<AssistantSessionCreateMenuProps> = ({
  assistants,
  primaryAssistant,
  onCreatePrimary,
  onCreateAssistant,
}) => {
  const { t } = useI18n('common');
  const [menuOpen, setMenuOpen] = useState(false);
  const anchorRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const orderedAssistants = useMemo(() => {
    if (!primaryAssistant) return assistants;
    return [
      primaryAssistant,
      ...assistants.filter(workspace => workspace.id !== primaryAssistant.id),
    ];
  }, [assistants, primaryAssistant]);

  const menuLayout = useAnchoredPopoverPosition({
    open: menuOpen,
    anchorRef,
    popoverRef: menuRef,
    preferredPlacement: 'bottom',
    alignment: 'end',
    gap: 6,
    layoutRevision: orderedAssistants.length,
  });

  const closeMenu = useCallback(() => setMenuOpen(false), []);

  useEffect(() => {
    if (!menuOpen) return;

    const handleMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (
        target &&
        (anchorRef.current?.contains(target) || menuRef.current?.contains(target))
      ) {
        return;
      }
      closeMenu();
    };
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeMenu();
    };

    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('keydown', handleEscape, true);
    return () => {
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('keydown', handleEscape, true);
    };
  }, [closeMenu, menuOpen]);

  useEffect(() => {
    if (orderedAssistants.length === 0) closeMenu();
  }, [closeMenu, orderedAssistants.length]);

  const createPrimaryLabel = t('nav.sessions.newPrimaryAssistantSession');
  const chooseAssistantLabel = t('nav.sessions.chooseAssistant');

  return (
    <div
      ref={anchorRef}
      className={`bitfun-nav-panel__assistant-session-actions${menuOpen ? ' is-open' : ''}`}
      data-bf-component="nav-panel"
      data-bf-part="assistantSessionActions"
      data-bf-state={menuOpen ? 'open' : undefined}
    >
      <div className={`bitfun-nav-panel__assistant-session-split-button${menuOpen ? ' is-active' : ''}`}>
        <Tooltip content={createPrimaryLabel} placement="right" followCursor>
          <button
            type="button"
            className="bitfun-nav-panel__assistant-session-split-main"
            aria-label={createPrimaryLabel}
            onClick={() => {
              closeMenu();
              void onCreatePrimary();
            }}
            data-testid="nav-primary-assistant-session-add-btn"
          >
            <Plus size={13} />
          </button>
        </Tooltip>
        <Tooltip content={chooseAssistantLabel} placement="right" followCursor disabled={menuOpen}>
          <button
            type="button"
            className="bitfun-nav-panel__assistant-session-split-toggle"
            aria-label={chooseAssistantLabel}
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            disabled={orderedAssistants.length === 0}
            onClick={() => setMenuOpen(open => !open)}
            data-testid="nav-assistant-session-menu-toggle"
          >
            <ChevronDown size={11} />
          </button>
        </Tooltip>
      </div>

      {menuOpen ? createPortal(
        <div
          ref={menuRef}
          className="bitfun-nav-panel__assistant-session-menu"
          data-bf-component="nav-panel"
          data-bf-part="assistantSessionMenu"
          data-bf-placement={menuLayout?.placement ?? 'bottom'}
          role="menu"
          aria-label={chooseAssistantLabel}
          data-testid="nav-assistant-session-menu"
          style={{
            top: `${menuLayout?.top ?? 0}px`,
            left: `${menuLayout?.left ?? 0}px`,
            visibility: menuLayout ? 'visible' : 'hidden',
          }}
        >
          {orderedAssistants.map(workspace => {
            const assistantName = getAssistantDisplayName(workspace);
            return (
              <button
                key={workspace.id}
                type="button"
                className="bitfun-nav-panel__assistant-session-menu-item"
                role="menuitem"
                aria-label={t('nav.sessions.newAssistantSessionFor', { assistantName })}
                onClick={() => {
                  closeMenu();
                  void onCreateAssistant(workspace);
                }}
                data-testid={`nav-assistant-session-menu-item-${workspace.id}`}
              >
                <Plus size={13} aria-hidden="true" />
                <span className="bitfun-nav-panel__assistant-session-menu-name">{assistantName}</span>
              </button>
            );
          })}
        </div>,
        getAppearanceOverlayHost(),
      ) : null}
    </div>
  );
};

export default React.memo(AssistantSessionCreateMenu);
