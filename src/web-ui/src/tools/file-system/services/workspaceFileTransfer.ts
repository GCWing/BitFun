/**
 * Upload / download between workspace (local or remote SFTP) and local disk.
 */

import { PhysicalPosition } from '@tauri-apps/api/dpi';
import { sshApi } from '@/features/ssh-remote/sshApi';
import { workspaceAPI } from '@/infrastructure/api';
import { i18nService } from '@/infrastructure/i18n';
import { isRemoteWorkspace, type WorkspaceInfo } from '@/shared/types';
import {
  dirnameAbsolutePath,
  normalizeLocalPathForRename,
  normalizePath,
  normalizeRemoteWorkspacePath,
  pathsEquivalentFs,
} from '@/shared/utils/pathUtils';

export type TransferPhase = 'download' | 'upload';

export interface TransferProgressState {
  phase: TransferPhase;
  current: number;
  total: number;
  label: string;
  /** Single-file transfer: no byte-level progress from backend — show indeterminate bar */
  indeterminate?: boolean;
}

export interface WorkspaceTransferResult {
  successCount: number;
  failedFiles: Array<{ path: string; error: string }>;
}

export interface UploadToWorkspaceOptions {
  isCut?: boolean;
}

function normalizeClipboardLocalPath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed) {
    return '';
  }

  if (trimmed.startsWith('file://')) {
    let normalized = normalizePath(trimmed);
    // file:///absolute/unix/path loses its leading slash in normalizePath.
    if (
      /^file:\/\/\/(?!\/)/.test(trimmed)
      && !/^[A-Za-z]:/.test(normalized)
      && !normalized.startsWith('/')
    ) {
      normalized = `/${normalized}`;
    }
    return normalized;
  }

  if (trimmed.startsWith('\\\\')) {
    return trimmed;
  }

  return normalizeLocalPathForRename(trimmed);
}

export function normalizeClipboardLocalPaths(paths: string[]): string[] {
  const normalized: string[] = [];

  for (const path of paths) {
    const next = normalizeClipboardLocalPath(path);
    if (!next || normalized.some((existing) => pathsEquivalentFs(existing, next))) {
      continue;
    }
    normalized.push(next);
  }

  return normalized;
}

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

export function resolvePasteTargetDirectory<T extends { path: string; isDirectory: boolean; children?: T[] }>(options: {
  workspacePath: string;
  explicitTargetDir?: string;
  selectedFile?: string;
  fileTree: T[];
  findNode: (nodes: T[], path: string) => T | null;
}): string {
  if (options.explicitTargetDir) {
    return options.explicitTargetDir;
  }

  const targetDirectory = options.workspacePath;
  if (!options.selectedFile) {
    return targetDirectory;
  }

  const selectedNode = options.findNode(options.fileTree, options.selectedFile);
  if (!selectedNode) {
    return targetDirectory;
  }

  if (selectedNode.isDirectory) {
    return selectedNode.path;
  }

  return dirnameAbsolutePath(selectedNode.path) || targetDirectory;
}

export function normalizeWorkspaceTargetDirectory(
  targetDirectory: string,
  workspace: WorkspaceInfo | null
): string {
  if (isRemoteWorkspace(workspace)) {
    return normalizeRemoteWorkspacePath(targetDirectory);
  }
  return normalizeLocalPathForRename(targetDirectory);
}

export function joinWorkspaceTargetPath(dir: string, fileName: string, remote = false): string {
  const sep = remote ? '/' : (dir.includes('\\') ? '\\' : '/');
  const base = remote
    ? normalizeRemoteWorkspacePath(dir)
    : dir.replace(/[/\\]+$/, '');
  return `${base}${sep}${fileName}`;
}

export function resolveExplorerDropTargetDirectory(
  clientX: number,
  clientY: number,
  workspacePath: string,
  boundary?: HTMLElement | null
): string {
  const el = document.elementFromPoint(clientX, clientY);
  if (!el) {
    return workspacePath;
  }

  const explorer = boundary ?? el.closest('.bitfun-file-explorer');
  if (!explorer) {
    return workspacePath;
  }

  if (!explorer.contains(el)) {
    return workspacePath;
  }

  const node = el.closest('[data-file-path]');
  if (!node || !explorer.contains(node)) {
    return workspacePath;
  }

  const path = node.getAttribute('data-file-path');
  if (!path) {
    return workspacePath;
  }
  const isDir = node.getAttribute('data-is-directory') === 'true';
  if (isDir) {
    return path;
  }
  return dirnameAbsolutePath(path) || workspacePath;
}

function dragPositionToLogicalCandidates(
  position: { x: number; y: number },
  scaleFactor: number
): { x: number; y: number }[] {
  const logical = new PhysicalPosition(position.x, position.y).toLogical(scaleFactor);
  return [
    { x: logical.x, y: logical.y },
    { x: position.x, y: position.y },
    { x: position.x / scaleFactor, y: position.y / scaleFactor },
  ];
}

/**
 * Tauri emits physical pixel positions; `elementFromPoint` / `getBoundingClientRect` use logical CSS pixels.
 * Try a few conversions because platform / overlay titlebars can differ.
 */
export function resolveDropTargetDirectoryFromDragPosition(
  position: { x: number; y: number },
  scaleFactor: number,
  workspacePath: string,
  boundary?: HTMLElement | null
): string {
  for (const { x, y } of dragPositionToLogicalCandidates(position, scaleFactor)) {
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      continue;
    }

    const hit = document.elementFromPoint(x, y);
    const explorer = boundary ?? hit?.closest('.bitfun-file-explorer');
    if (!explorer) {
      continue;
    }

    if (hit && !explorer.contains(hit)) {
      continue;
    }

    return resolveExplorerDropTargetDirectory(x, y, workspacePath, explorer as HTMLElement);
  }

  return workspacePath;
}

export function isDragPositionOverElement(
  position: { x: number; y: number },
  scaleFactor: number,
  element: HTMLElement | null
): boolean {
  if (!element) {
    return false;
  }
  const rect = element.getBoundingClientRect();
  for (const { x, y } of dragPositionToLogicalCandidates(position, scaleFactor)) {
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      continue;
    }
    if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
      return true;
    }
  }
  return false;
}

export async function downloadWorkspaceFileToDisk(
  filePath: string,
  workspace: WorkspaceInfo | null,
  onProgress: (state: TransferProgressState | null) => void
): Promise<void> {
  if (!isTauri()) {
    throw new Error(i18nService.t('common:ssh.remote.transferNeedsDesktop'));
  }
  const { save } = await import('@tauri-apps/plugin-dialog');
  const baseName = filePath.split(/[/\\]/).pop() || 'file';
  const dest = await save({
    title: i18nService.t('common:file.downloadSaveTitle'),
    defaultPath: baseName,
  });
  if (dest === null) {
    return;
  }

  onProgress({
    phase: 'download',
    current: 0,
    total: 1,
    label: baseName,
    indeterminate: true,
  });
  try {
    if (isRemoteWorkspace(workspace)) {
      const cid = workspace?.connectionId;
      if (!cid) {
        throw new Error(i18nService.t('panels/files:transfer.missingConnection'));
      }
      await sshApi.downloadToLocalPath(cid, filePath, dest);
    } else {
      await workspaceAPI.exportLocalFileToPath(filePath, dest);
    }
    onProgress({
      phase: 'download',
      current: 1,
      total: 1,
      label: baseName,
      indeterminate: false,
    });
  } finally {
    window.setTimeout(() => onProgress(null), 450);
  }
}

export async function uploadLocalPathsToWorkspaceDirectory(
  localPaths: string[],
  targetDirectory: string,
  workspace: WorkspaceInfo | null,
  onProgress: (state: TransferProgressState | null) => void,
  options: UploadToWorkspaceOptions = {}
): Promise<WorkspaceTransferResult> {
  if (!isTauri()) {
    throw new Error(i18nService.t('common:ssh.remote.transferNeedsDesktop'));
  }

  const normalizedLocalPaths = normalizeClipboardLocalPaths(localPaths);
  if (normalizedLocalPaths.length === 0) {
    return { successCount: 0, failedFiles: [] };
  }

  const remote = isRemoteWorkspace(workspace);
  const normalizedTargetDirectory = normalizeWorkspaceTargetDirectory(targetDirectory, workspace);
  const isCut = options.isCut ?? false;

  if (remote) {
    const cid = workspace?.connectionId;
    if (!cid) {
      throw new Error(i18nService.t('panels/files:transfer.missingConnection'));
    }

    const failedFiles: WorkspaceTransferResult['failedFiles'] = [];
    let successCount = 0;
    const total = normalizedLocalPaths.length;

    for (let i = 0; i < total; i++) {
      const localPath = normalizedLocalPaths[i]!;
      const name = localPath.split(/[/\\]/).pop();
      if (!name) {
        continue;
      }

      const destPath = joinWorkspaceTargetPath(normalizedTargetDirectory, name, true);
      onProgress({
        phase: 'upload',
        current: i,
        total,
        label: name,
        indeterminate: total === 1,
      });

      try {
        await sshApi.uploadFromLocalPath(cid, localPath, destPath);
        successCount += 1;
      } catch (error) {
        failedFiles.push({
          path: localPath,
          error: error instanceof Error ? error.message : String(error),
        });
      }
    }

    onProgress({
      phase: 'upload',
      current: total,
      total,
      label: '',
      indeterminate: false,
    });
    window.setTimeout(() => onProgress(null), 450);

    if (successCount === 0 && failedFiles.length > 0) {
      const details = failedFiles.map((entry) => `${entry.path}: ${entry.error}`).join('; ');
      throw new Error(details);
    }

    return { successCount, failedFiles };
  }

  onProgress({
    phase: 'upload',
    current: 0,
    total: normalizedLocalPaths.length,
    label: normalizedLocalPaths.length === 1
      ? (normalizedLocalPaths[0]?.split(/[/\\]/).pop() ?? '')
      : '',
    indeterminate: normalizedLocalPaths.length === 1,
  });

  const result = await workspaceAPI.pasteFiles(
    normalizedLocalPaths,
    normalizedTargetDirectory,
    isCut
  );

  onProgress({
    phase: 'upload',
    current: normalizedLocalPaths.length,
    total: normalizedLocalPaths.length,
    label: '',
    indeterminate: false,
  });
  window.setTimeout(() => onProgress(null), 450);

  if (result.successCount === 0 && result.failedFiles.length > 0) {
    const details = result.failedFiles
      .map((entry) => `${entry.path}: ${entry.error}`)
      .join('; ');
    throw new Error(details);
  }

  return {
    successCount: result.successCount,
    failedFiles: result.failedFiles,
  };
}

export async function pasteClipboardFilesToWorkspaceDirectory(
  targetDirectory: string,
  workspace: WorkspaceInfo | null,
  onProgress: (state: TransferProgressState | null) => void
): Promise<WorkspaceTransferResult> {
  const { files, isCut } = await workspaceAPI.getClipboardFiles();
  return uploadLocalPathsToWorkspaceDirectory(
    files,
    targetDirectory,
    workspace,
    onProgress,
    { isCut }
  );
}
