 

import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Image as ImageIcon, Eye } from 'lucide-react';
import { Modal, Button } from '@/component-library';
import type { ImageContext, ValidationResult, RenderOptions } from '../../../types/context';
import type { 
  ContextTransformer, 
  ContextValidator, 
  ContextCardRenderer 
} from '../../../services/ContextRegistry';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ImageContextValidator');



export class ImageContextTransformer implements ContextTransformer<'image'> {
  readonly type = 'image' as const;
  
  transform(context: ImageContext): unknown {
    
    
    
    return {
      type: 'image',
      id: context.id,
      image_path: context.imagePath || null, 
      data_url: context.dataUrl || null, 
      mime_type: context.mimeType,
      metadata: {
        name: context.imageName,
        width: context.width,
        height: context.height,
        file_size: context.fileSize,
        source: context.source,
        is_local: context.isLocal,
      }
    };
  }
  
  estimateSize(context: ImageContext): number {
    
    if (context.dataUrl) {
      return context.dataUrl.length;
    }
    return context.imagePath?.length || 100;
  }
}



export class ImageContextValidator implements ContextValidator<'image'> {
  readonly type = 'image' as const;
  
  async validate(context: ImageContext): Promise<ValidationResult> {
    try {
      
      if (!context.imagePath && !context.dataUrl) {
        return {
          valid: false,
          error: 'Image path or data must not be empty.'
        };
      }
      
      
      const supportedFormats = ['image/png', 'image/jpeg', 'image/jpg', 'image/gif', 'image/webp'];
      if (!supportedFormats.includes(context.mimeType)) {
        return {
          valid: false,
          error: `Unsupported image format: ${context.mimeType}`
        };
      }
      
      
      const maxSize = 20 * 1024 * 1024; // 20MB
      if (context.fileSize && context.fileSize > maxSize) {
        return {
          valid: false,
          error: `Image is too large (${(context.fileSize / 1024 / 1024).toFixed(2)}MB). Max supported size is 20MB.`
        };
      }
      
      
      if (context.isLocal && context.imagePath) {
        try {
          const exists = await invoke<boolean>('check_path_exists', {
            request: {
              path: context.imagePath
            }
          });
          
          if (!exists) {
            return {
              valid: false,
              error: 'Image file does not exist.'
            };
          }
        } catch (error) {
          log.error('Failed to check image file existence', error as Error);
          return {
            valid: false,
            error: 'Unable to check image file.'
          };
        }
      }
      
      return {
        valid: true,
        metadata: {
          size: context.fileSize,
          format: context.mimeType,
        }
      };
      
    } catch (error) {
      log.error('Validation failed', error as Error);
      return {
        valid: false,
        error: error instanceof Error ? error.message : 'Validation failed.'
      };
    }
  }
}



export class ImageCardRenderer implements ContextCardRenderer<'image'> {
  readonly type = 'image' as const;
  
  render(context: ImageContext, options?: RenderOptions): React.ReactElement {
    const { compact = false, interactive = true } = options || {};
    
    const [imagePreview, setImagePreview] = React.useState<string | null>(null);
    const [showFullImage, setShowFullImage] = React.useState(false);
    
    
    React.useEffect(() => {
      if (context.thumbnailUrl) {
        setImagePreview(context.thumbnailUrl);
      } else if (context.dataUrl) {
        setImagePreview(context.dataUrl);
      } else if (context.isLocal && context.imagePath) {
        
        
        setImagePreview(null);
      }
    }, [context]);
    
    return (
      <div className="context-card image-context-card" data-compact={compact}>
        <div className="context-card__header">
          <div className="context-card__icon">
            <ImageIcon size={16} />
          </div>
          <div className="context-card__info">
            <div className="context-card__title">{context.imageName}</div>
            {!compact && (
              <div className="context-card__meta">
                {context.width && context.height && (
                  <span>{context.width} × {context.height}</span>
                )}
                {context.fileSize && (
                  <span className="context-card__meta-separator">•</span>
                )}
                {context.fileSize && (
                  <span>{formatFileSize(context.fileSize)}</span>
                )}
              </div>
            )}
          </div>
        </div>
        
        {!compact && imagePreview && (
          <div className="context-card__preview">
            <div 
              className="image-context-card__thumbnail"
              onClick={() => interactive && setShowFullImage(true)}
              style={{ cursor: interactive ? 'pointer' : 'default' }}
            >
              <img 
                src={imagePreview} 
                alt={context.imageName}
                style={{ 
                  maxWidth: '100%', 
                  maxHeight: '200px',
                  objectFit: 'contain'
                }}
              />
            </div>
            {interactive && (
              <div className="image-context-card__actions">
                <Button 
                  variant="ghost"
                  size="small"
                  onClick={() => setShowFullImage(true)}
                >
                  <Eye size={14} />
                  <span>{i18nService.t('components:contextSystem.contextCard.viewLargeImage')}</span>
                </Button>
              </div>
            )}
          </div>
        )}
        
        
        <Modal
          isOpen={showFullImage && !!imagePreview}
          onClose={() => setShowFullImage(false)}
          title={context.imageName}
          size="large"
        >
          <div className="image-context-card__modal-content">
            <img 
              src={imagePreview || ''} 
              alt={context.imageName}
              style={{ maxWidth: '100%', maxHeight: '80vh', objectFit: 'contain' }}
            />
          </div>
        </Modal>
      </div>
    );
  }
}



function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
