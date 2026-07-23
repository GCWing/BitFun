 

import { BaseCommand } from '../../BaseCommand';
import { CommandResult } from '../../../types/command.types';
import { MenuContext, ContextType, FileNodeContext } from '../../../types/context.types';
import { i18nService } from '@/infrastructure/i18n';

export class CopyRelativePathCommand extends BaseCommand {
  constructor() {
    const t = i18nService.getT();
    super({
      id: 'file.copy-relative-path',
      label: t('common:file.copyRelativePath'),
      description: t('common:contextMenu.descriptions.copyRelativePath'),
      icon: 'Copy',
      category: 'file'
    });
  }

  canExecute(context: MenuContext): boolean {
    return context.type === ContextType.FILE_NODE || 
           context.type === ContextType.FOLDER_NODE;
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const t = i18nService.getT();
      const fileContext = context as FileNodeContext;
      
      
      let workspacePath = fileContext.workspacePath;
      
      if (!workspacePath) {
        
        const { workspaceManager } = await import('../../../../../infrastructure/services/business/workspaceManager');
        workspacePath = workspaceManager.getWorkspacePath();
      }
      
      
      let relativePath: string;
      
      if (workspacePath && fileContext.filePath) {
        
        const normalizedFilePath = fileContext.filePath.replace(/\\/g, '/');
        const normalizedWorkspacePath = workspacePath.replace(/\\/g, '/');
        
        
        if (normalizedFilePath.toLowerCase().startsWith(normalizedWorkspacePath.toLowerCase())) {
          
          relativePath = normalizedFilePath.slice(normalizedWorkspacePath.length);
          
          relativePath = relativePath.replace(/^[/\\]+/, '');
        } else {
          
          relativePath = fileContext.filePath;
        }
      } else {
        
        relativePath = fileContext.fileName;
      }
      
      if (navigator.clipboard) {
        await navigator.clipboard.writeText(relativePath);
      } else {
        const textarea = document.createElement('textarea');
        textarea.value = relativePath;
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
      }

      return this.success(t('common:contextMenu.status.copyRelativePathSuccess'), { path: relativePath });
    } catch (error) {
      const t = i18nService.getT();
      return this.failure(t('errors:contextMenu.copyRelativePathFailed'), error as Error);
    }
  }
}

