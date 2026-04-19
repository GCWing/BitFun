import { i18nService } from '@/infrastructure/i18n';
import type {
  SessionCustomMetadata,
  SessionKind,
  SessionMetadata,
} from '@/shared/types/session-history';
import type { Session } from '../types/flow-chat';

const BTW_TAG = 'btw';
const RELATIONSHIP_METADATA_KEYS = new Set([
  'kind',
  'parentSessionId',
  'parentRequestId',
  'parentDialogTurnId',
  'parentTurnIndex',
]);

type SessionRelationshipInput = Pick<Session, 'sessionKind' | 'parentSessionId' | 'btwOrigin'>;

export interface ResolvedSessionRelationship {
  kind: SessionKind;
  isBtw: boolean;
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
  return value === 'btw' ? 'btw' : 'normal';
}

export function normalizeSessionRelationship(
  input?: Partial<SessionRelationshipInput> | null
): Pick<Session, 'sessionKind' | 'parentSessionId' | 'btwOrigin'> {
  const sessionKind = normalizeSessionKind(input?.sessionKind);
  const parentSessionId = normalizeString(
    input?.btwOrigin?.parentSessionId ?? input?.parentSessionId
  );

  if (sessionKind !== 'btw') {
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

  return {
    kind: normalized.sessionKind,
    isBtw,
    parentSessionId: normalized.parentSessionId,
    displayAsChild: Boolean(normalized.parentSessionId),
    canOpenInAuxPane: Boolean(isBtw && normalized.parentSessionId),
    origin: normalized.btwOrigin,
  };
}

export function deriveSessionRelationshipFromMetadata(
  metadata?: Pick<SessionMetadata, 'customMetadata'> | null
): Pick<Session, 'sessionKind' | 'parentSessionId' | 'btwOrigin'> {
  const customMetadata = metadata?.customMetadata;
  const sessionKind = normalizeSessionKind(customMetadata?.kind);

  return normalizeSessionRelationship({
    sessionKind,
    parentSessionId: customMetadata?.parentSessionId ?? undefined,
    btwOrigin:
      sessionKind === 'btw'
        ? {
            requestId: normalizeString(customMetadata?.parentRequestId),
            parentSessionId: normalizeString(customMetadata?.parentSessionId),
            parentDialogTurnId: normalizeString(customMetadata?.parentDialogTurnId),
            parentTurnIndex: normalizeTurnIndex(customMetadata?.parentTurnIndex),
          }
        : undefined,
  });
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
  session: Pick<Session, 'sessionKind' | 'parentSessionId' | 'btwOrigin' | 'lastFinishedAt'>,
  existingCustomMetadata?: SessionCustomMetadata
): SessionCustomMetadata {
  const normalized = normalizeSessionRelationship(session);
  const nextCustomMetadata: SessionCustomMetadata = {};

  for (const [key, value] of Object.entries(existingCustomMetadata || {})) {
    if (!RELATIONSHIP_METADATA_KEYS.has(key)) {
      nextCustomMetadata[key] = value;
    }
  }

  nextCustomMetadata.kind = normalized.sessionKind;

  if (normalized.sessionKind === 'btw') {
    nextCustomMetadata.parentSessionId = normalized.parentSessionId ?? null;
    nextCustomMetadata.parentRequestId = normalized.btwOrigin?.requestId ?? null;
    nextCustomMetadata.parentDialogTurnId =
      normalized.btwOrigin?.parentDialogTurnId ?? null;
    nextCustomMetadata.parentTurnIndex =
      normalized.btwOrigin?.parentTurnIndex ?? null;
  }

  nextCustomMetadata.lastFinishedAt = session.lastFinishedAt ?? null;

  return nextCustomMetadata;
}

function buildSessionTags(
  sessionKind: SessionKind,
  existingTags?: string[]
): string[] {
  const baseTags = Array.isArray(existingTags) ? [...existingTags] : [];

  if (sessionKind === 'btw' && !baseTags.includes(BTW_TAG)) {
    baseTags.push(BTW_TAG);
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
    | 'storageScope'
    | 'todos'
    | 'dialogTurns'
    | 'sessionKind'
    | 'parentSessionId'
    | 'btwOrigin'
    | 'lastFinishedAt'
  >,
  existingMetadata?: SessionMetadata | null
): SessionMetadata {
  const stats = calculateSessionStats(session);
  const sessionKind = normalizeSessionKind(session.sessionKind);

  return {
    ...existingMetadata,
    sessionId: session.sessionId,
    sessionName:
      session.title ||
      existingMetadata?.sessionName ||
      i18nService.t('flow-chat:session.new'),
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
      },
      existingMetadata?.customMetadata
    ),
    todos: session.todos || existingMetadata?.todos || [],
    workspacePath: session.workspacePath || existingMetadata?.workspacePath,
    remoteConnectionId:
      session.remoteConnectionId ?? existingMetadata?.remoteConnectionId,
    remoteSshHost: session.remoteSshHost ?? existingMetadata?.remoteSshHost,
    storageScope: session.storageScope ?? existingMetadata?.storageScope ?? 'workspace',
  };
}
