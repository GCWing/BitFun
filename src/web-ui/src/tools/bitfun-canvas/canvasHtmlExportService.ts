import { createLogger } from '@/shared/utils/logger';

const log = createLogger('CanvasHtmlExportService');

interface ExportCanvasHtmlOptions {
  html: string;
  title: string;
}

export interface ExportCanvasHtmlResult {
  path?: string;
  downloaded: boolean;
}

function isTauriDesktop(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

export function safeCanvasHtmlFileName(title: string): string {
  const stem = title
    .trim()
    .replace(/[\\/:*?"<>|]+/g, '-')
    .replace(/\s+/g, ' ')
    .slice(0, 96)
    .trim();
  const safeStem = stem || 'BitFun Canvas';
  const stemWithoutHtmlSuffix = safeStem.replace(/\.html?$/i, '');
  return `${stemWithoutHtmlSuffix}.html`;
}

function defaultHtmlFileName(title: string): string {
  const stem = safeCanvasHtmlFileName(title);
  return /\.html?$/i.test(stem) ? stem : `${stem}.html`;
}

function downloadHtmlInBrowser(fileName: string, html: string): void {
  const blob = new Blob([html], { type: 'text/html;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = fileName;
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  URL.revokeObjectURL(url);
}

export async function exportCanvasHtml({
  html,
  title,
}: ExportCanvasHtmlOptions): Promise<ExportCanvasHtmlResult | null> {
  const fileName = defaultHtmlFileName(title);

  if (!isTauriDesktop()) {
    downloadHtmlInBrowser(fileName, html);
    return { downloaded: true };
  }

  const [{ save: dialogSave }, { writeFile }] = await Promise.all([
    import('@tauri-apps/plugin-dialog'),
    import('@tauri-apps/plugin-fs'),
  ]);
  const path = await dialogSave({
    title: 'Export Canvas HTML',
    defaultPath: fileName,
    filters: [{
      name: 'HTML',
      extensions: ['html', 'htm'],
    }],
  });

  if (!path) return null;

  await writeFile(path, new TextEncoder().encode(html));
  log.info('Canvas HTML exported', { path, bytes: html.length });
  return { path, downloaded: false };
}
