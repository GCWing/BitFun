import type { FileMetadata } from '@/infrastructure/api/service-api/WorkspaceAPI';
import type { ContextItem, DirectoryContext, FileContext, ImageContext } from '@/shared/types/context';
import { getMimeTypeFromFilename, isImageFile } from './imageUtils';

export type ExternalFileSource = 'drop' | 'clipboard';

export type ExternalFileIntakeUnavailableReason =
  | 'web'
  | 'remote-workspace'
  | 'peer-device'
  | 'detached-dispatch';

export interface ExternalFileIntakeEnvironment {
  desktopRuntime: boolean;
  remoteWorkspace: boolean;
  peerDevice: boolean;
  detachedDispatch: boolean;
}

export type ExternalFileIntakeAvailability =
  | { supported: true }
  | { supported: false; reason: ExternalFileIntakeUnavailableReason };

export interface ExternalFileIntakeFailure {
  path: string;
  reason: 'metadata' | 'invalid-path' | 'image-limit';
}

export interface BuildExternalFileContextsOptions {
  source: ExternalFileSource;
  paths: string[];
  existingContexts: ContextItem[];
  workspacePath?: string;
  maxImageCount: number;
  loadMetadata: (path: string) => Promise<FileMetadata>;
}

export interface ExternalFileIntakeResult {
  contexts: Array<FileContext | DirectoryContext | ImageContext>;
  failures: ExternalFileIntakeFailure[];
  duplicateCount: number;
}

export function resolveExternalFileIntakeAvailability(
  environment: ExternalFileIntakeEnvironment,
): ExternalFileIntakeAvailability {
  if (!environment.desktopRuntime) return { supported: false, reason: 'web' };
  if (environment.remoteWorkspace) return { supported: false, reason: 'remote-workspace' };
  if (environment.peerDevice) return { supported: false, reason: 'peer-device' };
  if (environment.detachedDispatch) return { supported: false, reason: 'detached-dispatch' };
  return { supported: true };
}

export function normalizeExternalFilePath(path: string): string {
  const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '');
  return /^[a-z]:\//i.test(normalized) ? normalized.toLowerCase() : normalized;
}

export function getContextLocalPath(context: ContextItem): string | undefined {
  if (context.type === 'file') return context.filePath;
  if (context.type === 'directory') return context.directoryPath;
  if (context.type === 'image' && context.isLocal && context.imagePath) return context.imagePath;
  return undefined;
}

function getPathName(path: string): string {
  const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '');
  return normalized.split('/').pop() || normalized;
}

function getFileMimeType(filename: string): string | undefined {
  const extension = filename.split('.').pop()?.toLowerCase();
  if (!extension || extension === filename.toLowerCase()) return undefined;
  const mimeTypes: Record<string, string> = {
    txt: 'text/plain',
    md: 'text/markdown',
    csv: 'text/csv',
    json: 'application/json',
    pdf: 'application/pdf',
    doc: 'application/msword',
    docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    xls: 'application/vnd.ms-excel',
    xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    ppt: 'application/vnd.ms-powerpoint',
    pptx: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
  };
  return mimeTypes[extension];
}

function createPathContext(
  path: string,
  name: string,
  isDirectory: boolean,
  workspacePath?: string,
): FileContext | DirectoryContext {
  const id = typeof globalThis.crypto?.randomUUID === 'function'
    ? globalThis.crypto.randomUUID()
    : `external-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
  const timestamp = Date.now();

  if (isDirectory) {
    return {
      id,
      type: 'directory',
      directoryPath: path,
      directoryName: name,
      recursive: true,
      timestamp,
    };
  }

  const normalizedWorkspace = workspacePath?.replace(/\\/g, '/').replace(/\/+$/, '');
  const normalizedPath = path.replace(/\\/g, '/');
  const relativePath = normalizedWorkspace
    && normalizedPath.toLowerCase().startsWith(`${normalizedWorkspace.toLowerCase()}/`)
    ? normalizedPath.slice(normalizedWorkspace.length + 1)
    : undefined;
  return {
    id,
    type: 'file',
    filePath: path,
    fileName: name,
    relativePath,
    timestamp,
  };
}

function withExternalMetadata<T extends FileContext | DirectoryContext>(
  context: T,
  source: ExternalFileSource,
  metadata: FileMetadata,
): T {
  return {
    ...context,
    ...(context.type === 'file'
      ? { fileSize: metadata.size, mimeType: getFileMimeType(context.fileName) }
      : {}),
    metadata: {
      ...context.metadata,
      externalLocal: true,
      externalFileSource: source,
      isSymlink: Boolean(metadata.isSymlink),
    },
  };
}

export async function buildExternalFileContexts(
  options: BuildExternalFileContextsOptions,
): Promise<ExternalFileIntakeResult> {
  const seen = new Set(
    options.existingContexts
      .map(getContextLocalPath)
      .filter((path): path is string => Boolean(path))
      .map(normalizeExternalFilePath),
  );
  let imageCount = options.existingContexts.filter((context) => context.type === 'image').length;
  const contexts: ExternalFileIntakeResult['contexts'] = [];
  const failures: ExternalFileIntakeFailure[] = [];
  let duplicateCount = 0;

  for (const path of options.paths) {
    const normalizedPath = normalizeExternalFilePath(path);
    if (!normalizedPath || seen.has(normalizedPath)) {
      duplicateCount += 1;
      continue;
    }
    seen.add(normalizedPath);

    let metadata: FileMetadata;
    try {
      metadata = await options.loadMetadata(path);
    } catch {
      failures.push({ path, reason: 'metadata' });
      continue;
    }

    if (!metadata.isFile && !metadata.isDir) {
      failures.push({ path, reason: 'invalid-path' });
      continue;
    }

    const name = getPathName(path);
    if (metadata.isFile && isImageFile(name)) {
      if (imageCount >= options.maxImageCount) {
        failures.push({ path, reason: 'image-limit' });
        continue;
      }
      imageCount += 1;
      contexts.push({
        id: `img-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`,
        type: 'image',
        imagePath: path,
        imageName: name,
        fileSize: metadata.size,
        mimeType: getMimeTypeFromFilename(name),
        source: 'file',
        isLocal: true,
        timestamp: Date.now(),
        metadata: {
          externalLocal: true,
          externalFileSource: options.source,
          isSymlink: Boolean(metadata.isSymlink),
          isImage: true,
        },
      });
      continue;
    }

    contexts.push(
      withExternalMetadata(
        createPathContext(path, name, metadata.isDir, options.workspacePath),
        options.source,
        metadata,
      ),
    );
  }

  return { contexts, failures, duplicateCount };
}
