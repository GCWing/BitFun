/**
 * Path helpers for the peer directory browser (Windows + POSIX).
 */

export function looksLikeWindowsPath(path: string): boolean {
  return /^[A-Za-z]:/.test(path) || path.includes('\\');
}

export function joinDirectoryPath(dir: string, name: string): string {
  const cleanName = name.replace(/^[/\\]+/, '');
  if (!cleanName) {
    return dir;
  }
  if (looksLikeWindowsPath(dir) || /^[A-Za-z]:$/.test(dir)) {
    const base = dir.replace(/[/\\]+$/, '');
    return `${base}\\${cleanName}`;
  }
  if (!dir || dir === '/') {
    return `/${cleanName}`;
  }
  return `${dir.replace(/\/+$/, '')}/${cleanName}`;
}

export function parentDirectoryPath(path: string): string | null {
  const trimmed = path.replace(/[/\\]+$/, '');
  if (!trimmed || trimmed === '/' || /^[A-Za-z]:$/.test(trimmed)) {
    return null;
  }
  if (/^[A-Za-z]:[\\/]?$/.test(path)) {
    return null;
  }
  if (looksLikeWindowsPath(path)) {
    const normalized = trimmed.replace(/\//g, '\\');
    const idx = normalized.lastIndexOf('\\');
    if (idx <= 0) {
      return null;
    }
    if (idx === 2 && /^[A-Za-z]:\\/.test(normalized)) {
      return normalized.slice(0, 3); // C:\
    }
    return normalized.slice(0, idx);
  }
  const parts = trimmed.split('/').filter(Boolean);
  if (parts.length === 0) {
    return null;
  }
  if (parts.length === 1) {
    return '/';
  }
  parts.pop();
  return `/${parts.join('/')}`;
}
