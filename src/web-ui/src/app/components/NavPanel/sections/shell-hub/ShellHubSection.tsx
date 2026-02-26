/**
 * ShellHubSection — inline accordion content for the "Shell Hub" nav item.
 *
 * Provides full Terminal Workshop functionality inside the nav panel:
 *   • Hub terminals (persistent, configurable entries from localStorage)
 *   • Worktree-based terminal grouping
 *   • Create / delete / start / stop hub terminals
 *   • Refresh & add worktree actions
 *
 * Click behavior mirrors ShellsSection:
 *   • Current scene is 'session' → open terminal as AuxPane tab
 *   • Any other scene → switch to terminal scene
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Plus,
  SquareTerminal,
  Circle,
  RefreshCw,
  GitBranch,
  ChevronRight,
  Trash2,
  Edit2,
  Square,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getTerminalService } from '../../../../../tools/terminal';
import type { TerminalService } from '../../../../../tools/terminal';
import type { SessionResponse, TerminalEvent } from '../../../../../tools/terminal/types/session';
import { createTerminalTab } from '../../../../../shared/utils/tabUtils';
import { useTerminalSceneStore } from '../../../../stores/terminalSceneStore';
import { resolveAndFocusOpenTarget } from '../../../../../shared/services/sceneOpenTargetResolver';
import { useCurrentWorkspace } from '../../../../../infrastructure/contexts/WorkspaceContext';
import { configManager } from '../../../../../infrastructure/config/services/ConfigManager';
import type { TerminalConfig } from '../../../../../infrastructure/config/types';
import { gitAPI, type GitWorktreeInfo } from '../../../../../infrastructure/api/service-api/GitAPI';
import { BranchSelectModal, type BranchSelectResult } from '../../../panels/BranchSelectModal';
import { TerminalEditModal } from '../../../panels/TerminalEditModal';
import { Tooltip } from '@/component-library';
import { createLogger } from '@/shared/utils/logger';
import './ShellHubSection.scss';

const log = createLogger('ShellHubSection');

// ── Hub config (shared localStorage schema for terminal hub) ─────────────────

const TERMINAL_HUB_STORAGE_KEY = 'bitfun-terminal-hub-config';
const HUB_TERMINAL_ID_PREFIX = 'hub_';

interface HubTerminalEntry {
  sessionId: string;
  name: string;
  startupCommand?: string;
}

interface HubConfig {
  terminals: HubTerminalEntry[];
  worktrees: Record<string, HubTerminalEntry[]>;
}

function loadHubConfig(workspacePath: string): HubConfig {
  try {
    const raw = localStorage.getItem(`${TERMINAL_HUB_STORAGE_KEY}:${workspacePath}`);
    if (raw) return JSON.parse(raw) as HubConfig;
  } catch {}
  return { terminals: [], worktrees: {} };
}

function saveHubConfig(workspacePath: string, config: HubConfig) {
  try {
    localStorage.setItem(`${TERMINAL_HUB_STORAGE_KEY}:${workspacePath}`, JSON.stringify(config));
  } catch (err) {
    log.error('Failed to save hub config', err);
  }
}

const generateHubTerminalId = () =>
  `${HUB_TERMINAL_ID_PREFIX}${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;

const ShellHubSection: React.FC = () => {
  const { t } = useTranslation('panels/terminal');
  const setActiveSession = useTerminalSceneStore(s => s.setActiveSession);
  const { workspacePath } = useCurrentWorkspace();

  const [sessions, setSessions] = useState<SessionResponse[]>([]);
  const [hubConfig, setHubConfig] = useState<HubConfig>({ terminals: [], worktrees: {} });
  const [worktrees, setWorktrees] = useState<GitWorktreeInfo[]>([]);
  const [isGitRepo, setIsGitRepo] = useState(false);
  const [expandedWorktrees, setExpandedWorktrees] = useState<Set<string>>(new Set());
  const [branchModalOpen, setBranchModalOpen] = useState(false);
  const [currentBranch, setCurrentBranch] = useState<string | undefined>();
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [editingTerminal, setEditingTerminal] = useState<{
    terminal: HubTerminalEntry;
    worktreePath?: string;
  } | null>(null);

  const serviceRef = useRef<TerminalService | null>(null);

  const runningIds = useMemo(() => new Set(sessions.map(s => s.id)), [sessions]);
  const isRunning = useCallback((id: string) => runningIds.has(id), [runningIds]);

  const refreshSessions = useCallback(async () => {
    const service = serviceRef.current;
    if (!service) return;
    try {
      setSessions(await service.listSessions());
    } catch (err) {
      log.error('Failed to list sessions', err);
    }
  }, []);

  useEffect(() => {
    const service = getTerminalService();
    serviceRef.current = service;

    const init = async () => {
      try {
        await service.connect();
        await refreshSessions();
      } catch (err) {
        log.error('Failed to connect terminal service', err);
      }
    };
    init();

    const unsub = service.onEvent((event: TerminalEvent) => {
      if (event.type === 'ready' || event.type === 'exit') {
        refreshSessions();
      }
    });

    return () => unsub();
  }, [refreshSessions]);

  const refreshWorktrees = useCallback(async () => {
    if (!workspacePath) return;
    try {
      const wtList = await gitAPI.listWorktrees(workspacePath);
      setWorktrees(wtList);
      try {
        const branches = await gitAPI.getBranches(workspacePath, false);
        const current = branches.find(b => b.current);
        setCurrentBranch(current?.name);
      } catch {
        setCurrentBranch(undefined);
      }
    } catch (err) {
      log.error('Failed to load worktrees', err);
    }
  }, [workspacePath]);

  const checkGitAndLoadWorktrees = useCallback(async () => {
    if (!workspacePath) return;
    try {
      const repo = await gitAPI.isGitRepository(workspacePath);
      setIsGitRepo(repo);
      if (repo) await refreshWorktrees();
    } catch {
      setIsGitRepo(false);
    }
  }, [workspacePath, refreshWorktrees]);

  useEffect(() => {
    if (!workspacePath) return;
    setHubConfig(loadHubConfig(workspacePath));
    checkGitAndLoadWorktrees();
  }, [workspacePath, checkGitAndLoadWorktrees]);



  const startHubTerminal = useCallback(
    async (entry: HubTerminalEntry, worktreePath?: string): Promise<boolean> => {
      const service = serviceRef.current;
      if (!service || !workspacePath) return false;

      try {
        let shellType: string | undefined;
        try {
          const cfg = await configManager.getConfig<TerminalConfig>('terminal');
          if (cfg?.default_shell) shellType = cfg.default_shell;
        } catch {}

        await service.createSession({
          sessionId: entry.sessionId,
          workingDirectory: worktreePath ?? workspacePath,
          name: entry.name,
          shellType,
        });

        if (entry.startupCommand?.trim()) {
          await new Promise(r => setTimeout(r, 800));
          try {
            await service.sendCommand(entry.sessionId, entry.startupCommand);
          } catch {}
        }

        await refreshSessions();
        return true;
      } catch (err) {
        log.error('Failed to start hub terminal', err);
        return false;
      }
    },
    [workspacePath, refreshSessions]
  );

  const handleOpen = useCallback(
    async (entry: HubTerminalEntry, worktreePath?: string) => {
      if (!isRunning(entry.sessionId)) {
        const ok = await startHubTerminal(entry, worktreePath);
        if (!ok) return;
      }

      const { mode } = resolveAndFocusOpenTarget('terminal');
      if (mode === 'agent') {
        createTerminalTab(entry.sessionId, entry.name, 'agent');
      } else {
        setActiveSession(entry.sessionId);
      }
    },
    [isRunning, startHubTerminal, setActiveSession]
  );

  const handleAddHubTerminal = useCallback(
    async (worktreePath?: string) => {
      const service = serviceRef.current;
      if (!workspacePath || !service) return;

      const newEntry: HubTerminalEntry = {
        sessionId: generateHubTerminalId(),
        name: `Terminal ${Date.now() % 1000}`,
      };

      setHubConfig(prev => {
        let next: HubConfig;
        if (worktreePath) {
          const existing = prev.worktrees[worktreePath] || [];
          next = { ...prev, worktrees: { ...prev.worktrees, [worktreePath]: [...existing, newEntry] } };
        } else {
          next = { ...prev, terminals: [...prev.terminals, newEntry] };
        }
        saveHubConfig(workspacePath, next);
        return next;
      });

      try {
        let shellType: string | undefined;
        try {
          const cfg = await configManager.getConfig<TerminalConfig>('terminal');
          if (cfg?.default_shell) shellType = cfg.default_shell;
        } catch {}

        await service.createSession({
          sessionId: newEntry.sessionId,
          workingDirectory: worktreePath ?? workspacePath,
          name: newEntry.name,
          shellType,
        });
        createTerminalTab(newEntry.sessionId, newEntry.name);
        refreshSessions();
      } catch (err) {
        log.error('Failed to auto-start terminal', err);
      }
    },
    [workspacePath, refreshSessions]
  );

  const handleStopTerminal = useCallback(
    async (sessionId: string, e: React.MouseEvent) => {
      e.stopPropagation();
      const service = serviceRef.current;
      if (!service || !isRunning(sessionId)) return;

      try {
        await service.closeSession(sessionId);
        window.dispatchEvent(
          new CustomEvent('terminal-session-destroyed', { detail: { sessionId } })
        );
        refreshSessions();
      } catch (err) {
        log.error('Failed to stop terminal', err);
      }
    },
    [isRunning, refreshSessions]
  );

  const handleDeleteHubTerminal = useCallback(
    async (entry: HubTerminalEntry, worktreePath: string | undefined, e: React.MouseEvent) => {
      e.stopPropagation();
      const service = serviceRef.current;
      if (!workspacePath) return;

      if (isRunning(entry.sessionId) && service) {
        try {
          await service.closeSession(entry.sessionId);
          window.dispatchEvent(
            new CustomEvent('terminal-session-destroyed', { detail: { sessionId: entry.sessionId } })
          );
        } catch {}
      }

      setHubConfig(prev => {
        let next: HubConfig;
        if (worktreePath) {
          const terms = (prev.worktrees[worktreePath] || []).filter(t => t.sessionId !== entry.sessionId);
          next = { ...prev, worktrees: { ...prev.worktrees, [worktreePath]: terms } };
        } else {
          next = { ...prev, terminals: prev.terminals.filter(t => t.sessionId !== entry.sessionId) };
        }
        saveHubConfig(workspacePath, next);
        return next;
      });
    },
    [workspacePath, isRunning]
  );

  const handleOpenEditModal = useCallback(
    (terminal: HubTerminalEntry, worktreePath: string | undefined, e: React.MouseEvent) => {
      e.stopPropagation();
      setEditingTerminal({ terminal, worktreePath });
      setEditModalOpen(true);
    },
    []
  );

  const handleSaveTerminalEdit = useCallback(
    (newName: string, newStartupCommand?: string) => {
      if (!editingTerminal || !workspacePath) return;
      const { terminal, worktreePath } = editingTerminal;

      setHubConfig(prev => {
        let next: HubConfig;
        if (worktreePath) {
          const terms = (prev.worktrees[worktreePath] || []).map(t =>
            t.sessionId === terminal.sessionId ? { ...t, name: newName, startupCommand: newStartupCommand } : t
          );
          next = { ...prev, worktrees: { ...prev.worktrees, [worktreePath]: terms } };
        } else {
          const terms = prev.terminals.map(t =>
            t.sessionId === terminal.sessionId ? { ...t, name: newName, startupCommand: newStartupCommand } : t
          );
          next = { ...prev, terminals: terms };
        }
        saveHubConfig(workspacePath, next);
        return next;
      });

      if (isRunning(terminal.sessionId)) {
        setSessions(prev => prev.map(s => (s.id === terminal.sessionId ? { ...s, name: newName } : s)));
        window.dispatchEvent(
          new CustomEvent('terminal-session-renamed', {
            detail: { sessionId: terminal.sessionId, newName },
          })
        );
      }

      setEditingTerminal(null);
    },
    [editingTerminal, workspacePath, isRunning]
  );

  const toggleWorktree = useCallback((path: string) => {
    setExpandedWorktrees(prev => {
      const next = new Set(prev);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    });
  }, []);

  const handleAddWorktree = useCallback(() => {
    if (!isGitRepo) return;
    setBranchModalOpen(true);
  }, [isGitRepo]);

  const handleBranchSelect = useCallback(
    async (result: BranchSelectResult) => {
      if (!workspacePath) return;
      try {
        await gitAPI.addWorktree(workspacePath, result.branch, result.isNew);
        await refreshWorktrees();
      } catch (err) {
        log.error('Failed to add worktree', err);
      }
    },
    [workspacePath, refreshWorktrees]
  );

  const handleRefresh = useCallback(async () => {
    await refreshSessions();
    if (workspacePath) {
      setHubConfig(loadHubConfig(workspacePath));
      await checkGitAndLoadWorktrees();
    }
  }, [workspacePath, refreshSessions, checkGitAndLoadWorktrees]);

  const nonMainWorktrees = useMemo(
    () => worktrees.filter(wt => !wt.isMain),
    [worktrees]
  );

  const renderTerminalItem = (entry: HubTerminalEntry, worktreePath?: string) => {
    const running = isRunning(entry.sessionId);

    return (
      <button
        key={entry.sessionId}
        type="button"
        className="bitfun-nav-panel__inline-item"
        onClick={() => handleOpen(entry, worktreePath)}
        title={entry.name}
      >
        <SquareTerminal size={12} className="bitfun-nav-panel__inline-item-icon" />
        <span className="bitfun-nav-panel__inline-item-label">{entry.name}</span>
        <Circle
          size={6}
          className={`bitfun-nav-panel__shell-dot ${running ? 'is-running' : 'is-stopped'}`}
        />
        <div className="bitfun-nav-panel__inline-item-actions">
          <Tooltip content={t('actions.edit')}>
            <button
              type="button"
              className="bitfun-nav-panel__inline-item-action-btn"
              onClick={(e) => handleOpenEditModal(entry, worktreePath, e)}
            >
              <Edit2 size={10} />
            </button>
          </Tooltip>
          {running && (
            <Tooltip content={t('actions.stopTerminal')}>
              <button
                type="button"
                className="bitfun-nav-panel__inline-item-action-btn"
                onClick={(e) => handleStopTerminal(entry.sessionId, e)}
              >
                <Square size={10} />
              </button>
            </Tooltip>
          )}
          <Tooltip content={t('actions.deleteTerminal')}>
            <button
              type="button"
              className="bitfun-nav-panel__inline-item-action-btn delete"
              onClick={(e) => handleDeleteHubTerminal(entry, worktreePath, e)}
            >
              <Trash2 size={10} />
            </button>
          </Tooltip>
        </div>
      </button>
    );
  };

  const hasContent = hubConfig.terminals.length > 0 || nonMainWorktrees.length > 0;

  return (
    <div className="bitfun-nav-panel__inline-list bitfun-nav-panel__inline-list--shell-hub">
      {/* Action bar: New + Refresh + Worktree */}
      <div className="shell-hub-actions">
        <Tooltip content={t('actions.newTerminal')}>
          <button type="button" className="shell-hub-actions__icon-btn" onClick={() => handleAddHubTerminal()}>
            <Plus size={12} />
          </button>
        </Tooltip>
        <Tooltip content={t('actions.refresh')}>
          <button type="button" className="shell-hub-actions__icon-btn" onClick={handleRefresh}>
            <RefreshCw size={12} />
          </button>
        </Tooltip>
        {isGitRepo && (
          <Tooltip content={t('actions.newWorktree')}>
            <button type="button" className="shell-hub-actions__icon-btn" onClick={handleAddWorktree}>
              <GitBranch size={12} />
            </button>
          </Tooltip>
        )}
      </div>

      {/* Hub terminals (main workspace) */}
      {hubConfig.terminals.map(entry => renderTerminalItem(entry))}

      {/* Worktree groups */}
      {nonMainWorktrees.map(wt => {
        const expanded = expandedWorktrees.has(wt.path);
        const terms = hubConfig.worktrees[wt.path] || [];
        const branchLabel = wt.branch || wt.path.split(/[/\\]/).pop();

        return (
          <div key={wt.path} className="shell-hub-worktree">
            <button
              type="button"
              className="shell-hub-worktree__header"
              onClick={() => toggleWorktree(wt.path)}
            >
              <ChevronRight
                size={10}
                className={`shell-hub-worktree__chevron${expanded ? ' is-expanded' : ''}`}
              />
              <GitBranch size={11} className="shell-hub-worktree__icon" />
              <span className="shell-hub-worktree__label">{branchLabel}</span>
              <span className="bitfun-nav-panel__inline-item-badge">{terms.length}</span>
              <div className="bitfun-nav-panel__inline-item-actions">
                <Tooltip content={t('actions.newTerminal')}>
                  <button
                    type="button"
                    className="bitfun-nav-panel__inline-item-action-btn"
                    onClick={(e) => { e.stopPropagation(); handleAddHubTerminal(wt.path); }}
                  >
                    <Plus size={10} />
                  </button>
                </Tooltip>
              </div>
            </button>
            {expanded && terms.length > 0 && (
              <div className="shell-hub-worktree__list">
                {terms.map(entry => renderTerminalItem(entry, wt.path))}
              </div>
            )}
          </div>
        );
      })}

      {/* Empty state */}
      {!hasContent && (
        <div className="bitfun-nav-panel__inline-empty">{t('sections.terminalHub')}</div>
      )}

      {/* Modals */}
      {workspacePath && (
        <BranchSelectModal
          isOpen={branchModalOpen}
          onClose={() => setBranchModalOpen(false)}
          onSelect={handleBranchSelect}
          repositoryPath={workspacePath}
          currentBranch={currentBranch}
          existingWorktreeBranches={worktrees.map(wt => wt.branch).filter(Boolean) as string[]}
        />
      )}

      {editingTerminal && (
        <TerminalEditModal
          isOpen={editModalOpen}
          onClose={() => { setEditModalOpen(false); setEditingTerminal(null); }}
          onSave={handleSaveTerminalEdit}
          initialName={editingTerminal.terminal.name}
          initialStartupCommand={editingTerminal.terminal.startupCommand}
        />
      )}
    </div>
  );
};

export default ShellHubSection;
