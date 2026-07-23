 

import { BaseCommand } from '../../BaseCommand';
import { CommandResult } from '../../../types/command.types';
import { MenuContext, ContextType, FileNodeContext, TabContext } from '../../../types/context.types';
import { i18nService } from '@/infrastructure/i18n';

function getContextFilePath(context: MenuContext): string | undefined {
  if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
    return (context as FileNodeContext).filePath;
  }

  if (context.type === ContextType.TAB) {
    return (context as TabContext).filePath;
  }

  return undefined;
}

export class CopyPathCommand extends BaseCommand {
  constructor() {
    const t = i18nService.getT();
    super({
      id: 'file.copy-path',
      label: t('common:file.copyPath'),
      description: t('common:contextMenu.descriptions.copyPath'),
      icon: 'Copy',
      category: 'file'
    });
  }

  canExecute(context: MenuContext): boolean {
    if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
      return true;
    }

    return context.type === ContextType.TAB && Boolean((context as TabContext).filePath);
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const t = i18nService.getT();
      let filePath = getContextFilePath(context);

      if (!filePath) {
        return this.failure(t('errors:contextMenu.copyPathFailed'));
      }

      // Convert forward slashes back to native backslashes on Windows-style
      // paths (drive letters or UNC) so the clipboard yields OS-native paths.
      if (/^[a-zA-Z]:\//.test(filePath) || filePath.startsWith('//')) {
        filePath = filePath.replace(/\//g, '\\');
      }

      if (navigator.clipboard) {
        await navigator.clipboard.writeText(filePath);
      } else {
        
        const textarea = document.createElement('textarea');
        textarea.value = filePath;
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
      }

      return this.success(t('common:contextMenu.status.copyPathSuccess'), { path: filePath });
    } catch (error) {
      const t = i18nService.getT();
      return this.failure(t('errors:contextMenu.copyPathFailed'), error as Error);
    }
  }
}

