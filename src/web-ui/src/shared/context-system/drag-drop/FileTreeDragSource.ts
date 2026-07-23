 

import type { FileSystemNode } from '../../../tools/file-system/types';
import type { FileContext, DirectoryContext, ImageContext, ContextItem } from '../../types/context';
import type { IDragSource, DragPayload, PreviewData } from '../../types/drag';
import { isImageFile, getMimeTypeFromFilename } from '../../../flow_chat/utils/imageUtils';
import { i18nService } from '@/infrastructure/i18n';
export class FileTreeDragSource implements IDragSource<FileSystemNode> {
  readonly sourceId = 'file-tree-primary';
  readonly sourceType = 'file-tree' as const;
  
  createPayload(node: FileSystemNode): DragPayload<ContextItem> {
    
    const id = `context-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
    
    
    let contextData: ContextItem;
    
    if (node.isDirectory) {
      
      contextData = {
        id,
        type: 'directory',
        directoryPath: node.path,
        directoryName: node.name,
        recursive: false, 
        timestamp: Date.now(),
        metadata: {
          isDirectory: true
        }
      } as DirectoryContext;
    } else {
      
      if (isImageFile(node.name)) {
        
        contextData = {
          id,
          type: 'image',
          imagePath: node.path,
          imageName: node.name,
          fileSize: node.size || 0,
          mimeType: getMimeTypeFromFilename(node.name),
          source: 'file',
          isLocal: true,
          timestamp: Date.now(),
          metadata: {
            isDirectory: false,
            isImage: true
          }
        } as ImageContext;
      } else {
        
        contextData = {
          id,
          type: 'file',
          filePath: node.path,
          fileName: node.name,
          fileSize: node.size,
          timestamp: Date.now(),
          metadata: {
            isDirectory: false
          }
        } as FileContext;
      }
    }
    
    
    let dataType: 'file' | 'directory' | 'image' = 'file';
    if (node.isDirectory) {
      dataType = 'directory';
    } else if (contextData.type === 'image') {
      dataType = 'image';
    }
    
    const payload: DragPayload<ContextItem> = {
      id,
      sourceType: this.sourceType,
      dataType: dataType as any,
      timestamp: Date.now(),
      data: contextData,
      metadata: {
        sourceId: this.sourceId,
        sourcePath: node.path.split(/[/\\]/),
        preview: this.generatePreview(node, contextData)
      }
    };
    
    return payload;
  }
  
  generatePreview(node: FileSystemNode, contextData?: ContextItem): PreviewData {
    
    if (contextData?.type === 'image') {
      return {
        type: 'text',
        title: node.name,
        subtitle: this.formatFileSize(node.size || 0)
      };
    }
    
    return {
      type: 'text',
      title: node.name,
      subtitle: node.isDirectory 
        ? i18nService.t('common:file.folder') 
        : this.formatFileSize(node.size || 0)
    };
  }
  
  onDragStart(): void {
    
  }
  
  onDragEnd(): void {
    
  }
  
  private formatFileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
}


export const fileTreeDragSource = new FileTreeDragSource();
