import { useEffect, useState } from 'react';
import { Icon } from '@openbitfun/ui';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { getActiveSurfaceScope } from '@/infrastructure/peer-device/deviceSurface';
import { useI18n } from '@/infrastructure/i18n';
import type { ImageContext } from '@/types/context';
import { getMimeTypeFromFilename } from '../utils/imageUtils';

export function ChatInputImagePreview({ image, surfaceEpoch }: {
  image: ImageContext;
  surfaceEpoch: number;
}) {
  const { t } = useI18n('tools');
  const embedded = image.thumbnailUrl || image.dataUrl;
  const path = image.imagePath;
  const [loaded, setLoaded] = useState<{ path: string; epoch: number; source: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const source = embedded || (loaded?.path === path && loaded.epoch === surfaceEpoch ? loaded.source : undefined);

  useEffect(() => {
    let cancelled = false;
    setLoaded(null);
    setError(null);
    if (!embedded && path) {
      // Paths belong to the runtime host, just like sent-message attachments.
      // Read through the transport; never turn them into controller asset URLs.
      void workspaceAPI.readFileContent(path, 'base64').then(content => {
        if (!cancelled && getActiveSurfaceScope().epoch === surfaceEpoch) {
          setLoaded({ path, epoch: surfaceEpoch,
            source: `data:${image.mimeType || getMimeTypeFromFilename(path)};base64,${content}` });
        }
      }).catch(cause => {
        if (!cancelled && getActiveSurfaceScope().epoch === surfaceEpoch) setError(String(cause));
      });
    }
    return () => { cancelled = true; };
  }, [embedded, path, image.mimeType, surfaceEpoch]);

  return source && !error ? (
    <img className="openbitfun-chat-input__image-chip-thumb"
      data-openbitfun-component="chat-input" data-openbitfun-part="imagePreview"
      src={source} alt={image.imageName} onError={() => setError(image.imageName)} />
  ) : (
    <div className="openbitfun-chat-input__image-chip-thumb openbitfun-chat-input__image-chip-thumb--placeholder"
      data-openbitfun-component="chat-input" data-openbitfun-part="imagePreview"
      role={error ? 'img' : undefined}
      aria-label={error ? t('editor.imageViewer.loadImageFailedWithMessage', { message: error }) : undefined}
      title={error ? t('editor.imageViewer.loadImageFailedWithMessage', { message: error }) : image.imageName}>
      <Icon name="image" size="sm" />
    </div>
  );
}
