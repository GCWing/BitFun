import { downloadDir, join } from '@tauri-apps/api/path';
import { writeFile } from '@tauri-apps/plugin-fs';
import { notificationService } from '@/shared/notification-system';
import { i18nService } from '@/infrastructure/i18n';
import { workspaceAPI } from '@/infrastructure/api';
import { createLogger } from '@/shared/utils/logger';
import { withTimeout } from '@/shared/utils/timing';

const log = createLogger('captureElementToDownloadsPng');

const loadModernScreenshot = () => import('modern-screenshot');

/** Maximum time to wait for capture before aborting (ms). */
const CAPTURE_TIMEOUT_MS = 15_000;

export async function captureElementToDownloadsPng(
  element: HTMLElement,
  fileNamePrefix: string,
): Promise<void> {
  const computedStyle = getComputedStyle(document.documentElement);
  const bgColor = computedStyle.getPropertyValue('--color-bg-flowchat').trim() || '#121214';

  await new Promise((resolve) => setTimeout(resolve, 0));

  const modernScreenshot = await loadModernScreenshot();
  let blob: Blob | null = null;

  const captureOptions = {
    quality: 1,
    scale: 2,
    backgroundColor: bgColor,
    fetch: {
      bypassingCache: true,
    } as const,
    font: false as const,
    features: {
      removeControlCharacter: true,
      fixSvgXmlDecode: true,
    } as const,
  };

  // Strategy 1: domToBlob (primary — avoids data-URL overhead)
  try {
    blob = await withTimeout(
      modernScreenshot.domToBlob(element, { ...captureOptions, type: 'image/png' }),
      CAPTURE_TIMEOUT_MS,
      'domToBlob capture',
    );
  } catch (e) {
    log.warn('domToBlob failed, trying domToPng', e);

    // Strategy 2: domToPng (fallback)
    try {
      const dataUrl = await withTimeout(
        modernScreenshot.domToPng(element, captureOptions),
        CAPTURE_TIMEOUT_MS,
        'domToPng capture',
      );
      if (!dataUrl || dataUrl === 'data:,' || dataUrl.length < 100) {
        throw new Error('domToPng returned empty or corrupt image data');
      }
      const response = await fetch(dataUrl);
      blob = await response.blob();
    } catch (e2) {
      log.warn('domToPng also failed, trying reduced scale', e2);

      // Strategy 3: domToBlob with scale=1 (reduced memory)
      blob = await withTimeout(
        modernScreenshot.domToBlob(element, { ...captureOptions, scale: 1, type: 'image/png' }),
        CAPTURE_TIMEOUT_MS,
        'domToBlob reduced-scale capture',
      );
    }
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
