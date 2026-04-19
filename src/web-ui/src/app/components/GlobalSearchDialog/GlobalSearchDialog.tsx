import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { FolderOpen, User, ListChecks, Bot, Sparkles } from 'lucide-react';
import { Search } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { useOverlayManager } from '@/app/hooks/useOverlayManager';
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
import { liveAppAPI, type LiveAppMeta } from '@/infrastructure/api/service-api/LiveAppAPI';
import { APP_REGISTRY } from '@/app/scenes/apps/appRegistry';
import {
  NewSessionDialog,
  launchSessionForChoice,
  type NewSessionAgentChoice,
} from '@/app/components/SessionCapsule/NewSessionDialog';
import './GlobalSearchDialog.scss';

interface GlobalSearchDialogProps {
  open: boolean;
  onClose: () => void;
}

type SearchResultKind = 'workspace' | 'assistant' | 'session' | 'agent-app' | 'live-app';

interface SearchResultItem {
  kind: SearchResultKind;
  id: string;
  label: string;
  sublabel?: string;
  workspaceId?: string;
  agentChoice?: NewSessionAgentChoice;
}

const MAX_PER_GROUP = 20;

const getSessionTitle = (session: Session): string =>
  session.title?.trim() || `Task ${session.sessionId.slice(0, 6)}`;

const getSessionRecencyTime = (session: Session): number =>
  session.updatedAt ?? session.lastActiveAt ?? session.createdAt ?? 0;

const matchesQuery = (query: string, ...fields: (string | undefined | null)[]): boolean => {
  const normalizedQuery = query.toLowerCase();
  return fields.some(field => field && field.toLowerCase().includes(normalizedQuery));
};

const APP_TO_AGENT_CHOICE: Record<string, NewSessionAgentChoice> = {
  'coding-app': 'agentic',
  'cowork-app': 'Cowork',
  'design-app': 'Design',
  'claw-app': 'Claw',
};

const GlobalSearchDialog: React.FC<GlobalSearchDialogProps> = ({ open, onClose }) => {
  const { t } = useI18n('common');
  const { t: tApps } = useI18n('scenes/apps');
  const { activeWorkspace, openedWorkspacesList, assistantWorkspacesList, setActiveWorkspace } = useWorkspaceContext();
  const { openOverlay } = useOverlayManager();
  const { switchLeftPanelTab } = useApp();
  const setSelectedAssistantWorkspaceId = useMyAgentStore(s => s.setSelectedAssistantWorkspaceId);
  const openNurseryAssistant = useNurseryStore(s => s.openAssistant);
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const [liveApps, setLiveApps] = useState<LiveAppMeta[]>([]);
  const [newSessionDialogOpen, setNewSessionDialogOpen] = useState(false);
  const [pendingAgentChoice, setPendingAgentChoice] = useState<NewSessionAgentChoice>('agentic');
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const [persistedOpenWorkspaceSessions, setPersistedOpenWorkspaceSessions] = useState<
    Array<{ meta: SessionMetadata; workspace: WorkspaceInfo }>
  >([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    setFlowChatState(flowChatStore.getState());
    const unsubscribe = flowChatStore.subscribe(state => setFlowChatState(state));
    return () => unsubscribe();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    setQuery('');
    setActiveIndex(0);
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
        for (const workspace of openedWorkspacesList) {
          const sessionList = await sessionAPI.listSessions(
            workspace.rootPath,
            workspace.connectionId ?? undefined,
            workspace.sshHost ?? undefined
          );
          for (const meta of sessionList) {
            rows.push({ meta, workspace });
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

  useEffect(() => {
    if (!open) {
      setLiveApps([]);
      return;
    }

    let cancelled = false;
    void liveAppAPI.listLiveApps()
      .then(items => {
        if (!cancelled) {
          setLiveApps(items);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLiveApps([]);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [open]);

  const projectWorkspaces = useMemo(
    () => openedWorkspacesList.filter(workspace => workspace.workspaceKind !== WorkspaceKind.Assistant),
    [openedWorkspacesList]
  );

  const openedWorkspaceIdSet = useMemo(
    () => new Set(openedWorkspacesList.map(workspace => workspace.id)),
    [openedWorkspacesList]
  );

  const sessionsInOpenedWorkspaces = useMemo((): Array<{ session: Session; workspace: WorkspaceInfo }> => {
    const result: Array<{ session: Session; workspace: WorkspaceInfo }> = [];
    for (const session of flowChatState.sessions.values()) {
      const workspace = findWorkspaceForSession(session, openedWorkspacesList);
      if (workspace && openedWorkspaceIdSet.has(workspace.id)) {
        result.push({ session, workspace });
      }
    }
    result.sort((left, right) => getSessionRecencyTime(right.session) - getSessionRecencyTime(left.session));
    return result;
  }, [flowChatState.sessions, openedWorkspacesList, openedWorkspaceIdSet]);

  const topLevelSessions = useMemo(
    () => sessionsInOpenedWorkspaces.filter(({ session }) => !session.parentSessionId),
    [sessionsInOpenedWorkspaces]
  );

  const quickProjectWorkspace = useMemo(() => {
    if (activeWorkspace && activeWorkspace.workspaceKind !== WorkspaceKind.Assistant) {
      return activeWorkspace;
    }
    return projectWorkspaces[0] ?? null;
  }, [activeWorkspace, projectWorkspaces]);

  const quickClawWorkspace = useMemo(() => activeWorkspace ?? openedWorkspacesList[0] ?? null, [
    activeWorkspace,
    openedWorkspacesList,
  ]);

  const results = useMemo((): SearchResultItem[] => {
    const items: SearchResultItem[] = [];
    const trimmedQuery = query.trim();

    if (!trimmedQuery) {
      for (const workspace of projectWorkspaces.slice(0, MAX_PER_GROUP)) {
        items.push({
          kind: 'workspace',
          id: workspace.id,
          label: workspace.name,
          sublabel: workspace.rootPath,
        });
      }
      for (const workspace of assistantWorkspacesList.slice(0, MAX_PER_GROUP)) {
        const displayName = workspace.identity?.name?.trim() || workspace.name;
        items.push({
          kind: 'assistant',
          id: workspace.id,
          label: displayName,
          sublabel: workspace.description,
        });
      }
      for (const app of APP_REGISTRY.slice(0, MAX_PER_GROUP)) {
        items.push({
          kind: 'agent-app',
          id: app.id,
          label: tApps(app.nameKey),
          sublabel: tApps(app.descriptionKey),
          agentChoice: APP_TO_AGENT_CHOICE[app.id],
        });
      }
      for (const app of liveApps.slice(0, MAX_PER_GROUP)) {
        items.push({
          kind: 'live-app',
          id: app.id,
          label: app.name,
          sublabel: app.description || app.tags.join(' · '),
        });
      }
      return items;
    }

    const filteredWorkspaces = projectWorkspaces
      .filter(workspace => matchesQuery(trimmedQuery, workspace.name, workspace.rootPath))
      .slice(0, MAX_PER_GROUP);
    for (const workspace of filteredWorkspaces) {
      items.push({
        kind: 'workspace',
        id: workspace.id,
        label: workspace.name,
        sublabel: workspace.rootPath,
      });
    }

    const filteredAssistants = assistantWorkspacesList
      .filter(workspace => matchesQuery(trimmedQuery, workspace.name, workspace.identity?.name, workspace.description))
      .slice(0, MAX_PER_GROUP);
    for (const workspace of filteredAssistants) {
      const displayName = workspace.identity?.name?.trim() || workspace.name;
      items.push({
        kind: 'assistant',
        id: workspace.id,
        label: displayName,
        sublabel: workspace.description,
      });
    }

    const filteredAgentApps = APP_REGISTRY
      .filter(app =>
        matchesQuery(trimmedQuery, app.id, tApps(app.nameKey), tApps(app.descriptionKey))
      )
      .slice(0, MAX_PER_GROUP);
    for (const app of filteredAgentApps) {
      items.push({
        kind: 'agent-app',
        id: app.id,
        label: tApps(app.nameKey),
        sublabel: tApps(app.descriptionKey),
        agentChoice: APP_TO_AGENT_CHOICE[app.id],
      });
    }

    const filteredLiveApps = liveApps
      .filter(app =>
        matchesQuery(trimmedQuery, app.id, app.name, app.description, app.category, ...app.tags)
      )
      .slice(0, MAX_PER_GROUP);
    for (const app of filteredLiveApps) {
      items.push({
        kind: 'live-app',
        id: app.id,
        label: app.name,
        sublabel: app.description || app.tags.join(' · '),
      });
    }

    const storeMatches = topLevelSessions.filter(({ session }) =>
      matchesQuery(trimmedQuery, getSessionTitle(session), session.sessionId)
    );
    const loadedSessionIds = new Set(storeMatches.map(({ session }) => session.sessionId));

    const diskMatches = persistedOpenWorkspaceSessions.filter(({ meta, workspace }) => {
      if (!openedWorkspaceIdSet.has(workspace.id)) return false;
      if (meta.customMetadata?.parentSessionId) return false;
      const label = meta.sessionName?.trim() || `Task ${meta.sessionId.slice(0, 6)}`;
      if (!matchesQuery(trimmedQuery, label, meta.sessionId)) return false;
      return !loadedSessionIds.has(meta.sessionId);
    });

    const mergedEntries: Array<
      { session: Session; workspace: WorkspaceInfo } |
      { disk: SessionMetadata; workspace: WorkspaceInfo }
    > = [
      ...storeMatches.map(({ session, workspace }) => ({ session, workspace })),
      ...diskMatches.map(({ meta, workspace }) => ({ disk: meta, workspace })),
    ];
    mergedEntries.sort((left, right) => {
      const leftTime =
        'session' in left
          ? getSessionRecencyTime(left.session)
          : left.disk.lastActiveAt ?? left.disk.createdAt ?? 0;
      const rightTime =
        'session' in right
          ? getSessionRecencyTime(right.session)
          : right.disk.lastActiveAt ?? right.disk.createdAt ?? 0;
      return rightTime - leftTime;
    });

    for (const entry of mergedEntries.slice(0, MAX_PER_GROUP)) {
      if ('session' in entry) {
        const { session, workspace } = entry;
        items.push({
          kind: 'session',
          id: session.sessionId,
          label: getSessionTitle(session),
          sublabel: t('nav.search.sessionWorkspaceHint', { workspace: workspace.name }),
          workspaceId: workspace.id,
        });
      } else {
        const { disk, workspace } = entry;
        items.push({
          kind: 'session',
          id: disk.sessionId,
          label: disk.sessionName?.trim() || `Task ${disk.sessionId.slice(0, 6)}`,
          sublabel: t('nav.search.sessionWorkspaceHint', { workspace: workspace.name }),
          workspaceId: workspace.id,
        });
      }
    }

    return items;
  }, [
    assistantWorkspacesList,
    liveApps,
    openedWorkspaceIdSet,
    persistedOpenWorkspaceSessions,
    projectWorkspaces,
    query,
    t,
    tApps,
    topLevelSessions,
  ]);

  useEffect(() => {
    setActiveIndex(0);
  }, [results.length]);

  const handleSelect = useCallback(async (item: SearchResultItem) => {
    onClose();
    if (item.kind === 'workspace') {
      await setActiveWorkspace(item.id);
      return;
    }

    if (item.kind === 'assistant') {
      setSelectedAssistantWorkspaceId(item.id);
      openNurseryAssistant(item.id);
      await setActiveWorkspace(item.id).catch(() => {});
      switchLeftPanelTab('profile');
      openOverlay('assistant');
      return;
    }

    if (item.kind === 'agent-app') {
      const choice = item.agentChoice ?? 'agentic';
      const workspace = choice === 'Claw' ? quickClawWorkspace : quickProjectWorkspace;
      if (workspace) {
        await launchSessionForChoice({
          agentChoice: choice,
          workspace,
          setActiveWorkspace,
        });
        return;
      }

      setPendingAgentChoice(choice);
      setNewSessionDialogOpen(true);
      return;
    }

    if (item.kind === 'live-app') {
      openOverlay(`live-app:${item.id}`);
      return;
    }

    await openMainSession(item.id, {
      workspaceId: item.workspaceId,
      activateWorkspace: item.workspaceId ? setActiveWorkspace : undefined,
    });
  }, [
    onClose,
    openNurseryAssistant,
    openOverlay,
    quickClawWorkspace,
    quickProjectWorkspace,
    setActiveWorkspace,
    setSelectedAssistantWorkspaceId,
    switchLeftPanelTab,
  ]);

  const handleInputKeyDown = useCallback((event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActiveIndex(index => Math.min(index + 1, Math.max(0, results.length - 1)));
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActiveIndex(index => Math.max(index - 1, 0));
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      const item = results[activeIndex];
      if (item) {
        void handleSelect(item);
      }
    }
  }, [activeIndex, handleSelect, onClose, results]);

  useEffect(() => {
    const listElement = listRef.current;
    if (!listElement) return;
    const activeElement = listElement.querySelector<HTMLButtonElement>('.bitfun-nav-search-dialog__item--active');
    activeElement?.scrollIntoView({ block: 'nearest' });
  }, [activeIndex]);

  if (!open) return null;

  const workspaceItems = results.filter(result => result.kind === 'workspace');
  const assistantItems = results.filter(result => result.kind === 'assistant');
  const agentAppItems = results.filter(result => result.kind === 'agent-app');
  const liveAppItems = results.filter(result => result.kind === 'live-app');
  const sessionItems = results.filter(result => result.kind === 'session');
  const queryTrimmed = query.trim();
  const showDefaultSessionColumn = !queryTrimmed;

  let globalIndex = 0;
  const renderGroup = (
    groupLabel: string,
    items: SearchResultItem[],
    renderIcon: (item: SearchResultItem) => React.ReactNode
  ) => {
    if (items.length === 0) return null;
    const startIndex = globalIndex;
    globalIndex += items.length;
    return (
      <div className="bitfun-nav-search-dialog__group" key={groupLabel}>
        <div className="bitfun-nav-search-dialog__group-label">{groupLabel}</div>
        {items.map((item, itemIndex) => {
          const itemGlobalIndex = startIndex + itemIndex;
          return (
            <button
              key={item.id}
              type="button"
              className={`bitfun-nav-search-dialog__item${itemGlobalIndex === activeIndex ? ' bitfun-nav-search-dialog__item--active' : ''}`}
              onMouseEnter={() => setActiveIndex(itemGlobalIndex)}
              onClick={() => void handleSelect(item)}
            >
              <span className="bitfun-nav-search-dialog__item-icon">{renderIcon(item)}</span>
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
    <div
      className="bitfun-nav-search-dialog__overlay"
      onMouseDown={event => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
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
              {renderGroup(t('nav.search.groupAgentApps'), agentAppItems, () => <Bot size={14} />)}
              {renderGroup(t('nav.search.groupLiveApps'), liveAppItems, () => <Sparkles size={14} />)}
              {showDefaultSessionColumn ? (
                <div className="bitfun-nav-search-dialog__group" key="nav-search-sessions-default">
                  <div className="bitfun-nav-search-dialog__group-label">{t('nav.search.groupSessions')}</div>
                  <div className="bitfun-nav-search-dialog__session-hint" role="status">
                    {t('nav.search.sessionSearchHintDefault')}
                  </div>
                </div>
              ) : (
                renderGroup(t('nav.search.groupSessions'), sessionItems, () => <ListChecks size={14} />)
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );

  return createPortal(
    <>
      {dialog}
      <NewSessionDialog
        open={newSessionDialogOpen}
        onClose={() => setNewSessionDialogOpen(false)}
        initialAgentChoice={pendingAgentChoice}
      />
    </>,
    document.body
  );
};

export default GlobalSearchDialog;
