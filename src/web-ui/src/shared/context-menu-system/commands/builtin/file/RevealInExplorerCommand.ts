 

import { BaseCommand } from '../../BaseCommand';
import { CommandResult } from '../../../types/command.types';
import { MenuContext, ContextType, FileNodeContext, TabContext } from '../../../types/context.types';
import { globalEventBus } from '../../../../../infrastructure/event-bus';
import { i18nService } from '../../../../../infrastructure/i18n';
import { workspaceManager } from '../../../../../infrastructure/services/business/workspaceManager';
import { isRemoteWorkspace } from '../../../../../shared/types';

function getContextFilePath(context: MenuContext): string | undefined {
  if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
    return (context as FileNodeContext).filePath;
  }

  if (context.type === ContextType.TAB) {
    return (context as TabContext).filePath;
  }

  return undefined;
}

export class RevealInExplorerCommand extends BaseCommand {
  constructor() {
    super({
      id: 'file.reveal-in-explorer',
      label: i18nService.t('common:file.reveal'),
      description: i18nService.t('common:file.revealDescription'),
      icon: 'FolderOpen',
      category: 'file'
    });
  }

  canExecute(context: MenuContext): boolean {
    const isFileOrFolder =
      context.type === ContextType.FILE_NODE ||
      context.type === ContextType.FOLDER_NODE ||
      context.type === ContextType.TAB;
    if (!isFileOrFolder) return false;
    if (isRemoteWorkspace(workspaceManager.getState().currentWorkspace)) return false;
    return Boolean(getContextFilePath(context));
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const filePath = getContextFilePath(context);
      if (!filePath) {
        return this.failure(i18nService.t('errors:file.revealFailed'));
      }
      
      globalEventBus.emit('file:reveal', { path: filePath });

      return this.success(i18nService.t('common:file.revealOpening'));
    } catch (error) {
      return this.failure(i18nService.t('errors:file.revealFailed'), error as Error);
    }
  }
}

