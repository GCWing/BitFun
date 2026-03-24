/**
 * Heuristic detection of "file not found" from API/FS errors (local + remote).
 */

export function isLikelyFileNotFoundError(err: unknown): boolean {
  const s = String(err).toLowerCase();
  return (
    s.includes('no such file') ||
    s.includes('does not exist') ||
    s.includes('not found') ||
    s.includes('os error 2') ||
    s.includes('enoent') ||
    s.includes('path not found')
  );
}

/** Metadata from get_file_metadata: missing remote file uses is_file false and is_dir false. */
export function isFileMissingFromMetadata(fileInfo: Record<string, unknown> | null | undefined): boolean {
  if (!fileInfo || typeof fileInfo !== 'object') {
    return true;
  }
  return fileInfo.is_file !== true;
}
