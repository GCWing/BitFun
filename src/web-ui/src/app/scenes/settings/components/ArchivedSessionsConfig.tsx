/**
 * ArchivedSessionsConfig — settings page for managing archived sessions.
 *
 * Lists all archived sessions grouped by workspace, with per-session
 * restore / delete actions and a bulk "Delete All Archived" action.
 * Every destructive or state-changing operation is gated behind a
 * confirmation dialog.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Trash2, RotateCcw, Inbox, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  ConfigPageLayout,
  ConfigPageHeader,
  ConfigPageContent,
  ConfigPageSection,
} from '@/infrastructure/config/components/common';
import { Button } from '@/component-library';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { sessionAPI } from '@/infrastructure/api/service-api/SessionAPI';
import { confirmWarning, confirmDanger } from '@/component-library/components/ConfirmDialog/confirmService';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import type { SessionMetadata } from '@/shared/types/session-history';
import { i18nService } from '@/infrastructure/i18n';
import './ArchivedSessionsConfig.scss';

const log = createLogger('ArchivedSessionsConfig');

// ── Types ──────────────────────────────────────────────────────────────────

interface ArchivedEntry {
  session: SessionMetadata;
  workspacePath: string;
  workspaceName: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

// ── Helpers ────────────────────────────────────────────────────────────────

function formatDateTime(timestampMs: number): string {
  if (!timestampMs) return '';
  try {
    const d = new Date(timestampMs);
    return i18nService.formatDate(d, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return '';
  }
}

// ── Sub-components ─────────────────────────────────────────────────────────

interface ArchivedRowProps {
  entry: ArchivedEntry;
  onRestore: (entry: ArchivedEntry) => void;
  onDelete: (entry: ArchivedEntry) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
}

const ArchivedRow: React.FC<ArchivedRowProps> = ({ entry, onRestore, onDelete, t }) => {
  const { session } = entry;
  const displayName = session.sessionName || t('nav.sessions.untitled');
  const dateStr = formatDateTime(session.lastActiveAt);

  return (
    <div className="archived-sessions-config__row">
      <div className="archived-sessions-config__row-info">
        <span className="archived-sessions-config__row-name">{displayName}</span>
        {dateStr && (
          <span className="archived-sessions-config__row-date">{dateStr}</span>
        )}
      </div>
      <div className="archived-sessions-config__row-actions">
        <Button
          size="small"
          variant="ghost"
          onClick={() => onRestore(entry)}
          aria-label={t('nav.sessions.restore')}
        >
          <RotateCcw size={13} />
          <span>{t('nav.sessions.restore')}</span>
        </Button>
        <Button
          size="small"
          variant="ghost"
          onClick={() => onDelete(entry)}
          aria-label={t('nav.sessions.deleteArchived')}
          className="archived-sessions-config__delete-btn"
        >
          <Trash2 size={13} />
          <span>{t('nav.sessions.deleteArchived')}</span>
        </Button>
      </div>
    </div>
  );
};

// ── Main component ─────────────────────────────────────────────────────────

const ArchivedSessionsConfig: React.FC = () => {
  const { t } = useTranslation('common');
  const { openedWorkspacesList } = useWorkspaceContext();
  const activeTab = useSettingsStore(s => s.activeTab);

  const [loading, setLoading] = useState(true);
  const [entries, setEntries] = useState<ArchivedEntry[]>([]);

  // ── Load archived sessions from all open workspaces ──────────────────────

  const loadArchived = useCallback(async () => {
    setLoading(true);
    const collected: ArchivedEntry[] = [];

    for (const ws of openedWorkspacesList) {
      try {
        const archived = await sessionAPI.listArchivedSessions(
          ws.rootPath,
          ws.connectionId,
          ws.sshHost
        );
        for (const session of archived) {
          collected.push({
            session,
            workspacePath: ws.rootPath,
            workspaceName: ws.name,
            remoteConnectionId: ws.connectionId,
            remoteSshHost: ws.sshHost,
          });
        }
      } catch (err) {
        log.error('Failed to load archived sessions for workspace', { workspace: ws.rootPath, err });
      }
    }

    // Sort by last active descending
    collected.sort((a, b) => b.session.lastActiveAt - a.session.lastActiveAt);
    setEntries(collected);
    setLoading(false);
  }, [openedWorkspacesList]);

  // Re-fetch whenever this tab becomes active, or the workspace list changes
  useEffect(() => {
    if (activeTab === 'archived-sessions') {
      void loadArchived();
    }
  }, [activeTab, loadArchived]);

  // Re-fetch when a session is archived elsewhere while this page is open
  useEffect(() => {
    const handler = () => {
      void loadArchived();
    };
    window.addEventListener('bitfun:session-archived', handler);
    return () => window.removeEventListener('bitfun:session-archived', handler);
  }, [loadArchived]);

  // ── Group entries by workspace ───────────────────────────────────────────

  const grouped = useMemo(() => {
    const map = new Map<string, { name: string; entries: ArchivedEntry[] }>();
    for (const entry of entries) {
      const key = entry.workspacePath;
      let group = map.get(key);
      if (!group) {
        group = { name: entry.workspaceName, entries: [] };
        map.set(key, group);
      }
      group.entries.push(entry);
    }
    return map;
  }, [entries]);

  // ── Remove an entry from local state after mutation ──────────────────────

  const removeEntry = useCallback((sessionId: string) => {
    setEntries(prev => prev.filter(e => e.session.sessionId !== sessionId));
  }, []);

  const removeAllEntries = useCallback(() => {
    setEntries([]);
  }, []);

  // ── Restore single session ───────────────────────────────────────────────

  const handleRestore = useCallback(async (entry: ArchivedEntry) => {
    const confirmed = await confirmWarning(
      t('nav.sessions.unarchiveConfirmTitle'),
      t('nav.sessions.unarchiveConfirmMessage')
    );
    if (!confirmed) return;

    try {
      await sessionAPI.unarchiveSession(
        entry.session.sessionId,
        entry.workspacePath,
        entry.remoteConnectionId,
        entry.remoteSshHost
      );
      removeEntry(entry.session.sessionId);
      // Refresh the workspace sessions so the restored session appears in the sidebar immediately
      await flowChatManager.refreshWorkspaceSessions({
        rootPath: entry.workspacePath,
        connectionId: entry.remoteConnectionId,
        sshHost: entry.remoteSshHost,
      });
    } catch (err) {
      log.error('Failed to restore archived session', err);
      notificationService.error(
        err instanceof Error ? err.message : t('nav.sessions.restoreFailed'),
        { duration: 4000 }
      );
    }
  }, [t, removeEntry]);

  // ── Delete single archived session ───────────────────────────────────────

  const handleDelete = useCallback(async (entry: ArchivedEntry) => {
    const confirmed = await confirmDanger(
      t('nav.sessions.deleteArchivedConfirmTitle'),
      t('nav.sessions.deleteArchivedConfirmMessage')
    );
    if (!confirmed) return;

    try {
      await sessionAPI.deleteSession(
        entry.session.sessionId,
        entry.workspacePath,
        entry.remoteConnectionId,
        entry.remoteSshHost
      );
      removeEntry(entry.session.sessionId);
    } catch (err) {
      log.error('Failed to delete archived session', err);
      notificationService.error(
        err instanceof Error ? err.message : t('nav.sessions.deleteArchivedFailed'),
        { duration: 4000 }
      );
    }
  }, [t, removeEntry]);

  // ── Delete all archived sessions ─────────────────────────────────────────

  const handleDeleteAll = useCallback(async () => {
    const confirmed = await confirmDanger(
      t('nav.sessions.deleteAllArchivedConfirmTitle'),
      t('nav.sessions.deleteAllArchivedConfirmMessage')
    );
    if (!confirmed) return;

    try {
      // Delete across all workspaces that have archived sessions
      const processedPaths = new Set<string>();
      for (const entry of entries) {
        if (processedPaths.has(entry.workspacePath)) continue;
        processedPaths.add(entry.workspacePath);
        await sessionAPI.deleteAllArchivedSessions(
          entry.workspacePath,
          entry.remoteConnectionId,
          entry.remoteSshHost
        );
      }
      removeAllEntries();
    } catch (err) {
      log.error('Failed to delete all archived sessions', err);
      notificationService.error(
        err instanceof Error ? err.message : t('nav.sessions.deleteAllArchivedFailed'),
        { duration: 4000 }
      );
    }
  }, [t, entries, removeAllEntries]);

  // ── Render ───────────────────────────────────────────────────────────────

  const hasEntries = entries.length > 0;

  const headerExtra = (
    <div className="archived-sessions-config__header-actions">
      <Button
        size="small"
        variant="ghost"
        onClick={() => { void loadArchived(); }}
        aria-label="Refresh"
      >
        <RefreshCw size={13} />
      </Button>
      {hasEntries && (
        <Button
          size="small"
          variant="ghost"
          onClick={() => { void handleDeleteAll(); }}
          className="archived-sessions-config__delete-all-btn"
        >
          <Trash2 size={13} />
          <span>{t('nav.sessions.deleteAllArchived')}</span>
        </Button>
      )}
    </div>
  );

  return (
    <ConfigPageLayout className="archived-sessions-config">
      <ConfigPageHeader
        title={t('nav.sessions.archivedSessions')}
        subtitle={t('nav.sessions.archivedSessionsDescription')}
      />
      <ConfigPageContent>
        {loading ? (
          <div className="archived-sessions-config__loading">
            {t('nav.sessions.loading')}
          </div>
        ) : !hasEntries ? (
          <ConfigPageSection
            title={t('nav.sessions.archivedSessions')}
            extra={headerExtra}
          >
            <div className="archived-sessions-config__empty">
              <Inbox size={32} className="archived-sessions-config__empty-icon" />
              <span>{t('nav.sessions.noArchivedSessions')}</span>
            </div>
          </ConfigPageSection>
        ) : (
          <ConfigPageSection
            title={t('nav.sessions.archivedSessions')}
            extra={headerExtra}
          >
            {Array.from(grouped.entries()).map(([workspacePath, group]) => (
              <div key={workspacePath} className="archived-sessions-config__group">
                <div className="archived-sessions-config__group-header">
                  <span className="archived-sessions-config__group-name">{group.name}</span>
                  <span className="archived-sessions-config__group-count">
                    {group.entries.length}
                  </span>
                </div>
                <div className="archived-sessions-config__group-list">
                  {group.entries.map(entry => (
                    <ArchivedRow
                      key={entry.session.sessionId}
                      entry={entry}
                      onRestore={(e) => { void handleRestore(e); }}
                      onDelete={(e) => { void handleDelete(e); }}
                      t={t}
                    />
                  ))}
                </div>
              </div>
            ))}
          </ConfigPageSection>
        )}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default ArchivedSessionsConfig;
