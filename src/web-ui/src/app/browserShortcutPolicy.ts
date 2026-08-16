export function shouldBlockBrowserShortcut(key: string, allowPageReload: boolean): boolean {
  const normalizedKey = key.toLowerCase();
  return normalizedKey === 'f' || (normalizedKey === 'r' && !allowPageReload);
}
