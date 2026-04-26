import { downloadDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import { notificationService } from '@/shared/notification-system';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('captureElementToDownloadsPng');

const loadHtmlToImage = () => import('html-to-image');

export async function captureElementToDownloadsPng(
  element: HTMLElement,
  fileNamePrefix: string,
): Promise<void> {
  const computedStyle = getComputedStyle(document.documentElement);
  const bgColor = computedStyle.getPropertyValue('--color-bg-flowchat').trim() || '#121214';

  await new Promise((resolve) => setTimeout(resolve, 0));

  const htmlToImage = await loadHtmlToImage();
  let blob: Blob | null = null;

  try {
    blob = await htmlToImage.toBlob(element, {
      quality: 1,
      pixelRatio: 2,
      backgroundColor: bgColor,
      skipFonts: true,
      cacheBust: true,
    });
  } catch (e) {
    log.warn('toBlob failed, trying toPng', e);
    const dataUrl = await htmlToImage.toPng(element, {
      quality: 1,
      pixelRatio: 2,
      backgroundColor: bgColor,
      skipFonts: true,
      cacheBust: true,
    });
    const response = await fetch(dataUrl);
    blob = await response.blob();
  }

  if (!blob) {
    throw new Error(i18nService.t('flow-chat:exportImage.generateFailed'));
  }

  const timestampStr = i18nService
    .formatDate(new Date(), {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    })
    .replace(/[/:\s]/g, '-');
  const fileName = `${fileNamePrefix}_${timestampStr}.png`;
  const downloadsPath = await downloadDir();
  const filePath = await join(downloadsPath, fileName);

  const arrayBuffer = await blob.arrayBuffer();
  await writeFile(filePath, new Uint8Array(arrayBuffer));

  const plainSuccessMessage = i18nService.t('flow-chat:exportImage.exportSuccess', { filePath });
  const successPrefix = i18nService.t('flow-chat:exportImage.exportSuccessPrefix');

  const revealExportedFile = async () => {
    if (typeof window === 'undefined' || !('__TAURI__' in window)) {
      return;
    }
    try {
      await workspaceAPI.revealInExplorer(filePath);
    } catch (error) {
      log.error('Failed to reveal export path in file manager', { filePath, error });
    }
  };

  notificationService.success(plainSuccessMessage, {
    messageNode: (
      <>
        {successPrefix}
        <button
          type="button"
          className="notification-item__path-link"
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            void revealExportedFile();
          }}
        >
          {filePath}
        </button>
      </>
    ),
  });
}
