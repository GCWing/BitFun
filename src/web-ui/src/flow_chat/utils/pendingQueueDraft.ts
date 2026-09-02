import type { ContextItem } from '@/shared/types/context';
import type { QueuedComposerDraft, QueuedMessage } from '../types/flow-chat';
import { restoreImageContextsFromPayload } from './imageContextRestoration';

type QueuedDraftSource = Pick<
  QueuedMessage,
  | 'id'
  | 'content'
  | 'displayMessage'
  | 'timestamp'
  | 'imageContexts'
  | 'imageDisplayData'
  | 'composerDraft'
>;

type UnknownRecord = Record<string, unknown>;

interface ComposerDraftOccupancy {
  value: string;
  contexts: ContextItem[];
  pendingLargePastes: Record<string, string>;
  queuedInput?: string | null;
}

function asRecord(value: unknown): UnknownRecord | undefined {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? value as UnknownRecord
    : undefined;
}

function readString(record: UnknownRecord | undefined, key: string): string | undefined {
  const value = record?.[key];
  return typeof value === 'string' && value.length > 0 ? value : undefined;
}

function sanitizePendingLargePastes(value: unknown): Record<string, string> {
  const record = asRecord(value);
  if (!record) return {};
  return Object.fromEntries(
    Object.entries(record).filter((entry): entry is [string, string] => typeof entry[1] === 'string'),
  );
}

function sanitizeStoredContexts(value: unknown): ContextItem[] | undefined {
  if (!Array.isArray(value)) return undefined;
  return value.filter((context): context is ContextItem => {
    const record = asRecord(context);
    return Boolean(readString(record, 'id') && readString(record, 'type'));
  });
}

/** Protects an existing composer draft from being replaced by a queued item. */
export function canRestoreQueuedMessageToComposer(
  composer: ComposerDraftOccupancy,
): boolean {
  return composer.value.length === 0
    && composer.contexts.length === 0
    && Object.keys(composer.pendingLargePastes).length === 0
    && !composer.queuedInput;
}

/** Restores the original composer state, with a legacy image-payload fallback. */
export function getQueuedMessageComposerDraft(message: QueuedDraftSource): QueuedComposerDraft {
  const storedDraft = asRecord(message.composerDraft);
  const storedContexts = sanitizeStoredContexts(storedDraft?.contexts);
  if (storedDraft && typeof storedDraft.value === 'string' && storedContexts) {
    return {
      value: storedDraft.value,
      contexts: [...storedContexts],
      pendingLargePastes: sanitizePendingLargePastes(storedDraft.pendingLargePastes),
    };
  }

  return {
    value: message.displayMessage ?? message.content,
    contexts: restoreImageContextsFromPayload(message),
    pendingLargePastes: {},
  };
}
