import { RecentWorkspaceEntry, RemoteSession } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { TimeFormat } from '../../services/TimeFormat';
import { ConversationSessionFilterPolicy } from './ConversationSessionFilterPolicy';

/** Everything the sidebar's grouping depends on. */
export interface SessionListInputs {
  sessions: RemoteSession[];
  query: string;
  sortMode: string;
  workspaceName: string;
  workspacePath: string;
  workspaceKind: string;
  recentWorkspaces: RecentWorkspaceEntry[];
  workspaceFilter: string;
  agentFilter: string;
  statusFilter: string;
  showWorkspaceMetadata: boolean;
  showUpdatedMetadata: boolean;
  showStatusMetadata: boolean;
}

/**
 * The session list, already filtered, grouped and labelled.
 *
 * Every array here is shared, not copied, and must be treated as read-only:
 * a cache hands the same instance to every caller until the inputs change.
 */
export interface SessionListProjection {
  filtered: RemoteSession[];
  chats: RemoteSession[];
  byTime: RemoteSession[];
  today: RemoteSession[];
  yesterday: RemoteSession[];
  earlier: RemoteSession[];
  projects: RecentWorkspaceEntry[];
  projectSessions: Map<string, RemoteSession[]>;
  metadata: Map<string, string>;
  hasActiveFilter: boolean;
}

const EMPTY_SESSIONS: RemoteSession[] = [];

/**
 * Turns the raw session list into the shape the sidebar renders, in one pass.
 *
 * The sidebar used to derive this inline, and its builders asked for the
 * derived values rather than holding them: `filteredSessions()` alone was
 * reached around twenty times per build — once per section header, once per
 * project group, three times per row — and re-scanned the whole array every
 * time, each scan re-deciding whether a session was an assistant session by
 * searching the recent-workspace list again. Traced on device, one selection
 * change spent 690ms inside the list's rerender with no child layout beneath
 * it: all of it was this arithmetic, run again for inputs that had not moved.
 */
export class SessionListProjector {
  static project(inputs: SessionListInputs): SessionListProjection {
    const assistantPaths: Set<string> = SessionListProjector.assistantWorkspacePaths(inputs.recentWorkspaces);
    const hasActiveFilter = inputs.query.trim().length > 0 || inputs.workspaceFilter.length > 0 ||
      inputs.agentFilter.length > 0 || inputs.statusFilter.length > 0;
    const wantsMetadata = inputs.showWorkspaceMetadata || inputs.showUpdatedMetadata || inputs.showStatusMetadata;

    const filtered: RemoteSession[] = [];
    const chats: RemoteSession[] = [];
    const projectSessions: Map<string, RemoteSession[]> = new Map();
    const metadata: Map<string, string> = new Map();
    const nowMs = Date.now();

    inputs.sessions.forEach((item: RemoteSession) => {
      // Decided once and carried through the rest of the pass. It is the
      // expensive predicate — it searches the recent-workspace list — and it
      // feeds the filter, the chat/project split and the agent grouping alike.
      const assistant = SessionListProjector.isAssistantSession(item, inputs, assistantPaths);
      const matches = ConversationSessionFilterPolicy.matches(
        item,
        inputs.query,
        inputs.workspacePath,
        inputs.workspaceFilter,
        inputs.agentFilter,
        inputs.statusFilter,
        assistant
      );
      if (!matches) {
        return;
      }
      filtered.push(item);
      if (assistant) {
        chats.push(item);
      } else {
        const key = ConversationSessionFilterPolicy.normalizeWorkspacePath(
          item.workspacePath ? item.workspacePath : inputs.workspacePath);
        const bucket = projectSessions.get(key);
        if (bucket === undefined) {
          projectSessions.set(key, [item]);
        } else {
          bucket.push(item);
        }
      }
      if (wantsMetadata) {
        metadata.set(item.id, SessionListProjector.metadataText(item, inputs, nowMs));
      }
    });

    const projects = SessionListProjector.projectEntries(inputs, assistantPaths, hasActiveFilter, projectSessions);
    // Sorting by time parses two timestamps per session per comparison, so it
    // stays behind the mode that asks for it.
    const byTime = inputs.sortMode === 'time' ? SessionListProjector.sortByTime(filtered) : EMPTY_SESSIONS;

    return {
      filtered,
      chats,
      byTime,
      today: SessionListProjector.bucket(byTime, 'today', nowMs),
      yesterday: SessionListProjector.bucket(byTime, 'yesterday', nowMs),
      earlier: SessionListProjector.bucket(byTime, 'earlier', nowMs),
      projects,
      projectSessions,
      metadata,
      hasActiveFilter
    };
  }

  /** The sessions filed under a workspace, or an empty list when there are none. */
  static sessionsForProject(projection: SessionListProjection, path: string): RemoteSession[] {
    const bucket = projection.projectSessions.get(ConversationSessionFilterPolicy.normalizeWorkspacePath(path));
    return bucket === undefined ? EMPTY_SESSIONS : bucket;
  }

  static metadataFor(projection: SessionListProjection, sessionId: string): string {
    const value = projection.metadata.get(sessionId);
    return value === undefined ? '' : value;
  }

  static empty(): SessionListProjection {
    return {
      filtered: EMPTY_SESSIONS,
      chats: EMPTY_SESSIONS,
      byTime: EMPTY_SESSIONS,
      today: EMPTY_SESSIONS,
      yesterday: EMPTY_SESSIONS,
      earlier: EMPTY_SESSIONS,
      projects: [],
      projectSessions: new Map(),
      metadata: new Map(),
      hasActiveFilter: false
    };
  }

  private static assistantWorkspacePaths(entries: RecentWorkspaceEntry[]): Set<string> {
    const paths: Set<string> = new Set();
    entries.forEach((item: RecentWorkspaceEntry) => {
      if (item.workspaceKind.toLowerCase() === 'assistant') {
        paths.add(item.path);
      }
    });
    return paths;
  }

  private static isAssistantWorkspace(
    path: string,
    inputs: SessionListInputs,
    assistantPaths: Set<string>
  ): boolean {
    if (path.length === 0) {
      return inputs.workspaceKind.toLowerCase() === 'assistant';
    }
    if (path === inputs.workspacePath && inputs.workspaceKind.toLowerCase() === 'assistant') {
      return true;
    }
    return assistantPaths.has(path);
  }

  private static isAssistantSession(
    item: RemoteSession,
    inputs: SessionListInputs,
    assistantPaths: Set<string>
  ): boolean {
    const agentType = (item.agentType ? item.agentType : '').toLowerCase();
    if (agentType === 'claw' || agentType === 'assistant' || agentType === 'chat') {
      return true;
    }
    return SessionListProjector.isAssistantWorkspace(
      item.workspacePath ? item.workspacePath : '', inputs, assistantPaths);
  }

  private static projectEntries(
    inputs: SessionListInputs,
    assistantPaths: Set<string>,
    hasActiveFilter: boolean,
    projectSessions: Map<string, RemoteSession[]>
  ): RecentWorkspaceEntry[] {
    const entries: RecentWorkspaceEntry[] = [];
    const currentIsAssistant = SessionListProjector.isAssistantWorkspace(inputs.workspacePath, inputs, assistantPaths);
    if ((inputs.workspacePath.length > 0 || inputs.workspaceName.length > 0) && !currentIsAssistant) {
      entries.push({
        path: inputs.workspacePath,
        name: inputs.workspaceName,
        lastOpened: '',
        workspaceKind: 'normal'
      });
    }
    inputs.recentWorkspaces.forEach((item: RecentWorkspaceEntry) => {
      if (item.path.length === 0 ||
        SessionListProjector.isAssistantWorkspace(item.path, inputs, assistantPaths)) {
        return;
      }
      if (entries.some((entry: RecentWorkspaceEntry) => entry.path === item.path)) {
        return;
      }
      entries.push(item);
    });
    if (!hasActiveFilter) {
      return entries;
    }
    // A filter narrow enough to empty a project takes that project's header off
    // the list too, so the sidebar does not read as a wall of empty groups.
    return entries.filter((item: RecentWorkspaceEntry) => {
      const bucket = projectSessions.get(ConversationSessionFilterPolicy.normalizeWorkspacePath(item.path));
      return bucket !== undefined && bucket.length > 0;
    });
  }

  private static sortByTime(sessions: RemoteSession[]): RemoteSession[] {
    // Timestamps are parsed once each and sorted alongside their session, rather
    // than re-parsed inside the comparator for every comparison it makes.
    const stamped: TimestampedSession[] = sessions.map((item: RemoteSession): TimestampedSession => {
      return { session: item, timestamp: SessionListProjector.sessionTimestamp(item) };
    });
    stamped.sort((left: TimestampedSession, right: TimestampedSession) => right.timestamp - left.timestamp);
    return stamped.map((item: TimestampedSession) => item.session);
  }

  private static bucket(sessions: RemoteSession[], bucket: string, nowMs: number): RemoteSession[] {
    if (sessions.length === 0) {
      return EMPTY_SESSIONS;
    }
    return sessions.filter((item: RemoteSession) => SessionListProjector.timeBucket(item, nowMs) === bucket);
  }

  private static timeBucket(item: RemoteSession, nowMs: number): string {
    const timestamp = SessionListProjector.sessionTimestamp(item);
    if (timestamp <= 0) {
      return 'earlier';
    }
    const sessionDate = new Date(timestamp);
    const now = new Date(nowMs);
    if (SessionListProjector.sameCalendarDay(sessionDate, now)) {
      return 'today';
    }
    const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
    return SessionListProjector.sameCalendarDay(sessionDate, yesterday) ? 'yesterday' : 'earlier';
  }

  private static sessionTimestamp(item: RemoteSession): number {
    const updated = TimeFormat.timestampMs(item.updatedAt ? item.updatedAt : '');
    if (!Number.isNaN(updated) && updated > 0) {
      return updated;
    }
    const created = TimeFormat.timestampMs(item.createdAt ? item.createdAt : '');
    return !Number.isNaN(created) && created > 0 ? created : 0;
  }

  private static sameCalendarDay(left: Date, right: Date): boolean {
    return left.getFullYear() === right.getFullYear() &&
      left.getMonth() === right.getMonth() &&
      left.getDate() === right.getDate();
  }

  private static metadataText(item: RemoteSession, inputs: SessionListInputs, nowMs: number): string {
    const values: string[] = [];
    if (inputs.showWorkspaceMetadata) {
      const workspace = item.workspaceName ? item.workspaceName : (item.workspacePath ? item.workspacePath : '');
      if (workspace.length > 0) {
        values.push(workspace);
      }
    }
    if (inputs.showUpdatedMetadata && !Number.isNaN(TimeFormat.timestampMs(item.updatedAt))) {
      values.push(TimeFormat.relative(item.updatedAt, nowMs));
    }
    if (inputs.showStatusMetadata && item.status.length > 0) {
      values.push(item.status === 'archived' ? RemoteI18n.t('sidebar.archived') : item.status);
    }
    return values.join(' · ');
  }
}

interface TimestampedSession {
  session: RemoteSession;
  timestamp: number;
}

/**
 * Holds the last projection and rebuilds it only when its inputs move.
 *
 * The point of the cache is the selection change: picking a session rebuilds
 * the sidebar's whole builder tree, but touches none of the inputs here, so the
 * rerender becomes a property diff over lists that were already computed.
 *
 * The fast path is reference identity on the two arrays. When that misses —
 * a caller handing out a freshly filtered array each build would defeat it —
 * the contents are compared field by field, which is still one cheap pass
 * instead of twenty expensive ones.
 */
export class SessionListProjectionCache {
  private key: string = ' ';
  private sessionsRef: RemoteSession[] = EMPTY_SESSIONS;
  private workspacesRef: RecentWorkspaceEntry[] = [];
  private sessionFingerprint: string[] = [];
  private workspaceFingerprint: string[] = [];
  private projection: SessionListProjection = SessionListProjector.empty();

  get(inputs: SessionListInputs): SessionListProjection {
    const key = SessionListProjectionCache.scalarKey(inputs);
    if (key === this.key && inputs.sessions === this.sessionsRef &&
      inputs.recentWorkspaces === this.workspacesRef) {
      return this.projection;
    }
    const sessionFingerprint = SessionListProjectionCache.sessionFingerprint(inputs.sessions);
    const workspaceFingerprint = SessionListProjectionCache.workspaceFingerprint(inputs.recentWorkspaces);
    if (key === this.key &&
      SessionListProjectionCache.sameFingerprint(sessionFingerprint, this.sessionFingerprint) &&
      SessionListProjectionCache.sameFingerprint(workspaceFingerprint, this.workspaceFingerprint)) {
      // Same contents behind a new array. Adopt the new references so the next
      // call takes the identity fast path.
      this.sessionsRef = inputs.sessions;
      this.workspacesRef = inputs.recentWorkspaces;
      return this.projection;
    }
    this.key = key;
    this.sessionsRef = inputs.sessions;
    this.workspacesRef = inputs.recentWorkspaces;
    this.sessionFingerprint = sessionFingerprint;
    this.workspaceFingerprint = workspaceFingerprint;
    this.projection = SessionListProjector.project(inputs);
    return this.projection;
  }

  private static scalarKey(inputs: SessionListInputs): string {
    // Relative timestamps are rendered from the projection, so when they are on
    // screen the minute they were rendered in is part of what the projection
    // depends on. Off — which is the default — nothing here moves on its own.
    const minute = inputs.showUpdatedMetadata ? Math.floor(Date.now() / 60000) : 0;
    return `${inputs.query} ${inputs.sortMode} ${inputs.workspaceName} ${inputs.workspacePath}` +
      ` ${inputs.workspaceKind} ${inputs.workspaceFilter} ${inputs.agentFilter}` +
      ` ${inputs.statusFilter} ${inputs.showWorkspaceMetadata ? 1 : 0}` +
      `${inputs.showUpdatedMetadata ? 1 : 0}${inputs.showStatusMetadata ? 1 : 0} ${minute}`;
  }

  private static sessionFingerprint(sessions: RemoteSession[]): string[] {
    return sessions.map((item: RemoteSession): string => {
      return `${item.id}${item.title}${item.status}${item.agentType}` +
        `${item.workspacePath}${item.workspaceName}${item.updatedAt}${item.createdAt}`;
    });
  }

  private static workspaceFingerprint(entries: RecentWorkspaceEntry[]): string[] {
    return entries.map((item: RecentWorkspaceEntry): string => {
      return `${item.path}${item.name}${item.workspaceKind}`;
    });
  }

  private static sameFingerprint(left: string[], right: string[]): boolean {
    if (left.length !== right.length) {
      return false;
    }
    for (let index = 0; index < left.length; index++) {
      if (left[index] !== right[index]) {
        return false;
      }
    }
    return true;
  }
}
