/**
 * SessionListDialog — centered modal showing the full session list with search.
 *
 * Opened by the task list icon in UnifiedTopBar when settings / shell overlay
 * is active. Mirrors the visual structure of NavSearchDialog.
 */

import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { LayoutDashboard, ListChecks } from 'lucide-react';
import { Search, Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { findOpenedWorkspaceForSession } from '@/flow_chat/utils/sessionOrdering';
import { openMainSession } from '@/flow_chat/services/openBtwSession';
import { compareSessionsForDisplay } from '@/flow_chat/utils/sessionOrdering';
import type { FlowChatState, Session } from '@/flow_chat/types/flow-chat';
import { useSessionCapsuleStore } from '../../stores/sessionCapsuleStore';
import { useOverlayStore } from '../../stores/overlayStore';
import '../NavPanel/NavSearchDialog.scss';
import './SessionListDialog.scss';

const getTitle = (s: Session): string =>
  s.title?.trim() || `Task ${s.sessionId.slice(0, 6)}`;

const matchesQuery = (q: string, ...fields: (string | undefined | null)[]): boolean => {
  const lower = q.toLowerCase();
  return fields.some(f => f?.toLowerCase().includes(lower));
};

const SessionListDialog: React.FC = () => {
  const { t } = useI18n('common');
  const open = useSessionCapsuleStore((s) => s.sessionListDialogOpen);
  const close = useSessionCapsuleStore((s) => s.closeSessionListDialog);
  const openTaskDetail = useSessionCapsuleStore((s) => s.openTaskDetail);
  const openOverlay = useOverlayStore((s) => s.openOverlay);

  const { openedWorkspacesList, setActiveWorkspace, currentWorkspace } = useWorkspaceContext();
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const [flowChatState, setFlowChatState] = useState<FlowChatState>(() => flowChatStore.getState());
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    // Subscribe only while open; sync immediately so the first paint is not stale empty state.
    setFlowChatState(flowChatStore.getState());
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
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        close();
      }
    };
    document.addEventListener('keydown', handler, true);
    return () => document.removeEventListener('keydown', handler, true);
  }, [open, close]);

  const allSessions = useMemo((): Array<{ session: Session; workspaceName?: string }> => {
    const list = Array.from(flowChatState.sessions.values())
      .filter(s => s.mode !== 'Dispatcher')
      .sort(compareSessionsForDisplay);
    return list.map(session => {
      const ws = findOpenedWorkspaceForSession(session, openedWorkspacesList);
      return { session, workspaceName: ws?.name };
    });
  }, [flowChatState.sessions, openedWorkspacesList]);

  const filtered = useMemo(() => {
    const q = query.trim();
    if (!q) return allSessions;
    return allSessions.filter(({ session, workspaceName }) =>
      matchesQuery(q, getTitle(session), session.sessionId, workspaceName)
    );
  }, [allSessions, query]);

  useEffect(() => {
    setActiveIndex(0);
  }, [filtered.length]);

  const handleSelect = useCallback(async (session: Session) => {
    close();
    const ws = findOpenedWorkspaceForSession(session, openedWorkspacesList);
    const mustActivate = ws && ws.id !== currentWorkspace?.id;
    await openMainSession(session.sessionId, {
      workspaceId: ws?.id,
      activateWorkspace: mustActivate ? setActiveWorkspace : undefined,
    });
    window.dispatchEvent(
      new CustomEvent('flowchat:switch-session', { detail: { sessionId: session.sessionId } })
    );
  }, [close, openedWorkspacesList, currentWorkspace?.id, setActiveWorkspace]);

  const handleOpenDetail = useCallback((e: React.MouseEvent, session: Session) => {
    e.stopPropagation();
    openTaskDetail(session.sessionId);
    openOverlay('task-detail');
    close();
  }, [openTaskDetail, openOverlay, close]);

  const handleInputKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setActiveIndex(i => Math.min(i + 1, Math.max(0, filtered.length - 1)));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex(i => Math.max(i - 1, 0));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const item = filtered[activeIndex];
      if (item) void handleSelect(item.session);
    }
  }, [activeIndex, close, filtered, handleSelect]);

  // Scroll active item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const active = list.querySelector<HTMLElement>('.bitfun-nav-search-dialog__item--active');
    active?.scrollIntoView({ block: 'nearest' });
  }, [activeIndex]);

  if (!open) return null;

  const dialog = (
    <div
      className="bitfun-nav-search-dialog__overlay"
      onMouseDown={(e) => { if (e.target === e.currentTarget) close(); }}
    >
      <div className="bitfun-nav-search-dialog__card">
        {/* Search input row */}
        <div className="bitfun-nav-search-dialog__input-row">
          <Search
            ref={inputRef}
            className="bitfun-nav-search-dialog__search"
            placeholder={t('nav.sections.sessions')}
            value={query}
            onChange={setQuery}
            onClear={() => setQuery('')}
            onKeyDown={handleInputKeyDown}
            clearable
            size="medium"
            autoFocus
          />
        </div>

        {/* Results */}
        <div className="bitfun-nav-search-dialog__results" ref={listRef}>
          {filtered.length === 0 ? (
            <div className="bitfun-nav-search-dialog__empty">{t('nav.search.empty')}</div>
          ) : (
            <div className="bitfun-nav-search-dialog__group">
              <div className="bitfun-nav-search-dialog__group-label">
                {t('nav.search.groupSessions')}
              </div>
              {filtered.map(({ session, workspaceName }, i) => (
                <div
                  key={session.sessionId}
                  className={`bitfun-nav-search-dialog__item bitfun-session-list-item${i === activeIndex ? ' bitfun-nav-search-dialog__item--active' : ''}`}
                  onMouseEnter={() => setActiveIndex(i)}
                  onClick={() => void handleSelect(session)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={e => e.key === 'Enter' && void handleSelect(session)}
                >
                  <span className="bitfun-nav-search-dialog__item-icon">
                    <ListChecks size={14} />
                  </span>
                  <span className="bitfun-nav-search-dialog__item-content">
                    <span className="bitfun-nav-search-dialog__item-label">
                      {getTitle(session)}
                    </span>
                    {workspaceName && (
                      <span className="bitfun-nav-search-dialog__item-sublabel">
                        {t('nav.search.sessionWorkspaceHint', { workspace: workspaceName })}
                      </span>
                    )}
                  </span>
                  <Tooltip content="查看详情" placement="top">
                    <button
                      type="button"
                      className="bitfun-session-list-item__detail-btn"
                      onClick={(e) => handleOpenDetail(e, session)}
                      aria-label="查看任务详情"
                    >
                      <LayoutDashboard size={12} />
                    </button>
                  </Tooltip>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );

  return createPortal(dialog, document.body);
};

export default SessionListDialog;
