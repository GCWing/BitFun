 

import { BaseCommand } from '../../BaseCommand';
import { CommandResult } from '../../../types/command.types';
import { MenuContext, ContextType, FileNodeContext } from '../../../types/context.types';
import { globalEventBus } from '../../../../../infrastructure/event-bus';
import { i18nService } from '../../../../../infrastructure/i18n';

export class RenameCommand extends BaseCommand {
  constructor() {
    super({
      id: 'file.rename',
      label: i18nService.t('common:file.rename'),
      description: i18nService.t('common:file.renameDescription'),
      icon: 'Edit',
      shortcut: 'F2',
      category: 'file'
    });
  }

  canExecute(context: MenuContext): boolean {
    if (context.type === ContextType.FILE_NODE || context.type === ContextType.FOLDER_NODE) {
      return !(context as FileNodeContext).isReadOnly;
    }
    return false;
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const fileContext = context as FileNodeContext;
      
      globalEventBus.emit('file:rename', { 
        path: fileContext.filePath,
        name: fileContext.fileName 
      });

      return this.success(i18nService.t('common:file.renameStarted'));
    } catch (error) {
      return this.failure(i18nService.t('errors:file.renameFailed'), error as Error);
    }
  }
}

