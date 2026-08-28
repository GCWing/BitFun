import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, ChevronRight, ListFilter } from 'lucide-react';

import { useI18n } from '@/infrastructure/i18n';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { Tooltip } from '@bitfun/ui';
import {
  DEFAULT_WORKSPACE_SESSION_VIEW,
  hasWorkspaceSessionFilters,
  type WorkspaceSessionEnvironment,
  type WorkspaceSessionOrdering,
  type WorkspaceSessionShow,
  type WorkspaceSessionSource,
  type WorkspaceSessionStatus,
  type WorkspaceSessionWorktree,
  useWorkspaceSessionViewStore,
} from '../workspaceSessionView';

type Submenu = 'ordering' | 'show' | 'status' | 'worktree' | 'environment' | 'source';

interface SingleChoiceMenu<T extends string> {
  kind: 'single';
  value: T;
  options: readonly T[];
  choose: (value: T) => void;
}

interface MultiChoiceMenu<T extends string> {
  kind: 'multi';
  hidden: readonly T[];
  options: readonly T[];
  toggle: (value: T) => void;
}

type MenuDefinition = SingleChoiceMenu<string> | MultiChoiceMenu<string>;

const MAIN_MENU_WIDTH = 220;
const SUBMENU_WIDTH = 220;
const MENU_GAP = 5;
const VIEWPORT_PADDING = 8;
const ROW_HEIGHT = 34;

const clamp = (value: number, min: number, max: number): number =>
  Math.min(Math.max(value, min), Math.max(min, max));

const WorkspaceSessionFilterMenu: React.FC = () => {
  const { t } = useI18n('common');
  const view = useWorkspaceSessionViewStore();
  const [open, setOpen] = useState(false);
  const [activeSubmenu, setActiveSubmenu] = useState<Submenu | null>(null);
  const [menuPosition, setMenuPosition] = useState({ top: 0, left: 0 });
  const buttonRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const submenuRef = useRef<HTMLDivElement>(null);

  const isCustomized = view.ordering !== DEFAULT_WORKSPACE_SESSION_VIEW.ordering
    || view.show !== DEFAULT_WORKSPACE_SESSION_VIEW.show
    || hasWorkspaceSessionFilters(view.filters);

  const updatePosition = useCallback(() => {
    const anchor = buttonRef.current?.getBoundingClientRect();
    if (!anchor) return;
    const measuredHeight = menuRef.current?.offsetHeight ?? 422;
    const preferredRight = anchor.right + MENU_GAP;
    const canOpenRight = preferredRight + MAIN_MENU_WIDTH <= window.innerWidth - VIEWPORT_PADDING;
    setMenuPosition({
      top: clamp(anchor.top - 6, VIEWPORT_PADDING, window.innerHeight - measuredHeight - VIEWPORT_PADDING),
      left: clamp(
        canOpenRight ? preferredRight : anchor.left - MENU_GAP - MAIN_MENU_WIDTH,
        VIEWPORT_PADDING,
        window.innerWidth - MAIN_MENU_WIDTH - VIEWPORT_PADDING,
      ),
    });
  }, []);

  const close = useCallback(() => {
    setOpen(false);
    setActiveSubmenu(null);
  }, []);

  useEffect(() => {
    if (!open) return;
    updatePosition();
    requestAnimationFrame(updatePosition);
    const handlePointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (buttonRef.current?.contains(target) || menuRef.current?.contains(target) || submenuRef.current?.contains(target)) return;
      close();
    };
    const handleKeyDown = (event: KeyboardEvent) => event.key === 'Escape' && close();
    document.addEventListener('mousedown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);
    window.addEventListener('resize', updatePosition);
    window.addEventListener('scroll', updatePosition, true);
    return () => {
      document.removeEventListener('mousedown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('resize', updatePosition);
      window.removeEventListener('scroll', updatePosition, true);
    };
  }, [close, open, updatePosition]);

  const definitions = useMemo<Record<Submenu, MenuDefinition>>(() => ({
    ordering: {
      kind: 'single', value: view.ordering, options: ['updated', 'status', 'created', 'name'] as WorkspaceSessionOrdering[], choose: value => view.setOrdering(value as WorkspaceSessionOrdering),
    },
    show: {
      kind: 'single', value: view.show, options: ['all', 'unread', 'attention'] as WorkspaceSessionShow[], choose: value => view.setShow(value as WorkspaceSessionShow),
    },
    status: {
      kind: 'multi', hidden: view.filters.hiddenStatuses, options: ['running', 'attention', 'error', 'completed', 'idle'] as WorkspaceSessionStatus[], toggle: value => view.toggleHiddenStatus(value as WorkspaceSessionStatus),
    },
    worktree: {
      kind: 'multi', hidden: view.filters.hiddenWorktrees, options: ['main', 'worktree'] as WorkspaceSessionWorktree[], toggle: value => view.toggleHiddenWorktree(value as WorkspaceSessionWorktree),
    },
    environment: {
      kind: 'multi', hidden: view.filters.hiddenEnvironments, options: ['local', 'remote', 'detached'] as WorkspaceSessionEnvironment[], toggle: value => view.toggleHiddenEnvironment(value as WorkspaceSessionEnvironment),
    },
    source: {
      kind: 'multi', hidden: view.filters.hiddenSources, options: ['bitfun', 'external'] as WorkspaceSessionSource[], toggle: value => view.toggleHiddenSource(value as WorkspaceSessionSource),
    },
  }), [view]);

  const row = (submenu: Submenu, value?: string, active = false) => (
    <button
      type="button"
      className={activeSubmenu === submenu ? 'is-open' : ''}
      role="menuitem"
      aria-haspopup="menu"
      aria-expanded={activeSubmenu === submenu}
      onMouseEnter={() => setActiveSubmenu(submenu)}
      onFocus={() => setActiveSubmenu(submenu)}
      onClick={() => setActiveSubmenu(submenu)}
    >
      <span>{t(`nav.sessions.viewMenu.${submenu}.label`)}</span>
      <span className="bitfun-nav-panel__session-filter-menu-value">
        {active ? <span className="bitfun-nav-panel__session-filter-active-dot" aria-hidden="true" /> : null}
        {value ? t(`nav.sessions.viewMenu.${submenu}.${value}`) : null}
        <ChevronRight size={16} aria-hidden="true" />
      </span>
    </button>
  );

  const submenuIndex: Record<Submenu, number> = {
    ordering: 0,
    show: 1,
    status: 3,
    worktree: 4,
    environment: 5,
    source: 6,
  };
  const submenuTop = menuPosition.top + 4 + (activeSubmenu ? submenuIndex[activeSubmenu] * ROW_HEIGHT : 0);
  const preferredSubmenuLeft = menuPosition.left + MAIN_MENU_WIDTH + MENU_GAP;
  const submenuLeft = preferredSubmenuLeft + SUBMENU_WIDTH <= window.innerWidth - VIEWPORT_PADDING
    ? preferredSubmenuLeft
    : menuPosition.left - SUBMENU_WIDTH - MENU_GAP;
  const definition = activeSubmenu ? definitions[activeSubmenu] : null;

  const menu = open ? createPortal(
    <>
      <div
        ref={menuRef}
        className="bitfun-nav-panel__session-filter-menu"
        style={menuPosition}
        role="menu"
        aria-label={t('nav.sessions.viewMenu.title')}
        data-testid="nav-session-filter-menu"
      >
        {row('ordering', view.ordering)}
        {row('show')}
        <div className="bitfun-nav-panel__session-filter-menu-divider" role="separator" />
        <div className="bitfun-nav-panel__session-filter-menu-header">
          <span>{t('nav.sessions.viewMenu.filters.label')}</span>
          <button type="button" onClick={view.resetFilters}>{t('nav.sessions.viewMenu.filters.reset')}</button>
        </div>
        {row('status', undefined, view.filters.hiddenStatuses.length > 0)}
        {row('worktree', undefined, view.filters.hiddenWorktrees.length > 0)}
        {row('environment', undefined, view.filters.hiddenEnvironments.length > 0)}
        {row('source', undefined, view.filters.hiddenSources.length > 0)}
        <button
          type="button"
          className={view.filters.hideArchived ? 'is-filtered' : ''}
          role="menuitemcheckbox"
          aria-checked={!view.filters.hideArchived}
          onMouseEnter={() => setActiveSubmenu(null)}
          onClick={view.toggleArchived}
        >
          <span>{t('nav.sessions.viewMenu.archived')}</span>
          {!view.filters.hideArchived ? <Check size={15} aria-hidden="true" /> : null}
        </button>
        <div className="bitfun-nav-panel__session-filter-menu-divider" role="separator" />
        {view.grouping === 'grouped' ? (
          <button
            type="button"
            role="menuitem"
            data-testid="nav-session-collapse-all"
            onMouseEnter={() => setActiveSubmenu(null)}
            onClick={() => { view.requestCollapseAll(); close(); }}
          >
            <span>{t('nav.sessions.viewMenu.collapseAll')}</span>
          </button>
        ) : null}
        <button
          type="button"
          role="menuitem"
          onMouseEnter={() => setActiveSubmenu(null)}
          onClick={() => {
            for (const session of flowChatStore.getState().sessions.values()) {
              if (session.hasUnreadCompletion) flowChatStore.clearSessionUnreadCompletion(session.sessionId);
            }
            close();
          }}
        >
          <span>{t('nav.sessions.viewMenu.markAllRead')}</span>
        </button>
      </div>

      {activeSubmenu && definition ? (
        <div
          ref={submenuRef}
          className="bitfun-nav-panel__session-filter-submenu"
          style={{
            top: clamp(submenuTop, VIEWPORT_PADDING, window.innerHeight - (definition.options.length * ROW_HEIGHT + 8) - VIEWPORT_PADDING),
            left: clamp(submenuLeft, VIEWPORT_PADDING, window.innerWidth - SUBMENU_WIDTH - VIEWPORT_PADDING),
          }}
          role="menu"
          aria-label={t(`nav.sessions.viewMenu.${activeSubmenu}.label`)}
          data-testid={`nav-session-filter-${activeSubmenu}-menu`}
        >
          {definition.options.map(option => {
            const selected = definition.kind === 'single'
              ? option === definition.value
              : !definition.hidden.includes(option);
            return (
              <button
                key={option}
                type="button"
                className={selected ? 'is-selected' : ''}
                role={definition.kind === 'single' ? 'menuitemradio' : 'menuitemcheckbox'}
                aria-checked={selected}
                onClick={() => {
                  if (definition.kind === 'single') {
                    definition.choose(option);
                    close();
                  } else {
                    definition.toggle(option);
                  }
                }}
              >
                <span className="bitfun-nav-panel__session-filter-check" aria-hidden="true">{selected ? <Check size={15} /> : null}</span>
                <span>{t(`nav.sessions.viewMenu.${activeSubmenu}.${option}`)}</span>
              </button>
            );
          })}
        </div>
      ) : null}
    </>,
    getAppearanceOverlayHost(),
  ) : null;

  return (
    <>
      <Tooltip content={t('nav.sessions.viewMenu.tooltip')} placement="right" followCursor disabled={open}>
        <button
          ref={buttonRef}
          type="button"
          className={`bitfun-nav-panel__section-action${open || isCustomized ? ' is-active' : ''}`}
          data-bf-action="session-filter"
          data-bf-state={[open && 'open', isCustomized && 'filtered'].filter(Boolean).join(' ') || undefined}
          aria-label={t('nav.sessions.viewMenu.tooltip')}
          aria-haspopup="menu"
          aria-expanded={open}
          onClick={() => setOpen(current => !current)}
          data-testid="nav-session-filter-btn"
        >
          <ListFilter size={13} />
        </button>
      </Tooltip>
      {menu}
    </>
  );
};

export default WorkspaceSessionFilterMenu;
