const HTML_FILE_RE = /\.(?:html|htm)$/i;

export function isHtmlFilePath(pathOrName: string | null | undefined): boolean {
  if (!pathOrName) {
    return false;
  }

  return HTML_FILE_RE.test(pathOrName.trim());
}

export async function openHtmlFileInExternalBrowser(path: string): Promise<void> {
  const { systemAPI } = await import('@/infrastructure/api/service-api/SystemAPI');
  await systemAPI.openHtmlFileInBrowser(path);
}
