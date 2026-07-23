 

import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FileIcon, CheckCircle } from 'lucide-react';
import type { FileContext, ValidationResult, RenderOptions } from '../../../types/context';
import type { 
  ContextTransformer, 
  ContextValidator, 
  ContextCardRenderer 
} from '../../../services/ContextRegistry';
import { i18nService } from '@/infrastructure/i18n';



export class FileContextTransformer implements ContextTransformer<'file'> {
  readonly type = 'file' as const;
  
  transform(context: FileContext): unknown {
    
    return {
      type: 'file',
      path: context.relativePath || context.filePath,
      name: context.fileName,
      metadata: {
        size: context.fileSize,
        mimeType: context.mimeType
      }
    };
  }
  
  estimateSize(context: FileContext): number {
    return context.fileSize || 0;
  }
}



export class FileContextValidator implements ContextValidator<'file'> {
  readonly type = 'file' as const;
  
  async validate(context: FileContext): Promise<ValidationResult> {
    try {
      
      const exists = await invoke<boolean>('fs_exists', { path: context.filePath });
      
      if (!exists) {
        return {
          valid: false,
          error: 'File does not exist.'
        };
      }
      
      
      const warnings: string[] = [];
      if (context.fileSize) {
        if (context.fileSize > 10 * 1024 * 1024) { // 10MB
          warnings.push(i18nService.t('components:contextSystem.validation.warnings.fileLarge', { size: 10 }));
        }
        if (context.fileSize > 50 * 1024 * 1024) { // 50MB
          return {
            valid: false,
            error: 'File is too large (>50MB). Cannot process.'
          };
        }
      }
      
      return {
        valid: true,
        warnings: warnings.length > 0 ? warnings : undefined
      };
    } catch (error) {
      return {
        valid: false,
        error: `Validation failed: ${String(error)}`
      };
    }
  }
  
  quickValidate(context: FileContext): ValidationResult {
    
    if (!context.filePath || context.filePath.trim() === '') {
      return { valid: false, error: 'File path is empty.' };
    }
    
    if (context.fileSize && context.fileSize > 50 * 1024 * 1024) {
      return { valid: false, error: 'File is too large (>50MB).' };
    }
    
    return { valid: true };
  }
}



export class FileCardRenderer implements ContextCardRenderer<'file'> {
  readonly type = 'file' as const;
  
  render(context: FileContext, options?: RenderOptions): React.ReactNode {
    const { compact = false, interactive = true } = options || {};
    
    return (
      <div className={`bitfun-context-card bitfun-context-card--file ${compact ? 'bitfun-context-card--compact' : ''}`}>
        <div className="bitfun-context-card__icon">
          <FileIcon size={compact ? 16 : 20} />
        </div>
        
        <div className="bitfun-context-card__content">
          <div className="bitfun-context-card__title">
            {context.fileName}
          </div>
          
          {!compact && (
            <div className="bitfun-context-card__subtitle">
              {context.relativePath || context.filePath}
              {context.fileSize && (
                <span className="bitfun-context-card__meta">
                  {' • '}{this.formatFileSize(context.fileSize)}
                </span>
              )}
            </div>
          )}
        </div>
        
        {interactive && (
          <div className="bitfun-context-card__actions">
            {this.renderValidationIndicator(context)}
          </div>
        )}
      </div>
    );
  }
  
  private formatFileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  
  private renderValidationIndicator(_context: FileContext): React.ReactNode {
    
    
    return (
      <div className="bitfun-context-card__status">
        <CheckCircle size={16} className="bitfun-context-card__status-icon--success" />
      </div>
    );
  }
}



export function getFileIcon(_fileName: string): React.ReactNode {
  
  
  // const ext = fileName.split('.').pop()?.toLowerCase();
  
  
  return <FileIcon size={16} />;
}



export function formatFilePath(path: string, maxLength: number = 50): string {
  if (path.length <= maxLength) return path;
  
  const parts = path.split(/[/\\]/);
  if (parts.length <= 2) {
    return `...${path.slice(-maxLength)}`;
  }
  
  
  return `${parts[0]}/.../${parts.slice(-2).join('/')}`;
}
