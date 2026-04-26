import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { FolderOpen, User, MessageSquare } from 'lucide-react';
import { Search } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import { useApp } from '@/app/hooks/useApp';
import { useMyAgentStore } from '@/app/scenes/my-agent/myAgentStore';
import { useNurseryStore } from '@/app/scenes/profile/nurseryStore';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { findWorkspaceForSession } from '@/flow_chat/utils/workspaceScope';
import { openMainSession } from '@/flow_chat/services/openBtwSession';
import type { FlowChatState, Session } from '@/flow_chat/types/flow-chat';
import type { SessionMetadata } from '@/shared/types/session-history';
import type { WorkspaceInfo } from '@/shared/types';
import { sessionAPI } from '@/infrastructure/api';
import { WorkspaceKind } from '@/shared/types';
import { i18nService } from '@/infrastructure/i18n';
import { resolvePersistedSessionTitle, resolveSessionTitle } from '@/flow_chat/utils/sessionTitle';
import {
  compareSessionsForDisplay,
  getSessionMetadataSortTimestamp,
  getSessionSortTimestamp,
} from '@/flow_chat/utils/sessionOrdering';
import './NavSearchDialog.scss';

interface NavSearchDialogProps {
  open: boolean;
  onClose: () => void;
}

type ResultKind = 'workspace' | 'assistant' | 'session';

interface SearchResultItem {
  kind: ResultKind;
  id: string;
  label: string;
  sublabel?: string;
  workspaceId?: string;
}

const MAX_PER_GROUP = 20;

const getTitle = (session: Session): string =>
  resolveSessionTitle(session, (key, options) => i18nService.t(key, options));

const matchesQuery = (query: string, ...fields: (string | undefined | null)[]): boolean => {
  const q = query.toLowerCase();
  return fields.some(f => f && f.toLowerCase().includes(q));
};

const NavSearchDialog: React.FC<NavSearchDialogProps> = ({ open, onClose }) => {
  const { t } = useI18n('common');
  const { openedWorkspacesList, assistantWorkspacesList, setActiveWorkspace } = useWorkspaceContext();
  const { openScene } = useSceneManager();
  const { switchLeftPanelTab } = useApp();
  const setSelectedAssistantWorkspaceId = useMyAgentStore(s => s.setSelectedAssistantWorkspaceId);
  const openNurseryAssistant = useNurseryStore(s => s.openAssistant);
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  /** Persisted session rows for opened workspaces — filled when dialog opens (search filters client-side). */
  const [persistedOpenWorkspaceSessions, setPersistedOpenWorkspaceSessions] = useState<
    Array<{ meta: SessionMetadata; workspace: WorkspaceInfo }>
  >([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const unsub = flowChatStore.subscribe(s => setFlowChatState(s));
    return () => unsub();
  }, [open]);

  useEffect(() => {
    if (open) {
      setQuery('');
      setActiveIndex(0);
    }
  }, [open]);

  useEffect(() => {
    if (!open) {
      setPersistedOpenWorkspaceSessions([]);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const rows: Array<{ meta: SessionMetadata; workspace: WorkspaceInfo }> = [];
        for (const w of openedWorkspacesList) {
          const list = await sessionAPI.listSessions(
            w.rootPath,
            w.connectionId ?? undefined,
            w.sshHost ?? undefined
          );
          for (const meta of list) {
            rows.push({ meta, workspace: w });
          }
        }
        if (!cancelled) {
          setPersistedOpenWorkspaceSessions(rows);
        }
      } catch {
        if (!cancelled) {
          setPersistedOpenWorkspaceSessions([]);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, openedWorkspacesList]);

  const projectWorkspaces = useMemo(
    () => openedWorkspacesList.filter(w => w.workspaceKind !== WorkspaceKind.Assistant),
    [openedWorkspacesList]
  );

  const openedWorkspaceIdSet = useMemo(
    () => new Set(openedWorkspacesList.map(w => w.id)),
    [openedWorkspacesList]
  );

  /** Sessions that resolve to an opened workspace (project + assistant rows in the nav). */
  const sessionsInOpenedWorkspaces = useMemo((): Array<{ session: Session; workspace: WorkspaceInfo }> => {
    const result: Array<{ session: Session; workspace: WorkspaceInfo }> = [];
    for (const session of flowChatState.sessions.values()) {
      const workspace = findWorkspaceForSession(session, openedWorkspacesList);
      if (workspace && openedWorkspaceIdSet.has(workspace.id)) {
        result.push({ session, workspace });
      }
    }
    result.sort((a, b) => compareSessionsForDisplay(a.session, b.session));
    return result;
  }, [flowChatState.sessions, openedWorkspacesList, openedWorkspaceIdSet]);

  const mainLineSessionsOpen = useMemo(
    () => sessionsInOpenedWorkspaces.filter(({ session }) => !session.parentSessionId),
    [sessionsInOpenedWorkspaces]
  );

  const results = useMemo((): SearchResultItem[] => {
    const items: SearchResultItem[] = [];
    const q = query.trim();

    if (!q) {
      for (const w of projectWorkspaces.slice(0, MAX_PER_GROUP)) {
        items.push({ kind: 'workspace', id: w.id, label: w.name, sublabel: w.rootPath });
      }
      for (const w of assistantWorkspacesList.slice(0, MAX_PER_GROUP)) {
        const displayName = w.identity?.name?.trim() || w.name;
        items.push({ kind: 'assistant', id: w.id, label: displayName, sublabel: w.description });
      }
      return items;
    }

    const filteredWorkspaces = projectWorkspaces
      .filter(w => matchesQuery(q, w.name, w.rootPath))
      .slice(0, MAX_PER_GROUP);
    for (const w of filteredWorkspaces) {
      items.push({ kind: 'workspace', id: w.id, label: w.name, sublabel: w.rootPath });
    }

    const filteredAssistants = assistantWorkspacesList
      .filter(w => matchesQuery(q, w.name, w.identity?.name, w.description))
      .slice(0, MAX_PER_GROUP);
    for (const w of filteredAssistants) {
      const displayName = w.identity?.name?.trim() || w.name;
      items.push({ kind: 'assistant', id: w.id, label: displayName, sublabel: w.description });
    }

    const storeMatches = mainLineSessionsOpen.filter(({ session }) =>
      matchesQuery(q, getTitle(session), session.sessionId)
    );
    const storeIds = new Set(storeMatches.map(({ session }) => session.sessionId));

    const diskMatches = persistedOpenWorkspaceSessions.filter(({ meta, workspace }) => {
      if (!openedWorkspaceIdSet.has(workspace.id)) return false;
      if (meta.customMetadata?.parentSessionId) return false;
      const label = resolvePersistedSessionTitle(meta, (key, options) =>
        i18nService.t(key, options)
      );
      if (!matchesQuery(q, label, meta.sessionId)) return false;
      return !storeIds.has(meta.sessionId);
    });

    const merged: Array<{ session: Session; workspace: WorkspaceInfo } | { disk: SessionMetadata; workspace: WorkspaceInfo }> = [
      ...storeMatches.map(({ session, workspace }) => ({ session, workspace })),
      ...diskMatches.map(({ meta, workspace }) => ({ disk: meta, workspace })),
    ];
    merged.sort((a, b) => {
      const ta =
        'session' in a
          ? getSessionSortTimestamp(a.session)
          : getSessionMetadataSortTimestamp(a.disk);
      const tb =
        'session' in b
          ? getSessionSortTimestamp(b.session)
          : getSessionMetadataSortTimestamp(b.disk);
      const timestampDiff = tb - ta;
      if (timestampDiff !== 0) return timestampDiff;

      const createdAtA = 'session' in a ? a.session.createdAt : a.disk.createdAt;
      const createdAtB = 'session' in b ? b.session.createdAt : b.disk.createdAt;
      const createdAtDiff = createdAtB - createdAtA;
      if (createdAtDiff !== 0) return createdAtDiff;

      const sessionIdA = 'session' in a ? a.session.sessionId : a.disk.sessionId;
      const sessionIdB = 'session' in b ? b.session.sessionId : b.disk.sessionId;
      return sessionIdA.localeCompare(sessionIdB);
    });

    for (const entry of merged.slice(0, MAX_PER_GROUP)) {
      if ('session' in entry) {
        const { session, workspace } = entry;
        items.push({
          kind: 'session',
          id: session.sessionId,
          label: getTitle(session),
          sublabel: t('nav.search.sessionWorkspaceHint', { workspace: workspace.name }),
          workspaceId: workspace.id,
        });
      } else {
        const { disk: meta, workspace } = entry;
        items.push({
          kind: 'session',
          id: meta.sessionId,
          label: resolvePersistedSessionTitle(meta, (key, options) =>
            i18nService.t(key, options)
          ),
          sublabel: t('nav.search.sessionWorkspaceHint', { workspace: workspace.name }),
          workspaceId: workspace.id,
        });
      }
    }

    return items;
  }, [
    query,
    projectWorkspaces,
    assistantWorkspacesList,
    mainLineSessionsOpen,
    persistedOpenWorkspaceSessions,
    openedWorkspaceIdSet,
    t,
  ]);

  useEffect(() => {
    setActiveIndex(0);
  }, [results.length]);

  const handleSelect = useCallback(async (item: SearchResultItem) => {
    onClose();
    if (item.kind === 'workspace') {
      await setActiveWorkspace(item.id);
    } else if (item.kind === 'assistant') {
      setSelectedAssistantWorkspaceId(item.id);
      openNurseryAssistant(item.id);
      await setActiveWorkspace(item.id).catch(() => {});
      switchLeftPanelTab('profile');
      openScene('assistant');
    } else if (item.kind === 'session') {
      await openMainSession(item.id, {
        workspaceId: item.workspaceId,
        activateWorkspace: item.workspaceId ? setActiveWorkspace : undefined,
      });
    }
  }, [onClose, setActiveWorkspace, setSelectedAssistantWorkspaceId, openNurseryAssistant, switchLeftPanelTab, openScene]);

  // Passed to Search component's onKeyDown — called before its built-in handling.
  // Use e.preventDefault() to suppress Search's own Enter/Escape logic when needed.
  const handleInputKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex(i => Math.min(i + 1, Math.max(0, results.length - 1)));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex(i => Math.max(i - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const item = results[activeIndex];
      if (item) void handleSelect(item);
    }
  }, [activeIndex, handleSelect, onClose, results]);

  // Scroll active item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const active = list.querySelector<HTMLButtonElement>('.bitfun-nav-search-dialog__item--active');
    active?.scrollIntoView({ block: 'nearest' });
  }, [activeIndex]);

  if (!open) return null;

  const workspaceItems = results.filter(r => r.kind === 'workspace');
  const assistantItems = results.filter(r => r.kind === 'assistant');
  const sessionItems = results.filter(r => r.kind === 'session');
  const queryTrimmed = query.trim();
  const showDefaultSessionColumn = !queryTrimmed;

  let globalIndex = 0;
  const renderGroup = (
    groupLabel: string,
    items: SearchResultItem[],
    icon: (item: SearchResultItem) => React.ReactNode
  ) => {
    if (items.length === 0) return null;
    const startIndex = globalIndex;
    globalIndex += items.length;
    return (
      <div className="bitfun-nav-search-dialog__group" key={groupLabel}>
        <div className="bitfun-nav-search-dialog__group-label">{groupLabel}</div>
        {items.map((item, i) => {
          const idx = startIndex + i;
          return (
            <button
              key={item.id}
              type="button"
              className={`bitfun-nav-search-dialog__item${idx === activeIndex ? ' bitfun-nav-search-dialog__item--active' : ''}`}
              onMouseEnter={() => setActiveIndex(idx)}
              onClick={() => void handleSelect(item)}
            >
              <span className="bitfun-nav-search-dialog__item-icon">{icon(item)}</span>
              <span className="bitfun-nav-search-dialog__item-content">
                <span className="bitfun-nav-search-dialog__item-label">{item.label}</span>
                {item.sublabel && (
                  <span className="bitfun-nav-search-dialog__item-sublabel">{item.sublabel}</span>
                )}
              </span>
            </button>
          );
        })}
      </div>
    );
  };

  const dialog = (
    <div className="bitfun-nav-search-dialog__overlay" onMouseDown={e => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="bitfun-nav-search-dialog__card" ref={cardRef}>
        <div className="bitfun-nav-search-dialog__input-row">
          <Search
            ref={inputRef}
            className="bitfun-nav-search-dialog__search"
            placeholder={t('nav.search.inputPlaceholder')}
            value={query}
            onChange={setQuery}
            onClear={() => setQuery('')}
            onKeyDown={handleInputKeyDown}
            clearable
            size="medium"
            autoFocus
          />
        </div>
        <div className="bitfun-nav-search-dialog__results" ref={listRef}>
          {results.length === 0 && !showDefaultSessionColumn ? (
            <div className="bitfun-nav-search-dialog__empty">{t('nav.search.empty')}</div>
          ) : (
            <>
              {renderGroup(t('nav.search.groupWorkspaces'), workspaceItems, () => <FolderOpen size={14} />)}
              {renderGroup(t('nav.search.groupAssistants'), assistantItems, () => <User size={14} />)}
              {showDefaultSessionColumn ? (
                <div className="bitfun-nav-search-dialog__group" key="nav-search-sessions-default">
                  <div className="bitfun-nav-search-dialog__group-label">{t('nav.search.groupSessions')}</div>
                  <div className="bitfun-nav-search-dialog__session-hint" role="status">
                    {t('nav.search.sessionSearchHintDefault')}
                  </div>
                </div>
              ) : (
                renderGroup(t('nav.search.groupSessions'), sessionItems, () => <MessageSquare size={14} />)
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );

  return createPortal(dialog, document.body);
};

export default NavSearchDialog;
