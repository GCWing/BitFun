import type { ContextItem, ImageContext } from '@/shared/types/context';
import type { QueuedComposerDraft, QueuedMessage } from '../types/flow-chat';

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

function readNumber(record: UnknownRecord | undefined, key: string): number | undefined {
  const value = record?.[key];
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
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

function mimeTypeFromDataUrl(dataUrl: string | undefined): string | undefined {
  return dataUrl?.match(/^data:([^;,]+)[;,]/)?.[1];
}

function restoreLegacyImageContexts(message: QueuedDraftSource): ImageContext[] {
  const displayItems = Array.isArray(message.imageDisplayData)
    ? message.imageDisplayData.map(asRecord).filter((item): item is UnknownRecord => Boolean(item))
    : [];
  const transportItems = Array.isArray(message.imageContexts)
    ? message.imageContexts.map(asRecord).filter((item): item is UnknownRecord => Boolean(item))
    : [];
  const transportById = new Map(
    transportItems.flatMap(item => {
      const id = readString(item, 'id');
      return id ? [[id, item] as const] : [];
    }),
  );
  const restored: ImageContext[] = [];
  const seenIds = new Set<string>();
  const entryCount = Math.max(displayItems.length, transportItems.length);

  for (let index = 0; index < entryCount; index += 1) {
    const display = displayItems[index];
    const displayId = readString(display, 'id');
    const transport = (displayId ? transportById.get(displayId) : undefined) ?? transportItems[index];
    const id = displayId ?? readString(transport, 'id') ?? `${message.id}-image-${index + 1}`;
    if (seenIds.has(id)) continue;
    seenIds.add(id);

    const metadata = asRecord(transport?.metadata);
    const dataUrl = readString(display, 'dataUrl');
    const imagePath = readString(display, 'imagePath') ?? readString(transport, 'image_path') ?? '';
    const sourceValue = readString(metadata, 'source');
    const source: ImageContext['source'] =
      sourceValue === 'file' || sourceValue === 'clipboard' || sourceValue === 'url'
        ? sourceValue
        : dataUrl
          ? 'clipboard'
          : 'file';

    restored.push({
      id,
      type: 'image',
      timestamp: message.timestamp,
      imagePath,
      imageName: readString(display, 'name') ?? readString(metadata, 'name') ?? 'Image',
      width: readNumber(metadata, 'width'),
      height: readNumber(metadata, 'height'),
      fileSize: readNumber(metadata, 'file_size') ?? 0,
      mimeType:
        readString(display, 'mimeType')
        ?? readString(transport, 'mime_type')
        ?? mimeTypeFromDataUrl(dataUrl)
        ?? 'image/png',
      dataUrl,
      thumbnailUrl: dataUrl,
      source,
      // Queued payloads with a host path are already uploaded and can be
      // resubmitted without repeating the clipboard upload.
      isLocal: Boolean(imagePath),
    });
  }

  return restored;
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
    contexts: restoreLegacyImageContexts(message),
    pendingLargePastes: {},
  };
}
