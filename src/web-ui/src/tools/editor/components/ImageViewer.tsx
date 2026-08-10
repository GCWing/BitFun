/**
 * Image Viewer Component
 * 
 * Previews image files in the editor.
 * @module components/ImageViewer
 */

import React, { useState, useEffect, useCallback } from 'react';
import { ZoomIn, ZoomOut, RotateCw, Download, Maximize2 } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import { createBrowserImageDataUrl, getImageMimeType, isTiffPath } from '@/shared/utils/imageDataUrl';
import { apiClient } from '@/infrastructure/api/service-api/ApiClient';
import { TauriTransportAdapter } from '@/infrastructure/api/adapters';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import './ImageViewer.scss';

const log = createLogger('ImageViewer');

const MIN_SMALL_IMAGE_DISPLAY_SIZE = 32;
const MAX_IMAGE_PREVIEW_BYTES = 64 * 1024 * 1024;
const MAX_TIFF_PREVIEW_BYTES = 16 * 1024 * 1024;
const MAX_CACHED_IMAGE_BYTES = 128 * 1024 * 1024;
const imageDataCache = new Map<string, { dataUrl: string; byteLength: number }>();
const imageLoadPromises = new Map<string, Promise<{ dataUrl: string; byteLength: number }>>();
let cachedImageBytes = 0;

export function getImagePreviewLimit(filePath: string): number {
  return isTiffPath(filePath) ? MAX_TIFF_PREVIEW_BYTES : MAX_IMAGE_PREVIEW_BYTES;
}

export function isImagePreviewAllowed(filePath: string, byteLength: number): boolean {
  return byteLength <= getImagePreviewLimit(filePath);
}

function formatFileSizeValue(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function cacheImage(filePath: string, dataUrl: string, byteLength: number): void {
  if (byteLength > MAX_CACHED_IMAGE_BYTES) {
    if (dataUrl.startsWith('blob:')) {
      URL.revokeObjectURL(dataUrl);
    }
    return;
  }

  const previous = imageDataCache.get(filePath);
  if (previous) {
    cachedImageBytes -= previous.byteLength;
    if (previous.dataUrl !== dataUrl && previous.dataUrl.startsWith('blob:')) {
      URL.revokeObjectURL(previous.dataUrl);
    }
  }

  imageDataCache.delete(filePath);
  imageDataCache.set(filePath, { dataUrl, byteLength });
  cachedImageBytes += byteLength;

  while (cachedImageBytes > MAX_CACHED_IMAGE_BYTES && imageDataCache.size > 1) {
    const oldest = imageDataCache.entries().next().value as [string, { dataUrl: string; byteLength: number }] | undefined;
    if (!oldest) break;
    imageDataCache.delete(oldest[0]);
    cachedImageBytes -= oldest[1].byteLength;
    if (oldest[1].dataUrl.startsWith('blob:')) {
      URL.revokeObjectURL(oldest[1].dataUrl);
    }
  }
}

export function shouldUseBinaryImageTransfer(filePath: string): boolean {
  return !isTiffPath(filePath) && apiClient.getAdapter() instanceof TauriTransportAdapter;
}

export function getSmallImageDisplayScale(width: number, height: number): number {
  if (width <= 0 || height <= 0 || width > MIN_SMALL_IMAGE_DISPLAY_SIZE || height > MIN_SMALL_IMAGE_DISPLAY_SIZE) {
    return 1;
  }

  return Math.max(1, MIN_SMALL_IMAGE_DISPLAY_SIZE / Math.max(width, height));
}

export interface ImageViewerProps {
  /** Image file path */
  filePath: string;
  /** File name */
  fileName?: string;
  /** Workspace path (for relative path resolution) */
  workspacePath?: string;
  /** CSS class name */
  className?: string;
}

export const ImageViewer: React.FC<ImageViewerProps> = ({
  filePath,
  fileName,
  className = ''
}) => {
  const { t } = useI18n('tools');
  const tRef = React.useRef(t);
  tRef.current = t;
  const [imageUrl, setImageUrl] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(100);
  const [displayScale, setDisplayScale] = useState(1);
  const [rotation, setRotation] = useState(0);
  const [imageDimensions, setImageDimensions] = useState<{ width: number; height: number } | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [fileSize, setFileSize] = useState<number>(0);

  useEffect(() => {
    let cancelled = false;

    const loadImage = async () => {
      if (!filePath) {
        setError(tRef.current('editor.imageViewer.filePathEmpty'));
        setLoading(false);
        return;
      }

      const cached = imageDataCache.get(filePath);
      if (cached) {
        imageDataCache.delete(filePath);
        imageDataCache.set(filePath, cached);
        setImageUrl(cached.dataUrl);
        setFileSize(cached.byteLength);
        setLoading(false);
        setError(null);
        return;
      }

      try {
        setLoading(true);
        setError(null);
        setImageDimensions(null);
        setDisplayScale(1);

        const { workspaceAPI } = await import('@/infrastructure/api');
        const metadata = await workspaceAPI.getFileMetadata(filePath);
        const previewLimit = getImagePreviewLimit(filePath);
        setFileSize(metadata.size);

        if (!isImagePreviewAllowed(filePath, metadata.size)) {
          setError(tRef.current('editor.imageViewer.fileTooLarge', {
            size: formatFileSizeValue(metadata.size),
            limit: formatFileSizeValue(previewLimit),
          }));
          setLoading(false);
          return;
        }

        let loadPromise = imageLoadPromises.get(filePath);
        if (!loadPromise) {
          loadPromise = (async () => {
            if (shouldUseBinaryImageTransfer(filePath)) {
              const bytes = await workspaceAPI.readFileBinary(filePath);
              return {
                byteLength: bytes.byteLength,
                dataUrl: URL.createObjectURL(new Blob([bytes], { type: getImageMimeType(filePath) })),
              };
            }

            const result = await workspaceAPI.readFileContent(filePath, 'base64');
            return {
              byteLength: Math.round(result.length * 0.75),
              dataUrl: await createBrowserImageDataUrl(filePath, result),
            };
          })();
          imageLoadPromises.set(filePath, loadPromise);
          const clearLoadPromise = () => {
            if (imageLoadPromises.get(filePath) === loadPromise) {
              imageLoadPromises.delete(filePath);
            }
          };
          void loadPromise.then(clearLoadPromise, clearLoadPromise);
        }

        const { dataUrl, byteLength } = await loadPromise;
        cacheImage(filePath, dataUrl, byteLength);

        if (cancelled) {
          return;
        }

        setImageUrl(dataUrl);
        setFileSize(byteLength);
      } catch (err) {
        log.error('Failed to load image', err);
        if (!cancelled) {
          setError(tRef.current('editor.imageViewer.loadImageFailedWithMessage', { message: String(err) }));
          setLoading(false);
        }
      }
    };

    loadImage();
    return () => {
      cancelled = true;
    };
  }, [filePath]);

  const handleImageLoad = useCallback((e: React.SyntheticEvent<HTMLImageElement>) => {
    const img = e.currentTarget;
    cacheImage(filePath, imageUrl, fileSize);
    setLoading(false);

    setImageDimensions({
      width: img.naturalWidth,
      height: img.naturalHeight
    });
    setDisplayScale(getSmallImageDisplayScale(img.naturalWidth, img.naturalHeight));
  }, [filePath, fileSize, imageUrl]);

  const handleImageError = useCallback((e: React.SyntheticEvent<HTMLImageElement>) => {
    log.error('Image load error', { filePath, srcLength: e.currentTarget.src.length });
    setError(t('editor.imageViewer.decodeFailed'));
    setLoading(false);
  }, [filePath, t]);

  const handleZoomIn = useCallback(() => {
    setZoom(prev => Math.min(prev + 25, 500));
  }, []);

  const handleZoomOut = useCallback(() => {
    setZoom(prev => Math.max(prev - 25, 25));
  }, []);

  const handleZoomReset = useCallback(() => {
    setZoom(100);
  }, []);

  const handleRotate = useCallback(() => {
    setRotation(prev => (prev + 90) % 360);
  }, []);

  const handleDownload = useCallback(async () => {
    try {
      const name = fileName || filePath.split(/[/\\]/).pop() || 'image';
      const link = document.createElement('a');
      link.href = imageUrl;
      link.download = name;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
    } catch (err) {
      log.error('Failed to download image', err);
    }
  }, [imageUrl, fileName, filePath]);

  const handleToggleFullscreen = useCallback(() => {
    setIsFullscreen(prev => !prev);
  }, []);

  return (
    <div
      className={`bitfun-image-viewer ${className} ${isFullscreen ? 'fullscreen' : ''}`}
      data-bf-component="image-viewer"
      data-bf-part="root"
      data-bf-state={isFullscreen ? 'fullscreen' : undefined}
    >
      <div data-bf-component="image-viewer" data-bf-part="toolbar" className="bitfun-image-viewer__toolbar">
        <div data-bf-component="image-viewer" data-bf-part="info" className="bitfun-image-viewer__info">
          <span className="bitfun-image-viewer__filename">{fileName || filePath.split(/[/\\]/).pop()}</span>
          {imageDimensions && (
            <span className="bitfun-image-viewer__dimensions">
              {imageDimensions.width} × {imageDimensions.height}
            </span>
          )}
          {fileSize > 0 && (
            <span className="bitfun-image-viewer__filesize">
              {formatFileSizeValue(fileSize)}
            </span>
          )}
        </div>
        <div data-bf-component="image-viewer" data-bf-part="controls" className="bitfun-image-viewer__controls">
          <Tooltip content={t('editor.imageViewer.zoomOut')} placement="top">
            <button
              data-bf-component="image-viewer"
              data-bf-part="action"
              className="bitfun-image-viewer__btn"
              onClick={handleZoomOut}
              disabled={zoom <= 25}
            >
              <ZoomOut size={14} />
            </button>
          </Tooltip>
          <Tooltip content={t('editor.imageViewer.zoomReset')} placement="top">
            <button
              data-bf-component="image-viewer"
              data-bf-part="action"
              className="bitfun-image-viewer__btn bitfun-image-viewer__btn--zoom-display"
              onClick={handleZoomReset}
            >
              {zoom}%
            </button>
          </Tooltip>
          <Tooltip content={t('editor.imageViewer.zoomIn')} placement="top">
            <button
              data-bf-component="image-viewer"
              data-bf-part="action"
              className="bitfun-image-viewer__btn"
              onClick={handleZoomIn}
              disabled={zoom >= 500}
            >
              <ZoomIn size={14} />
            </button>
          </Tooltip>
          <div className="bitfun-image-viewer__divider" />
          <Tooltip content={t('editor.imageViewer.rotate90')} placement="top">
            <button
              data-bf-component="image-viewer"
              data-bf-part="action"
              className="bitfun-image-viewer__btn"
              onClick={handleRotate}
            >
              <RotateCw size={14} />
            </button>
          </Tooltip>
          <Tooltip content={t('editor.imageViewer.download')} placement="top">
            <button
              data-bf-component="image-viewer"
              data-bf-part="action"
              className="bitfun-image-viewer__btn"
              onClick={handleDownload}
              style={{display: 'none'}}
            >
              <Download size={14} />
            </button>
          </Tooltip>
          <Tooltip
            content={isFullscreen ? t('editor.imageViewer.exitFullscreen') : t('editor.imageViewer.enterFullscreen')}
            placement="top"
          >
            <button
              data-bf-component="image-viewer"
              data-bf-part="action"
              className="bitfun-image-viewer__btn"
              onClick={handleToggleFullscreen}
              style={{display: 'none'}}
            >
              <Maximize2 size={14} />
            </button>
          </Tooltip>
        </div>
      </div>

      <div data-bf-component="image-viewer" data-bf-part="container" className="bitfun-image-viewer__container">
        {loading && (
          <div data-bf-component="image-viewer" data-bf-part="loading" className="bitfun-image-viewer__loading">
            <div className="bitfun-image-viewer__spinner" />
            <p>{t('editor.common.loading')}</p>
          </div>
        )}

        {error && (
          <div data-bf-component="image-viewer" data-bf-part="error" className="bitfun-image-viewer__error">
            <p>{error}</p>
            <p className="bitfun-image-viewer__error-path">{filePath}</p>
          </div>
        )}

        {!error && imageUrl && (
          <div className="bitfun-image-viewer__image-wrapper" data-bf-component="image-viewer" data-bf-part="imageWrapper">
            <img
              src={imageUrl}
              alt={fileName || filePath}
              draggable={false}
              className="bitfun-image-viewer__image"
              data-bf-component="image-viewer"
              data-bf-part="image"
              style={{
                width: imageDimensions ? `${imageDimensions.width * displayScale * zoom / 100}px` : undefined,
                height: imageDimensions ? `${imageDimensions.height * displayScale * zoom / 100}px` : undefined,
                transform: `rotate(${rotation}deg)`,
                imageRendering: displayScale > 1 ? 'pixelated' : undefined,
                borderRadius: 0,
              }}
              onLoad={handleImageLoad}
              onError={handleImageError}
            />
          </div>
        )}
        
        {!loading && !error && !imageUrl && (
          <div data-bf-component="image-viewer" data-bf-part="error" className="bitfun-image-viewer__error">
            <p>{t('editor.imageViewer.imageUrlEmpty')}</p>
          </div>
        )}
      </div>
    </div>
  );
};

export default ImageViewer;

