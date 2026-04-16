 

 
/**
 * Normalize a remote SSH/SFTP path (always POSIX). Safe on Windows clients where
 * UI or path APIs may introduce backslashes or duplicate slashes.
 */
export function normalizeRemoteWorkspacePath(path: string): string {
  if (typeof path !== 'string') return path;
  let s = path.replace(/\\/g, '/');
  while (s.includes('//')) {
    s = s.replace('//', '/');
  }
  if (s === '/') return s;
  return s.replace(/\/+$/, '');
}

export function hasNonFileUriScheme(path: string): boolean {
  if (typeof path !== 'string') return false;
  return /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(path) && !path.startsWith('file://');
}

export function isBitFunRuntimeUri(path: string): boolean {
  return typeof path === 'string' && path.startsWith('bitfun://runtime/');
}

export function normalizePath(path: string): string {
  if (typeof path !== 'string') return path;

  if (hasNonFileUriScheme(path)) {
    return path;
  }
  
  
  
  let normalized = path.replace(/^file:\/+/, '');
  
  
  normalized = normalized.replace(/\\/g, '/');
  
  
  
  //   - /D:/code/... -> D:/code/...
  //   - /d:/code/... -> d:/code/...
  //   - //D:/code/... -> D:/code/...
  normalized = normalized.replace(/^\/+([a-zA-Z]:)/, '$1');
  
  
  normalized = normalized.replace(/^([a-z]):/, (_match, letter) => letter.toUpperCase() + ':');
  
  
  normalized = normalized.replace(/\/+/g, '/');
  
  
  try {
    const decoded = decodeURIComponent(normalized);
    
    if (decoded !== normalized) {
      normalized = decoded;
    }
  } catch (_error) {
    
  }
  
  return normalized;
}

 
export function isSamePath(path1: string, path2: string): boolean {
  return normalizePath(path1) === normalizePath(path2);
}

 
export function uriToPath(uri: string): string {
  return normalizePath(uri);
}

 
export function pathToUri(path: string): string {
  
  const normalized = normalizePath(path);
  
  return `file:///${normalized}`;
}

 
export function joinPath(basePath: string, relativePath: string): string {
  
  const normalizedBase = basePath.replace(/\\/g, '/').replace(/\/+$/, '');
  
  const normalizedRelative = relativePath.replace(/\\/g, '/').replace(/^\/+/, '');
  
  return `${normalizedBase}/${normalizedRelative}`;
}

/** Last path segment (file or folder name). Handles mixed `/` and `\\`. */
export function basenamePath(fullPath: string): string {
  if (!fullPath || typeof fullPath !== 'string') return '';
  const i = Math.max(fullPath.lastIndexOf('/'), fullPath.lastIndexOf('\\'));
  if (i < 0) return fullPath;
  return fullPath.slice(i + 1);
}

/** Parent directory; supports mixed separators and Unix root (`/` parent of `/foo`). */
export function dirnameAbsolutePath(fullPath: string): string {
  if (!fullPath || typeof fullPath !== 'string') return '';
  const i = Math.max(fullPath.lastIndexOf('/'), fullPath.lastIndexOf('\\'));
  if (i < 0) return '';
  if (i === 0) return fullPath[0] === '/' ? '/' : '';
  return fullPath.slice(0, i);
}

/** Replace the final segment; keeps the separator style before the basename. */
export function replaceBasename(fullPath: string, newName: string): string {
  if (!fullPath || typeof fullPath !== 'string') return newName;
  const i = Math.max(fullPath.lastIndexOf('/'), fullPath.lastIndexOf('\\'));
  if (i < 0) return newName;
  return `${fullPath.slice(0, i + 1)}${newName}`;
}

/**
 * Normalize for local rename IPC: `normalizePath` except skip UNC (`\\?\`, `\\server\...`)
 * so we do not turn backslashes into slashes there.
 */
export function normalizeLocalPathForRename(path: string): string {
  const t = path.trim();
  if (t.startsWith('\\\\')) return t;
  return normalizePath(t);
}

/**
 * True if two absolute filesystem paths refer to the same location.
 * Normalizes `\\` vs `/`; on Windows-style roots (`C:` or `\\`) compares case-insensitively.
 */
export function pathsEquivalentFs(a: string, b: string): boolean {
  if (a === b) return true;
  const ka = a.replace(/\\/g, '/');
  const kb = b.replace(/\\/g, '/');
  if (ka === kb) return true;
  const winLike = /^[a-zA-Z]:/.test(a.trim()) || a.startsWith('\\\\');
  if (winLike) return ka.toLowerCase() === kb.toLowerCase();
  return false;
}

/** Whether `path` is expanded when the set may mix separators or drive letter case (Windows). */
export function expandedFoldersContains(expandedFolders: Set<string>, path: string): boolean {
  if (expandedFolders.has(path)) return true;
  for (const p of expandedFolders) {
    if (pathsEquivalentFs(p, path)) return true;
  }
  return false;
}

export function expandedFoldersDeleteEquivalent(set: Set<string>, path: string): Set<string> {
  const next = new Set(set);
  const toDelete: string[] = [];
  next.forEach((p) => {
    if (pathsEquivalentFs(p, path)) toDelete.push(p);
  });
  toDelete.forEach((p) => next.delete(p));
  return next;
}

/** Add `path` after removing any equivalent entry (single canonical key). */
export function expandedFoldersAddEquivalent(set: Set<string>, path: string): Set<string> {
  const next = expandedFoldersDeleteEquivalent(set, path);
  next.add(path);
  return next;
}
