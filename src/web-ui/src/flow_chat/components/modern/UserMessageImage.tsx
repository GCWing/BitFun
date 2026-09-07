import { useEffect, useState } from 'react';
import { workspaceAPI } from '@/infrastructure/api/service-api/WorkspaceAPI';
import { useI18n } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';
import type { ImageDisplayData } from '../../utils/imagePayload';
import { getMimeTypeFromFilename } from '../../utils/imageUtils';

const log = createLogger('UserMessageImage');

interface UserMessageImageProps {
  image: ImageDisplayData;
  onPreview: (source: string) => void;
}

export function UserMessageImage({ image, onPreview }: UserMessageImageProps) {
  const { t } = useI18n('tools');
  const { dataUrl, imagePath, mimeType, name } = image;
  const [loaded, setLoaded] = useState<{ path: string; source: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const source = dataUrl || (loaded && loaded.path === imagePath ? loaded.source : undefined);

  useEffect(() => {
    let disposed = false;
    setLoaded(null);
    setError(null);
    if (!dataUrl && imagePath) {
      // Attachment paths belong to the runtime host. The transport routes this
      // read to that host, including peer mode; never construct a local asset URL.
      void workspaceAPI.readFileContent(imagePath, 'base64').then(content => {
        if (!disposed) {
          setLoaded({ path: imagePath, source: `data:${mimeType || getMimeTypeFromFilename(imagePath)};base64,${content}` });
        }
      }).catch(cause => {
        if (!disposed) {
          log.warn('Failed to load message image', { imagePath, error: cause });
          setError(String(cause));
        }
      });
    }
    return () => { disposed = true; };
  }, [dataUrl, imagePath, mimeType]);

  const failure = error ? t('editor.imageViewer.loadImageFailedWithMessage', { message: error }) : null;
  return (
    <div
      data-openbitfun-component="user-message-item"
      data-openbitfun-part="image"
      className="user-message-item__image-thumb"
      onClick={event => { event.stopPropagation(); if (source && !error) onPreview(source); }}
      title={failure || name}
    >
      {failure ? <span role="alert">{failure}</span> : source ? (
        <img src={source} alt={name} onError={() => setError(name)} />
      ) : <span>{name}</span>}
    </div>
  );
}
