/**
 * Disk file identity for external-change detection (local + remote via get_file_metadata).
 */

export type DiskFileVersion = { modified: number; size: number };

export function diskVersionFromMetadata(fileInfo: unknown): DiskFileVersion | null {
  if (!fileInfo || typeof fileInfo !== 'object') {
    return null;
  }
  const o = fileInfo as Record<string, unknown>;
  if (typeof o.modified !== 'number') {
    return null;
  }
  return {
    modified: o.modified,
    size: typeof o.size === 'number' ? o.size : 0,
  };
}

export function diskVersionsDiffer(a: DiskFileVersion, b: DiskFileVersion): boolean {
  return a.modified !== b.modified || a.size !== b.size;
}
