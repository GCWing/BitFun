import pathUtils from 'path-browserify';
import type { ContentResourceScope } from '../types/contentResource';

/** Native resource paths are not URLs. Decode only explicitly supplied file URIs. */
export function resourceFilePath(path: string, scope: ContentResourceScope): string {
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(path) && !/^file:\/\//i.test(path)) return path;
  let value = path;
  if (/^file:\/\//i.test(value)) {
    const uri = new URL(value);
    value = (uri.hostname ? '//' + uri.hostname : '') + decodeURIComponent(uri.pathname);
  }
  value = value.replace(/\\/g, '/');
  if (!/^(?:[a-z]:|\/)/i.test(value) && scope.workspacePath) value = scope.workspacePath.replace(/\\/g, '/') + '/' + value;
  const unc = !scope.remoteConnectionId && value.startsWith('//');
  value = value.replace(/\/+/g, '/');
  if (!scope.remoteConnectionId) value = value.replace(/^\/?([a-z]):/i, (_, drive) => drive.toUpperCase() + ':');
  value = pathUtils.posix.normalize(value);
  if (unc) value = '/' + value;
  return value === '/' ? value : value.replace(/\/+$/, '');
}

export function resourcePathKey(path: string, scope: ContentResourceScope): string {
  const normalized = resourceFilePath(path, scope);
  return !scope.remoteConnectionId && /^(?:[a-z]:|\/\/)/i.test(normalized)
    ? normalized.toLowerCase() : normalized;
}
