import { i18nService } from '@/infrastructure/i18n/core/I18nService';
import type {
  SessionCustomMetadata,
  SessionKind,
  SessionMetadata,
} from '@/shared/types/session-history';
import type { Session } from '../types/flow-chat';
import { resolveSessionTitle } from './sessionTitle';

const CHILD_SESSION_KIND_TAGS = new Set<SessionKind>(['btw', 'review', 'deep_review']);
const RELATIONSHIP_METADATA_KEYS = new Set([
  'kind',
  'parentSessionId',
  'parentRequestId',
  'parentDialogTurnId',
  'parentTurnIndex',
]);
const TITLE_METADATA_KEYS = new Set([
  'titleSource',
  'titleKey',
  'titleParams',
]);

type SessionRelationshipInput = Pick<Session, 'sessionKind' | 'parentSessionId' | 'btwOrigin'>;

export interface ResolvedSessionRelationship {
  kind: SessionKind;
  isBtw: boolean;
  isReview: boolean;
  isDeepReview: boolean;
  parentSessionId?: string;
  displayAsChild: boolean;
  canOpenInAuxPane: boolean;
  origin?: Session['btwOrigin'];
}

function normalizeString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

function normalizeTurnIndex(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value;
  }

  if (typeof value === 'string' && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }

  return undefined;
}

export function normalizeSessionKind(value: unknown): SessionKind {
  if (value === 'btw' || value === 'review' || value === 'deep_review') {
    return value;
  }

  return 'normal';
}

export function normalizeSessionRelationship(
  input?: Partial<SessionRelationshipInput> | null
): Pick<Session, 'sessionKind' | 'parentSessionId' | 'btwOrigin'> {
  const sessionKind = normalizeSessionKind(input?.sessionKind);
  const parentSessionId = normalizeString(
    input?.btwOrigin?.parentSessionId ?? input?.parentSessionId
  );

  if (sessionKind === 'normal') {
    return {
      sessionKind,
      parentSessionId: undefined,
      btwOrigin: undefined,
    };
  }

  const origin: Session['btwOrigin'] = {
    requestId: normalizeString(input?.btwOrigin?.requestId),
    parentSessionId,
    parentDialogTurnId: normalizeString(input?.btwOrigin?.parentDialogTurnId),
    parentTurnIndex: normalizeTurnIndex(input?.btwOrigin?.parentTurnIndex),
  };

  return {
    sessionKind,
    parentSessionId,
    btwOrigin: origin,
  };
}

export function resolveSessionRelationship(
  input?: Partial<SessionRelationshipInput> | null
): ResolvedSessionRelationship {
  const normalized = normalizeSessionRelationship(input);
  const isBtw = normalized.sessionKind === 'btw';
  const isReview =
    normalized.sessionKind === 'review' ||
    normalized.sessionKind === 'deep_review';

  return {
    kind: normalized.sessionKind,
    isBtw,
    isReview,
    isDeepReview: normalized.sessionKind === 'deep_review',
    parentSessionId: normalized.parentSessionId,
    displayAsChild: Boolean(normalized.parentSessionId),
    canOpenInAuxPane: Boolean(
      normalized.sessionKind !== 'normal' && normalized.parentSessionId
    ),
    origin: normalized.btwOrigin,
  };
}

export function deriveSessionRelationshipFromMetadata(
  metadata?: Pick<SessionMetadata, 'customMetadata'> | null
): Pick<Session, 'sessionKind' | 'parentSessionId' | 'btwOrigin'> {
  const customMetadata = metadata?.customMetadata;
  const rawSessionKind = normalizeSessionKind(customMetadata?.kind);
  const sessionKind = rawSessionKind === 'btw' ? 'normal' : rawSessionKind;

  return normalizeSessionRelationship({
    sessionKind,
    parentSessionId: customMetadata?.parentSessionId ?? undefined,
    btwOrigin:
      sessionKind !== 'normal'
        ? {
            requestId: normalizeString(customMetadata?.parentRequestId),
            parentSessionId: normalizeString(customMetadata?.parentSessionId),
            parentDialogTurnId: normalizeString(customMetadata?.parentDialogTurnId),
            parentTurnIndex: normalizeTurnIndex(customMetadata?.parentTurnIndex),
          }
        : undefined,
  });
}

export function isLegacyPersistedBtwSession(
  metadata?: Pick<SessionMetadata, 'customMetadata' | 'tags'> | null
): boolean {
  const kind = normalizeSessionKind(metadata?.customMetadata?.kind);
  if (kind === 'btw') {
    return true;
  }

  const tags = metadata?.tags;
  return Array.isArray(tags) && tags.includes('btw');
}

export function deriveLastFinishedAtFromMetadata(
  metadata?: Pick<SessionMetadata, 'customMetadata'> | null
): number | undefined {
  const value = metadata?.customMetadata?.lastFinishedAt;
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

export function calculateSessionStats(
  session: Pick<Session, 'dialogTurns'>
): Pick<SessionMetadata, 'turnCount' | 'messageCount' | 'toolCallCount'> {
  const turnCount = session.dialogTurns.length;
  const messageCount = session.dialogTurns.reduce((sum, turn) => {
    return (
      sum +
      1 +
      turn.modelRounds.reduce((roundSum, round) => {
        return roundSum + round.items.filter(item => item.type === 'text').length;
      }, 0)
    );
  }, 0);
  const toolCallCount = session.dialogTurns.reduce((sum, turn) => {
    return sum + turn.modelRounds.reduce((roundSum, round) => {
      return roundSum + round.items.filter(item => item.type === 'tool').length;
    }, 0);
  }, 0);

  return { turnCount, messageCount, toolCallCount };
}

function buildSessionCustomMetadata(
  session: Pick<
    Session,
    | 'sessionKind'
    | 'parentSessionId'
    | 'btwOrigin'
    | 'lastFinishedAt'
    | 'titleSource'
    | 'titleI18nKey'
    | 'titleI18nParams'
  >,
  existingCustomMetadata?: SessionCustomMetadata
): SessionCustomMetadata {
  const normalized = normalizeSessionRelationship(session);
  const nextCustomMetadata: SessionCustomMetadata = {};

  for (const [key, value] of Object.entries(existingCustomMetadata || {})) {
    if (!RELATIONSHIP_METADATA_KEYS.has(key) && !TITLE_METADATA_KEYS.has(key)) {
      nextCustomMetadata[key] = value;
    }
  }

  nextCustomMetadata.kind = normalized.sessionKind;

  if (normalized.sessionKind !== 'normal') {
    nextCustomMetadata.parentSessionId = normalized.parentSessionId ?? null;
    nextCustomMetadata.parentRequestId = normalized.btwOrigin?.requestId ?? null;
    nextCustomMetadata.parentDialogTurnId =
      normalized.btwOrigin?.parentDialogTurnId ?? null;
    nextCustomMetadata.parentTurnIndex =
      normalized.btwOrigin?.parentTurnIndex ?? null;
  }

  nextCustomMetadata.lastFinishedAt = session.lastFinishedAt ?? null;

  // Default untitled sessions persist their title template so locale changes can
  // re-render them until the first real title is generated or the user renames it.
  if (session.titleSource === 'i18n' && normalizeString(session.titleI18nKey)) {
    nextCustomMetadata.titleSource = 'i18n';
    nextCustomMetadata.titleKey = session.titleI18nKey;
    nextCustomMetadata.titleParams = session.titleI18nParams ?? null;
  }

  return nextCustomMetadata;
}

function buildSessionTags(
  sessionKind: SessionKind,
  existingTags?: string[]
): string[] {
  const baseTags = Array.isArray(existingTags)
    ? existingTags.filter(
        (tag) =>
          !CHILD_SESSION_KIND_TAGS.has(tag as SessionKind) || tag === sessionKind
      )
    : [];

  if (sessionKind !== 'normal' && !baseTags.includes(sessionKind)) {
    baseTags.push(sessionKind);
  }

  return baseTags;
}

export function buildSessionMetadata(
  session: Pick<
    Session,
    | 'sessionId'
    | 'title'
    | 'mode'
    | 'config'
    | 'createdAt'
    | 'workspacePath'
    | 'remoteConnectionId'
    | 'remoteSshHost'
    | 'todos'
    | 'dialogTurns'
    | 'sessionKind'
    | 'parentSessionId'
    | 'btwOrigin'
    | 'lastFinishedAt'
    | 'titleSource'
    | 'titleI18nKey'
    | 'titleI18nParams'
    | 'hasUnreadCompletion'
    | 'needsUserAttention'
    | 'deepReviewRunManifest'
  >,
  existingMetadata?: SessionMetadata | null
): SessionMetadata {
  const stats = calculateSessionStats(session);
  const sessionKind = normalizeSessionKind(session.sessionKind);

  return {
    ...existingMetadata,
    sessionId: session.sessionId,
    sessionName: resolveSessionTitle(session, (key, options) =>
      i18nService.t(key, options)
    ),
    agentType:
      session.mode ||
      session.config.agentType ||
      existingMetadata?.agentType ||
      'agentic',
    modelName:
      session.config.modelName || existingMetadata?.modelName || 'auto',
    createdAt: existingMetadata?.createdAt ?? session.createdAt,
    lastActiveAt: Date.now(),
    turnCount: Math.max(stats.turnCount, existingMetadata?.turnCount ?? 0),
    messageCount: Math.max(
      stats.messageCount,
      existingMetadata?.messageCount ?? 0
    ),
    toolCallCount: Math.max(
      stats.toolCallCount,
      existingMetadata?.toolCallCount ?? 0
    ),
    status: 'active',
    snapshotSessionId: existingMetadata?.snapshotSessionId,
    tags: buildSessionTags(sessionKind, existingMetadata?.tags),
    customMetadata: buildSessionCustomMetadata(
      {
        sessionKind,
        parentSessionId: session.parentSessionId,
        btwOrigin: session.btwOrigin,
        lastFinishedAt: session.lastFinishedAt,
        titleSource: session.titleSource,
        titleI18nKey: session.titleI18nKey,
        titleI18nParams: session.titleI18nParams,
      },
      existingMetadata?.customMetadata
    ),
    todos: session.todos || existingMetadata?.todos || [],
    workspacePath: session.workspacePath || existingMetadata?.workspacePath,
    remoteConnectionId:
      session.remoteConnectionId ?? existingMetadata?.remoteConnectionId,
    remoteSshHost: session.remoteSshHost ?? existingMetadata?.remoteSshHost,
    // Always use the in-memory session value as the source of truth.
    // Previously this used `??` to fall back to existingMetadata, which prevented
    // clears from reaching disk: when the store sets `hasUnreadCompletion: undefined`,
    // `undefined ?? existingMetadata.unreadCompletion` would restore the old value.
    unreadCompletion: session.hasUnreadCompletion,
    needsUserAttention: session.needsUserAttention,
    deepReviewRunManifest:
      session.deepReviewRunManifest ?? existingMetadata?.deepReviewRunManifest,
  };
}
