import type { SessionMetadata } from '@/shared/types/session-history';
import { isSamePath, normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';
import type { Session } from '../types/flow-chat';

export interface SessionTitleDescriptor {
  source: 'text' | 'i18n';
  text: string;
  key?: string;
  params?: Record<string, unknown>;
}

type TranslateFn = (key: string, options?: Record<string, unknown>) => string;
export type DefaultSessionTitleMode = 'code' | 'cowork' | 'claw';

export interface DefaultSessionTitleCounterScope {
  mode: DefaultSessionTitleMode;
  workspaceId?: string | null;
  workspacePath?: string | null;
  remoteConnectionId?: string | null;
  remoteSshHost?: string | null;
}

type DefaultSessionTitleCounterSession = Pick<
  Session,
  | 'titleSource'
  | 'titleI18nKey'
  | 'titleI18nParams'
  | 'mode'
  | 'sessionKind'
  | 'workspaceId'
  | 'workspacePath'
  | 'remoteConnectionId'
  | 'remoteSshHost'
>;

const DEFAULT_SESSION_TITLE_KEY = 'flow-chat:session.new';
const DEFAULT_SESSION_TITLE_KEYS_BY_MODE: Record<DefaultSessionTitleMode, string> = {
  code: 'flow-chat:session.newCodeWithIndex',
  cowork: 'flow-chat:session.newCoworkWithIndex',
  claw: 'flow-chat:session.newClawWithIndex',
};

function normalizeTitleText(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

function normalizeTitleParams(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}

function normalizeText(value: string | null | undefined): string {
  return value?.trim() ?? '';
}

function normalizeTitleCount(value: unknown): number | undefined {
  const numericValue = typeof value === 'number' ? value : typeof value === 'string' ? Number(value) : NaN;
  if (!Number.isFinite(numericValue) || numericValue < 1) {
    return undefined;
  }
  return Math.floor(numericValue);
}

function defaultSessionTitleKeyForMode(mode: DefaultSessionTitleMode): string {
  return DEFAULT_SESSION_TITLE_KEYS_BY_MODE[mode];
}

export function normalizeDefaultSessionTitleMode(mode?: string): DefaultSessionTitleMode {
  if (!mode) return 'code';
  const normalizedMode = mode.toLowerCase();
  if (normalizedMode === 'cowork') return 'cowork';
  if (normalizedMode === 'claw') return 'claw';
  return 'code';
}

function remoteHostFromConnectionId(connectionId: string): string {
  const match = connectionId.trim().match(/^ssh-[^@]+@(.+):(\d+)$/);
  return match ? match[1].trim().toLowerCase() : '';
}

function effectiveRemoteHost(remoteSshHost?: string | null, remoteConnectionId?: string | null): string {
  return normalizeText(remoteSshHost).toLowerCase() || remoteHostFromConnectionId(remoteConnectionId ?? '');
}

function sessionMatchesCounterScope(
  session: DefaultSessionTitleCounterSession,
  scope: DefaultSessionTitleCounterScope,
): boolean {
  const sessionWorkspaceId = normalizeText(session.workspaceId);
  const targetWorkspaceId = normalizeText(scope.workspaceId);
  if (sessionWorkspaceId && targetWorkspaceId && sessionWorkspaceId === targetWorkspaceId) {
    return true;
  }

  const targetWorkspacePath = normalizeText(scope.workspacePath);
  if (!targetWorkspacePath) {
    return false;
  }

  const sessionWorkspacePath = normalizeText(session.workspacePath) || targetWorkspacePath;
  const pathsMatch =
    isSamePath(sessionWorkspacePath, targetWorkspacePath) ||
    normalizeRemoteWorkspacePath(sessionWorkspacePath) ===
      normalizeRemoteWorkspacePath(targetWorkspacePath);
  if (!pathsMatch) {
    return false;
  }

  const targetRemoteConnectionId = normalizeText(scope.remoteConnectionId);
  const sessionRemoteConnectionId = normalizeText(session.remoteConnectionId);
  const targetRemoteHost = effectiveRemoteHost(scope.remoteSshHost, targetRemoteConnectionId);
  const sessionRemoteHost = effectiveRemoteHost(session.remoteSshHost, sessionRemoteConnectionId);

  if (targetRemoteHost || sessionRemoteHost) {
    return targetRemoteHost === sessionRemoteHost;
  }

  if (targetRemoteConnectionId || sessionRemoteConnectionId) {
    return targetRemoteConnectionId === sessionRemoteConnectionId;
  }

  return true;
}

function defaultTitleCountForMode(
  session: DefaultSessionTitleCounterSession,
  mode: DefaultSessionTitleMode,
): number | undefined {
  if (
    session.titleSource !== 'i18n' ||
    session.titleI18nKey !== defaultSessionTitleKeyForMode(mode)
  ) {
    return undefined;
  }
  return normalizeTitleCount(session.titleI18nParams?.count);
}

export function getNextDefaultSessionTitleCount(
  sessions: Iterable<DefaultSessionTitleCounterSession>,
  scope: DefaultSessionTitleCounterScope,
): number {
  let matchingSessionCount = 0;
  let maxExplicitTitleCount = 0;

  for (const session of sessions) {
    if (session.sessionKind && session.sessionKind !== 'normal') {
      continue;
    }
    if (normalizeDefaultSessionTitleMode(session.mode) !== scope.mode) {
      continue;
    }
    if (!sessionMatchesCounterScope(session, scope)) {
      continue;
    }

    matchingSessionCount += 1;
    maxExplicitTitleCount = Math.max(
      maxExplicitTitleCount,
      defaultTitleCountForMode(session, scope.mode) ?? 0,
    );
  }

  // Generated or renamed sessions no longer carry the i18n title count, but they
  // still represent consumed default-name slots in this workspace/mode scope.
  return Math.max(maxExplicitTitleCount, matchingSessionCount) + 1;
}

export function createTextSessionTitleDescriptor(text: string): SessionTitleDescriptor {
  return {
    source: 'text',
    text: normalizeTitleText(text) || '',
  };
}

export function createI18nSessionTitleDescriptor(
  key: string,
  translate: TranslateFn,
  params?: Record<string, unknown>,
): SessionTitleDescriptor {
  return {
    source: 'i18n',
    text: translate(key, params),
    key,
    params,
  };
}

export function createDefaultSessionTitleDescriptor(
  mode: DefaultSessionTitleMode,
  count: number,
  translate: TranslateFn,
): SessionTitleDescriptor {
  const key = defaultSessionTitleKeyForMode(mode);
  return createI18nSessionTitleDescriptor(key, translate, { count });
}

export function deriveSessionTitleState(
  descriptor?: SessionTitleDescriptor,
): Pick<Session, 'title' | 'titleSource' | 'titleI18nKey' | 'titleI18nParams'> {
  if (descriptor?.source === 'i18n' && normalizeTitleText(descriptor.key)) {
    return {
      title: normalizeTitleText(descriptor.text),
      titleSource: 'i18n',
      titleI18nKey: descriptor.key,
      titleI18nParams: descriptor.params,
    };
  }

  return {
    title: normalizeTitleText(descriptor?.text),
    titleSource: 'text',
    titleI18nKey: undefined,
    titleI18nParams: undefined,
  };
}

export function freezeSessionTitleState(
  title: string,
): Pick<Session, 'title' | 'titleSource' | 'titleI18nKey' | 'titleI18nParams'> {
  return {
    title: normalizeTitleText(title),
    titleSource: 'text',
    titleI18nKey: undefined,
    titleI18nParams: undefined,
  };
}

export function deriveSessionTitleStateFromMetadata(
  metadata?: Pick<SessionMetadata, 'sessionName' | 'customMetadata' | 'turnCount'> | null,
): Pick<Session, 'title' | 'titleSource' | 'titleI18nKey' | 'titleI18nParams'> {
  const titleKey = normalizeTitleText(metadata?.customMetadata?.titleKey);
  const useDynamicTitle =
    metadata?.customMetadata?.titleSource === 'i18n' &&
    Boolean(titleKey) &&
    (metadata?.turnCount ?? 0) === 0;

  return {
    title: normalizeTitleText(metadata?.sessionName),
    titleSource: useDynamicTitle ? 'i18n' : 'text',
    titleI18nKey: useDynamicTitle ? titleKey : undefined,
    titleI18nParams: useDynamicTitle
      ? normalizeTitleParams(metadata?.customMetadata?.titleParams)
      : undefined,
  };
}

export function resolveSessionTitle(
  session: Pick<
    Session,
    'title' | 'titleSource' | 'titleI18nKey' | 'titleI18nParams'
  > | null | undefined,
  translate: TranslateFn,
  fallbackKey: string = DEFAULT_SESSION_TITLE_KEY,
): string {
  if (session?.titleSource === 'i18n' && normalizeTitleText(session.titleI18nKey)) {
    return translate(session.titleI18nKey!, session.titleI18nParams);
  }

  return normalizeTitleText(session?.title) || translate(fallbackKey);
}

export function resolvePersistedSessionTitle(
  metadata: Pick<SessionMetadata, 'sessionName' | 'customMetadata' | 'turnCount'> | null | undefined,
  translate: TranslateFn,
  fallbackKey: string = DEFAULT_SESSION_TITLE_KEY,
): string {
  const titleKey = normalizeTitleText(metadata?.customMetadata?.titleKey);
  if (
    metadata?.customMetadata?.titleSource === 'i18n' &&
    titleKey &&
    (metadata?.turnCount ?? 0) === 0
  ) {
    return translate(titleKey, normalizeTitleParams(metadata?.customMetadata?.titleParams));
  }

  return normalizeTitleText(metadata?.sessionName) || translate(fallbackKey);
}
