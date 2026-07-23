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

/**
 * Normalize text so BOM / newline style differences do not false-trigger
 * "file changed on disk" flows (metadata can change from saves or tooling while
 * logical content matches the editor buffer).
 */
export function normalizeTextForDiskSyncComparison(text: string): string {
  let s = text;
  if (s.length > 0 && s.charCodeAt(0) === 0xfeff) {
    s = s.slice(1);
  }
  return s.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
}

/** True if disk-read text and editor buffer are the same for external-sync purposes. */
export function diskContentMatchesEditorForExternalSync(
  diskText: string,
  editorText: string
): boolean {
  return (
    normalizeTextForDiskSyncComparison(diskText) ===
    normalizeTextForDiskSyncComparison(editorText)
  );
}

/**
 * SHA-256 hex (lowercase) of UTF-8 bytes of normalized text.
 * Must match desktop `get_file_editor_sync_hash` for non-binary files.
 */
export async function editorSyncContentSha256Hex(text: string): Promise<string> {
  const normalized = normalizeTextForDiskSyncComparison(text);
  const data = new TextEncoder().encode(normalized);
  if (typeof crypto === 'undefined' || !crypto.subtle?.digest) {
    throw new Error('crypto.subtle.digest is not available');
  }
  const digest = await crypto.subtle.digest('SHA-256', data);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}
