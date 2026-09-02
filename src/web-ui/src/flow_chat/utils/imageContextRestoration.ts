import type { ImageContext } from '@/shared/types/context';

type UnknownRecord = Record<string, unknown>;

export interface RestorableImagePayload {
  id: string;
  timestamp: number;
  imageContexts?: unknown[];
  imageDisplayData?: unknown[];
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

function mimeTypeFromDataUrl(dataUrl: string | undefined): string | undefined {
  return dataUrl?.match(/^data:([^;,]+)[;,]/)?.[1];
}

/** Rebuilds composer-owned image contexts from persisted display/transport shapes. */
export function restoreImageContextsFromPayload(
  message: RestorableImagePayload,
): ImageContext[] {
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
      // A host path means the image can be resubmitted without another upload.
      isLocal: Boolean(imagePath),
    });
  }

  return restored;
}
