 

 
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

export function normalizePath(path: string): string {
  if (typeof path !== 'string') return path;
  
  
  
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
  } catch (e) {
    
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

