/**
 * Image Viewer Component
 * 
 * Previews image files in the editor.
 * @module components/ImageViewer
 */

import React, { useState, useEffect, useCallback } from 'react';
import { ZoomIn, ZoomOut, RotateCw, Download, Maximize2 } from 'lucide-react';
import { createLogger } from '@/shared/utils/logger';
import { Tooltip } from '@/component-library';
import { useI18n } from '@/infrastructure/i18n';
import './ImageViewer.scss';

const log = createLogger('ImageViewer');

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
  const [imageUrl, setImageUrl] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(100);
  const [rotation, setRotation] = useState(0);
  const [imageDimensions, setImageDimensions] = useState<{ width: number; height: number } | null>(null);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [fileSize, setFileSize] = useState<number>(0);

  const getMimeType = useCallback((path: string): string => {
    const ext = path.toLowerCase().split('.').pop();
    const mimeTypes: Record<string, string> = {
      'jpg': 'image/jpeg',
      'jpeg': 'image/jpeg',
      'png': 'image/png',
      'gif': 'image/gif',
      'bmp': 'image/bmp',
      'webp': 'image/webp',
      'svg': 'image/svg+xml',
      'ico': 'image/x-icon',
      'avif': 'image/avif'
    };
    return mimeTypes[ext || ''] || 'image/jpeg';
  }, []);

  useEffect(() => {
    const loadImage = async () => {
      if (!filePath) {
        setError(t('editor.imageViewer.filePathEmpty'));
        setLoading(false);
        return;
      }

      try {
        setLoading(true);
        setError(null);

        const { workspaceAPI } = await import('@/infrastructure/api');
        const result = await workspaceAPI.readFileContent(filePath);

        const mimeType = getMimeType(filePath);

        const dataUrl = `data:${mimeType};base64,${result}`;
        
        setImageUrl(dataUrl);
        setFileSize(result.length);
        setLoading(false);
        
      } catch (err) {
        log.error('Failed to load image', err);
        setError(t('editor.imageViewer.loadImageFailedWithMessage', { message: String(err) }));
        setLoading(false);
      }
    };

    loadImage();
  }, [filePath, getMimeType, t]);

  const handleImageLoad = useCallback((e: React.SyntheticEvent<HTMLImageElement>) => {
    const img = e.currentTarget;
    
    setImageDimensions({
      width: img.naturalWidth,
      height: img.naturalHeight
    });
  }, []);

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

  /** Format file size (base64 length is roughly 1.33x original) */
  const formatFileSize = useCallback((base64Length: number): string => {
    const bytes = Math.round(base64Length * 0.75);
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }, []);

  return (
    <div className={`bitfun-image-viewer ${className} ${isFullscreen ? 'fullscreen' : ''}`}>
      <div className="bitfun-image-viewer__toolbar">
        <div className="bitfun-image-viewer__info">
          <span className="bitfun-image-viewer__filename">{fileName || filePath.split(/[/\\]/).pop()}</span>
          {imageDimensions && (
            <span className="bitfun-image-viewer__dimensions">
              {imageDimensions.width} × {imageDimensions.height}
            </span>
          )}
          {fileSize > 0 && (
            <span className="bitfun-image-viewer__filesize">
              {formatFileSize(fileSize)}
            </span>
          )}
        </div>
        <div className="bitfun-image-viewer__controls">
          <Tooltip content={t('editor.imageViewer.zoomOut')} placement="top">
            <button
              className="bitfun-image-viewer__btn"
              onClick={handleZoomOut}
              disabled={zoom <= 25}
            >
              <ZoomOut size={14} />
            </button>
          </Tooltip>
          <Tooltip content={t('editor.imageViewer.zoomReset')} placement="top">
            <button
              className="bitfun-image-viewer__btn bitfun-image-viewer__btn--zoom-display"
              onClick={handleZoomReset}
            >
              {zoom}%
            </button>
          </Tooltip>
          <Tooltip content={t('editor.imageViewer.zoomIn')} placement="top">
            <button
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
              className="bitfun-image-viewer__btn"
              onClick={handleRotate}
            >
              <RotateCw size={14} />
            </button>
          </Tooltip>
          <Tooltip content={t('editor.imageViewer.download')} placement="top">
            <button
              className="bitfun-image-viewer__btn"
              onClick={handleDownload}
            >
              <Download size={14} />
            </button>
          </Tooltip>
          <Tooltip
            content={isFullscreen ? t('editor.imageViewer.exitFullscreen') : t('editor.imageViewer.enterFullscreen')}
            placement="top"
          >
            <button
              className="bitfun-image-viewer__btn"
              onClick={handleToggleFullscreen}
            >
              <Maximize2 size={14} />
            </button>
          </Tooltip>
        </div>
      </div>

      <div className="bitfun-image-viewer__container">
        {loading && (
          <div className="bitfun-image-viewer__loading">
            <div className="bitfun-image-viewer__spinner" />
            <p>{t('editor.common.loading')}</p>
          </div>
        )}

        {error && (
          <div className="bitfun-image-viewer__error">
            <p>{error}</p>
            <p className="bitfun-image-viewer__error-path">{filePath}</p>
          </div>
        )}

        {!loading && !error && imageUrl && (
          <div className="bitfun-image-viewer__image-wrapper">
            <img
              src={imageUrl}
              alt={fileName || filePath}
              className="bitfun-image-viewer__image"
              style={{
                transform: `scale(${zoom / 100}) rotate(${rotation}deg)`,
              }}
              onLoad={handleImageLoad}
              onError={handleImageError}
            />
          </div>
        )}
        
        {!loading && !error && !imageUrl && (
          <div className="bitfun-image-viewer__error">
            <p>{t('editor.imageViewer.imageUrlEmpty')}</p>
          </div>
        )}
      </div>
    </div>
  );
};

export default ImageViewer;

