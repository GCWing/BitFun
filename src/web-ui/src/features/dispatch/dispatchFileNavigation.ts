import { flowChatStore } from '@/flow_chat/store/FlowChatStore';
import { resolveDispatchJobId, resolveSessionDriverId } from '@/flow_chat/session-drivers/resolve';
import { getActiveSurfaceScope } from '@/infrastructure/peer-device/deviceSurface';
import type { LineRange } from '@/shared/editor/LineRange';
import { notificationService } from '@/shared/notification-system';
import { createTab } from '@/shared/utils/tabUtils';
import { dispatchApi } from './dispatchApi';

export function isDispatchFileSession(sessionId: string | undefined): boolean {
  return Boolean(sessionId && resolveSessionDriverId(
    sessionId, flowChatStore.getState().sessions.get(sessionId),
  ) === 'dispatch');
}

const IMAGE_MIME = /^image\/(?:png|jpeg|gif|webp|bmp|svg\+xml|avif|x-icon)$/i;
const IMAGE_PATH = /\.(?:png|jpe?g|gif|webp|bmp|svg|avif|ico)$/i;
const TEXT_PATH = /\.(?:txt|md|mdx|log|json|jsonl|xml|ya?ml|toml|ini|css|s[ac]ss|[cm]?js|tsx?|jsx|rs|py|go|java|kt|swift|c|cpp|h|sh|sql)$/i;
const imageRequests = new Map<string, Promise<string>>();

function captureFileOrigin(sessionId: string) {
  const scope = getActiveSurfaceScope();
  const sessions = flowChatStore.getState().sessions;
  const jobId = resolveDispatchJobId(sessionId, sessions.get(sessionId), id => sessions.get(id));
  if (!jobId) throw new Error('This remote session is still connecting to its job. Try opening the file again.');
  return {
    scope, jobId,
    assertCurrent() {
      scope.assertCurrent('read dispatch output');
      const current = flowChatStore.getState().sessions;
      if (resolveDispatchJobId(sessionId, current.get(sessionId), id => current.get(id)) !== jobId) {
        throw new Error('The output session changed during transfer');
      }
    },
  };
}

type FileOrigin = ReturnType<typeof captureFileOrigin>;

async function readOutputBytes(origin: FileOrigin, filePath: string, maxBytes: number) {
  let first: Awaited<ReturnType<typeof dispatchApi.readFileChunk>> | undefined;
  const chunks: Uint8Array[] = [];
  let offset = 0;
  do {
    origin.assertCurrent();
    // Retry only this read at the acknowledged offset. A revision change still
    // fails validation; reconnecting never combines bytes from two revisions.
    const readChunk = () => dispatchApi.readFileChunk(origin.jobId, filePath, {
      offset, limit: 256 * 1024, expectedRevision: first?.revision,
    });
    let chunk: Awaited<ReturnType<typeof readChunk>> | undefined;
    for (let attempt = 0; attempt < 3; attempt += 1) {
      origin.assertCurrent();
      try { chunk = await readChunk(); break; }
      catch (error) {
        if (attempt === 2) throw error;
        await new Promise(resolve => setTimeout(resolve, 500 * (attempt + 1)));
      }
    }
    if (!chunk) throw new Error('The output file transfer was interrupted');
    origin.assertCurrent();
    if (chunk.kind !== 'readFileChunk' || chunk.jobId !== origin.jobId
      || !Number.isSafeInteger(chunk.totalSize) || chunk.totalSize < 0 || chunk.totalSize > maxBytes
      || chunk.offset !== offset || !chunk.revision || typeof chunk.name !== 'string'
      || typeof chunk.mimeType !== 'string' || typeof chunk.contentBase64 !== 'string'
      || (first && (chunk.revision !== first.revision || chunk.totalSize !== first.totalSize
        || chunk.filePath !== first.filePath || chunk.sessionId !== first.sessionId
        || chunk.name !== first.name || chunk.mimeType !== first.mimeType))) {
      throw new Error('The target returned an invalid, changed, or oversized output file');
    }
    const raw = atob(chunk.contentBase64);
    if (raw.length !== chunk.chunkSize || raw.length > 256 * 1024
      || offset + raw.length > chunk.totalSize || (!raw.length && offset < chunk.totalSize)) {
      throw new Error('The output file transfer is incomplete');
    }
    first ??= chunk;
    chunks.push(Uint8Array.from(raw, character => character.charCodeAt(0)));
    offset += raw.length;
  } while (offset < first.totalSize);
  origin.assertCurrent();
  const bytes = new Uint8Array(offset);
  let position = 0;
  for (const chunk of chunks) { bytes.set(chunk, position); position += chunk.length; }
  return { ...first, bytes };
}

function imageDataUrl(file: { bytes: Uint8Array; mimeType: string }): string {
  if (!IMAGE_MIME.test(file.mimeType)) throw new Error('This output cannot be displayed as an image');
  let binary = '';
  for (let start = 0; start < file.bytes.length; start += 8192) {
    binary += String.fromCharCode(...file.bytes.subarray(start, start + 8192));
  }
  return `data:${file.mimeType};base64,${btoa(binary)}`;
}

export async function readDispatchSessionImage(sessionId: string, filePath: string, refresh = false): Promise<string> {
  const origin = captureFileOrigin(sessionId);
  const key = origin.scope.key('dispatch-image', origin.scope.epoch, origin.jobId, filePath);
  if (refresh) imageRequests.delete(key);
  const cached = imageRequests.get(key);
  if (cached) return cached;
  const request = readOutputBytes(origin, filePath, 12 * 1024 * 1024).then(imageDataUrl).finally(() => {
    if (imageRequests.get(key) === request) imageRequests.delete(key);
  });
  if (imageRequests.size >= 16) imageRequests.delete(imageRequests.keys().next().value!);
  imageRequests.set(key, request);
  return request;
}

export async function downloadDispatchSessionFile(sessionId: string, filePath: string): Promise<void> {
  const origin = captureFileOrigin(sessionId);
  try {
    const file = await readOutputBytes(origin, filePath, 128 * 1024 * 1024);
    origin.assertCurrent();
    const url = URL.createObjectURL(new Blob([file.bytes], { type: file.mimeType }));
    const link = document.createElement('a');
    link.href = url;
    link.download = file.name;
    document.body.appendChild(link);
    link.click();
    link.remove();
    // Leave time for the browser's download handoff to consume the Blob.
    setTimeout(() => URL.revokeObjectURL(url), 30_000);
  } catch (error) {
    if (origin.scope.isCurrent()) notificationService.error(error instanceof Error ? error.message : String(error));
    throw error;
  }
}

/** An immutable target preview: the controller must never probe or watch this path. */
export async function openDispatchSessionFile(
  sessionId: string,
  filePath: string,
  fileName: string,
  lineRange?: LineRange,
): Promise<void> {
  const scope = getActiveSurfaceScope();
  const sessions = flowChatStore.getState().sessions;
  const session = sessions.get(sessionId);
  const jobId = resolveDispatchJobId(sessionId, session, id => sessions.get(id));
  try {
    if (!jobId) throw new Error('This remote session is still connecting to its job. Try opening the file again.');
    if (IMAGE_PATH.test(filePath)) {
      const origin = captureFileOrigin(sessionId);
      const file = await readOutputBytes(origin, filePath, 12 * 1024 * 1024);
      origin.assertCurrent();
      createTab({
        type: 'image-viewer', title: file.name,
        data: { filePath: `dispatch-file://${encodeURIComponent(jobId)}/${encodeURIComponent(file.filePath)}`,
          imageSource: { dataUrl: imageDataUrl(file), size: file.totalSize } },
        checkDuplicate: true, duplicateCheckKey: scope.key(jobId, file.filePath),
        replaceExisting: true, mode: 'agent', isCurrent: scope.isCurrent,
      });
      return;
    }
    if (!TEXT_PATH.test(filePath)) {
      await downloadDispatchSessionFile(sessionId, filePath).catch(() => undefined);
      return;
    }
    const response = await dispatchApi.readFile(jobId, filePath);
    if (!scope.isCurrent()) return;
    const current = flowChatStore.getState();
    if (resolveDispatchJobId(sessionId, current.sessions.get(sessionId), id => current.sessions.get(id)) !== jobId) return;
    if (response.kind !== 'readFile' || response.jobId !== jobId || typeof response.content !== 'string') {
      throw new Error('The target returned an invalid file preview.');
    }
    const previewPath = `dispatch-file://${encodeURIComponent(jobId)}/${encodeURIComponent(response.filePath)}`;
    createTab({
      type: 'code-editor',
      title: fileName,
      data: {
        filePath: previewPath,
        fileName,
        initialContent: response.content,
        readOnly: true,
        jumpToRange: lineRange,
        navigationToken: Date.now(),
      },
      checkDuplicate: true,
      duplicateCheckKey: scope.key(jobId, response.filePath),
      replaceExisting: true,
      mode: 'agent',
      isCurrent: scope.isCurrent,
    });
  } catch (error) {
    if (scope.isCurrent()) {
      notificationService.error(error instanceof Error ? error.message : String(error));
    }
  }
}
