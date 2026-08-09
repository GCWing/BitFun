/**
 * Browser fallback for "save as file" flows.
 *
 * The desktop build saves through the Tauri dialog + fs plugins; the browser
 * build has no filesystem, so it hands the payload to the download manager.
 */

export function downloadTextFileInBrowser(
  fileName: string,
  content: string,
  mimeType = 'text/plain;charset=utf-8'
): void {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
}

export function downloadMarkdownInBrowser(fileName: string, markdown: string): void {
  downloadTextFileInBrowser(fileName, markdown, 'text/markdown;charset=utf-8');
}
