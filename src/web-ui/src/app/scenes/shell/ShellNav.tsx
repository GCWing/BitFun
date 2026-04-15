import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Plus,
  ChevronDown,
  RefreshCw,
  Play,
  Pencil,
  Square,
  Trash2,
} from 'lucide-react';
import { useI18n } from '@/infrastructure/i18n';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { TerminalConfig } from '@/infrastructure/config/types';
import TerminalEditModal from '@/app/components/panels/TerminalEditModal';
import { useContextMenuStore } from '@/shared/context-menu-system';
import { ContextType } from '@/shared/context-menu-system/types/context.types';
import type { MenuItem } from '@/shared/context-menu-system/types/menu.types';
import { useOverlayStore } from '@/app/stores/overlayStore';
import { useTerminalSceneStore } from '@/app/stores/terminalSceneStore';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { getTerminalService } from '@/tools/terminal';
import type { ShellInfo } from '@/tools/terminal';
import { useShellStore } from './shellStore';
import { useShellEntries } from './hooks';
import type { ShellEntry } from './hooks/shellEntryTypes';
import { useShellNavMenuState } from './hooks/useShellNavMenuState';
import { Button } from '@/component-library/components/Button';
import { Tooltip } from '@/component-library/components/Tooltip';
import ShellNavEntryItem from './components/ShellNavEntryItem';
import ShellNavWorkspaceSwitcher from './components/ShellNavWorkspaceSwitcher';
import './ShellNav.scss';

function formatShellMenuLabel(shell: ShellInfo, isDefault: boolean, defaultBadgeLabel: string): string {
  const base = shell.version ? `${shell.name} (${shell.version})` : shell.name;
  return isDefault ? `${base} · ${defaultBadgeLabel}` : base;
}

const ShellNav: React.FC = () => {
  const { t } = useI18n('common');
  const { activeWorkspace, openedWorkspacesList, workspaceName, setActiveWorkspace } = useWorkspaceContext();
  const navView = useShellStore((s) => s.navView);
  const setNavView = useShellStore((s) => s.setNavView);
  const activeOverlay = useOverlayStore((s) => s.activeOverlay);
  const activeSceneId = activeOverlay ?? 'session';
  const activeTerminalSessionId = useTerminalSceneStore((s) => s.activeSessionId);
  const showMenu = useContextMenuStore((s) => s.showMenu);
  const [availableShells, setAvailableShells] = useState<ShellInfo[]>([]);
  const [defaultShellType, setDefaultShellType] = useState<string>('');

  const {
    manualEntries,
    agentEntries,
    editModalOpen,
    editingTerminal,
    closeEditModal,
    refresh: refreshEntries,
    createManualTerminal,
    openEntry,
    stopEntry,
    deleteEntry,
    openEditModal,
    saveEdit,
  } = useShellEntries();

  const visibleEntries = navView === 'agent' ? agentEntries : manualEntries;
  const hasMultipleWorkspaces = openedWorkspacesList.length > 1;
  const hasVisibleContent = visibleEntries.length > 0;
  const {
    menuOpen,
    setMenuOpen,
    workspaceMenuOpen,
    setWorkspaceMenuOpen,
    workspaceMenuPosition,
    menuRef,
    workspaceMenuRef,
    workspaceTriggerRef,
  } = useShellNavMenuState(hasMultipleWorkspaces);

  const loadAvailableShells = useCallback(async () => {
    try {
      const [shells, terminalConfig] = await Promise.all([
        getTerminalService().getAvailableShells(),
        configManager.getConfig<TerminalConfig>('terminal'),
      ]);
      setAvailableShells(shells.filter((shell) => shell.available));
      setDefaultShellType(terminalConfig?.default_shell || '');
    } catch {
      setAvailableShells([]);
      setDefaultShellType('');
    }
  }, []);

  useEffect(() => {
    void loadAvailableShells();
  }, [loadAvailableShells]);

  const handleRefresh = useCallback(async () => {
    await Promise.all([
      refreshEntries(),
      loadAvailableShells(),
    ]);
  }, [loadAvailableShells, refreshEntries]);

  const handleCreateManualTerminal = useCallback(async (shellType?: string) => {
    setMenuOpen(false);
    await createManualTerminal(shellType);
  }, [createManualTerminal, setMenuOpen]);

  const handleToggleCreateMenu = useCallback(() => {
    setWorkspaceMenuOpen(false);
    setMenuOpen((prev) => !prev);
  }, [setMenuOpen, setWorkspaceMenuOpen]);

  const shellMenuItems = useMemo(
    () =>
      availableShells.map((shell) => ({
        key: shell.shellType,
        label: formatShellMenuLabel(
          shell,
          shell.shellType === defaultShellType,
          t('nav.shell.badges.default'),
        ),
        shellType: shell.shellType,
      })),
    [availableShells, defaultShellType, t],
  );

  const handleToggleWorkspaceMenu = useCallback(() => {
    if (!hasMultipleWorkspaces) {
      return;
    }

    setMenuOpen(false);
    setWorkspaceMenuOpen((prev) => !prev);
  }, [hasMultipleWorkspaces, setMenuOpen, setWorkspaceMenuOpen]);

  const handleSelectWorkspace = useCallback(async (workspaceId: string) => {
    setWorkspaceMenuOpen(false);
    if (workspaceId === activeWorkspace?.id) {
      return;
    }
    await setActiveWorkspace(workspaceId);
  }, [activeWorkspace?.id, setActiveWorkspace, setWorkspaceMenuOpen]);

  const openContextMenu = useCallback((
    event: React.MouseEvent<HTMLElement>,
    items: MenuItem[],
    data: Record<string, unknown>,
  ) => {
    event.preventDefault();
    event.stopPropagation();

    showMenu(
      { x: event.clientX, y: event.clientY },
      items,
      {
        type: ContextType.CUSTOM,
        customType: 'shell-nav',
        data,
        event,
        targetElement: event.currentTarget,
        position: { x: event.clientX, y: event.clientY },
        timestamp: Date.now(),
      },
    );
  }, [showMenu]);

  const getEntryMenuItems = useCallback((entry: ShellEntry): MenuItem[] => {
    if (entry.kind === 'manual-profile') {
      return [
        !entry.isRunning
          ? {
              id: `start-${entry.sessionId}`,
              label: t('nav.shell.context.start'),
              icon: <Play size={14} />,
              onClick: async () => {
                await openEntry(entry);
              },
            }
          : {
              id: `stop-${entry.sessionId}`,
              label: t('nav.shell.context.stop'),
              icon: <Square size={14} />,
              onClick: async () => {
                await stopEntry(entry);
              },
            },
        {
          id: `edit-${entry.sessionId}`,
          label: t('nav.shell.context.editConfig'),
          icon: <Pencil size={14} />,
          onClick: () => {
            openEditModal(entry);
          },
        },
        {
          id: `delete-${entry.sessionId}`,
          label: t('nav.shell.context.deleteSavedTerminal'),
          icon: <Trash2 size={14} />,
          onClick: async () => {
            await deleteEntry(entry);
          },
        },
      ];
    }

    if (entry.kind === 'agent-session') {
      return [];
    }

    return [{
        id: `config-${entry.sessionId}`,
        label: t('nav.shell.context.saveConfig'),
        icon: <Pencil size={14} />,
        onClick: () => {
          openEditModal(entry);
        },
      }];
  }, [deleteEntry, openEditModal, openEntry, stopEntry, t]);

  const getQuickAction = useCallback((entry: ShellEntry) => {
    if (entry.isRunning) {
      return {
        icon: <Trash2 size={12} />,
        title: t('nav.shell.context.close'),
        onClick: () => { void deleteEntry(entry); },
      };
    }

    if (entry.isPersisted) {
      return {
        icon: <Trash2 size={12} />,
        title: t('nav.shell.context.deleteSavedTerminal'),
        onClick: () => { void deleteEntry(entry); },
      };
    }

    return {
      icon: <Trash2 size={12} />,
      title: t('nav.shell.context.close'),
      onClick: () => { void deleteEntry(entry); },
    };
  }, [deleteEntry, t]);

  return (
    <div className="bitfun-shell-nav">
      <div className="bitfun-shell-nav__header">
        <div className="bitfun-shell-nav__title-group">
          <span className="bitfun-shell-nav__title">{t('nav.shell.title')}</span>
          <ShellNavWorkspaceSwitcher
            workspaceName={workspaceName}
            hasMultipleWorkspaces={hasMultipleWorkspaces}
            workspaceMenuOpen={workspaceMenuOpen}
            workspaceMenuPosition={workspaceMenuPosition}
            openedWorkspacesList={openedWorkspacesList}
            activeWorkspaceId={activeWorkspace?.id}
            workspaceMenuRef={workspaceMenuRef}
            workspaceTriggerRef={workspaceTriggerRef}
            switchWorkspaceLabel={t('header.switchWorkspace')}
            onToggle={handleToggleWorkspaceMenu}
            onSelectWorkspace={handleSelectWorkspace}
          />
        </div>
        <div className="bitfun-shell-nav__header-actions" ref={menuRef}>
          <div className={`bitfun-shell-nav__split-button${menuOpen ? ' is-active' : ''}`}>
            <Tooltip content={t('nav.shell.actions.newTerminal')} placement="bottom">
              <button
                type="button"
                className="bitfun-shell-nav__split-button-main"
                onClick={() => { void handleCreateManualTerminal(); }}
              >
                <Plus size={14} />
              </button>
            </Tooltip>
            <Tooltip content={t('actions.more')} placement="bottom">
              <button
                type="button"
                className="bitfun-shell-nav__split-button-toggle"
                onClick={handleToggleCreateMenu}
                aria-haspopup="menu"
                aria-expanded={menuOpen}
              >
                <ChevronDown size={12} />
              </button>
            </Tooltip>
          </div>

          {menuOpen ? (
            <div className="bitfun-shell-nav__dropdown-menu" role="menu">
              {shellMenuItems.map((shell) => (
                <button
                  key={shell.key}
                  type="button"
                  className="bitfun-shell-nav__dropdown-item"
                  role="menuitem"
                  onClick={() => { void handleCreateManualTerminal(shell.shellType); }}
                >
                  <Plus size={14} />
                  <span>{shell.label}</span>
                </button>
              ))}
              {shellMenuItems.length > 0 ? <div className="bitfun-shell-nav__dropdown-separator" /> : null}
              <button type="button" className="bitfun-shell-nav__dropdown-item" role="menuitem" onClick={() => { setMenuOpen(false); void handleRefresh(); }}>
                <RefreshCw size={14} />
                <span>{t('nav.shell.actions.refresh')}</span>
              </button>
            </div>
          ) : null}
        </div>
      </div>

      <div className="bitfun-shell-nav__view-toggle" role="tablist" aria-label={t('nav.shell.title')}>
        <button
          type="button"
          role="tab"
          className={`bitfun-shell-nav__view-toggle-btn${navView === 'manual' ? ' is-active' : ''}`}
          aria-selected={navView === 'manual'}
          onClick={() => setNavView('manual')}
        >
          {t('nav.shell.views.manual')}
        </button>
        <button
          type="button"
          role="tab"
          className={`bitfun-shell-nav__view-toggle-btn${navView === 'agent' ? ' is-active' : ''}`}
          aria-selected={navView === 'agent'}
          onClick={() => setNavView('agent')}
        >
          {t('nav.shell.views.agent')}
        </button>
      </div>

      <div
        className={`bitfun-shell-nav__sections${!hasVisibleContent ? ' bitfun-shell-nav__sections--empty' : ''}`}
      >
        {hasVisibleContent ? (
          <div className="bitfun-shell-nav__terminal-list">
            {visibleEntries.map((entry) => (
              <ShellNavEntryItem
                key={entry.sessionId}
                entry={entry}
                isActive={activeSceneId === 'shell' && activeTerminalSessionId === entry.sessionId}
                showSavedBadge={navView === 'manual' && entry.isPersisted}
                startupCommandBadgeLabel={t('nav.shell.badges.startupCommand')}
                savedBadgeLabel={t('nav.shell.badges.saved')}
                quickAction={getQuickAction(entry)}
                getEntryMenuItems={getEntryMenuItems}
                onOpen={openEntry}
                onOpenContextMenu={openContextMenu}
              />
            ))}
          </div>
        ) : (
          <div className="bitfun-shell-nav__empty">
            <p className="bitfun-shell-nav__empty-message">
              {navView === 'agent' ? t('nav.shell.empty.agent') : t('nav.shell.empty.manual')}
            </p>
            <Button
              type="button"
              variant="secondary"
              size="small"
              onClick={() => { void handleCreateManualTerminal(); }}
            >
              <Plus size={14} aria-hidden />
              {t('nav.shell.empty.quickNew')}
            </Button>
          </div>
        )}
      </div>

      {editingTerminal ? (
        <TerminalEditModal
          isOpen={editModalOpen}
          onClose={closeEditModal}
          onSave={saveEdit}
          initialName={editingTerminal.entry.name}
          initialWorkingDirectory={editingTerminal.entry.workingDirectory ?? editingTerminal.entry.cwd ?? ''}
          initialStartupCommand={editingTerminal.entry.startupCommand}
          showWorkingDirectory
          showStartupCommand
        />
      ) : null}
    </div>
  );
};

export default ShellNav;
