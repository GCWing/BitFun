 

import { BaseCommand } from '../../BaseCommand';
import { CommandResult } from '../../../types/command.types';
import { MenuContext, ContextType, FileNodeContext } from '../../../types/context.types';
import { globalEventBus } from '../../../../../infrastructure/event-bus';
import { i18nService } from '../../../../../infrastructure/i18n';
import { dirnameAbsolutePath } from '@/shared/utils/pathUtils';

export class NewFileCommand extends BaseCommand {
  constructor() {
    super({
      id: 'file.new-file',
      label: i18nService.t('common:file.newFile'),
      description: i18nService.t('common:file.newFileDescription'),
      icon: 'FilePlus',
      category: 'file'
    });
  }

  canExecute(context: MenuContext): boolean {
    if (context.type === ContextType.FOLDER_NODE) {
      return !(context as FileNodeContext).isReadOnly;
    }
    if (context.type === ContextType.FILE_NODE) {
      return !(context as FileNodeContext).isReadOnly;
    }
    return false;
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const fileContext = context as FileNodeContext;
      const parentPath = fileContext.isDirectory
        ? fileContext.filePath
        : dirnameAbsolutePath(fileContext.filePath);

      
      globalEventBus.emit('file:new-file', { parentPath });

      return this.success(i18nService.t('common:file.createFileSuccess'), { parentPath });
    } catch (error) {
      return this.failure(i18nService.t('errors:file.createFileFailed'), error as Error);
    }
  }
}

