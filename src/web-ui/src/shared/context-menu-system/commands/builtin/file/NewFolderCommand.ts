 

import { BaseCommand } from '../../BaseCommand';
import { CommandResult } from '../../../types/command.types';
import { MenuContext, ContextType, FileNodeContext } from '../../../types/context.types';
import { globalEventBus } from '../../../../../infrastructure/event-bus';
import { i18nService } from '../../../../../infrastructure/i18n';
import { dirnameAbsolutePath } from '@/shared/utils/pathUtils';

export class NewFolderCommand extends BaseCommand {
  constructor() {
    super({
      id: 'file.new-folder',
      label: i18nService.t('common:file.newFolder'),
      description: i18nService.t('common:file.newFolderDescription'),
      icon: 'FolderPlus',
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

      globalEventBus.emit('file:new-folder', { parentPath });

      return this.success(i18nService.t('common:file.createFolderSuccess'), { parentPath });
    } catch (error) {
      return this.failure(i18nService.t('errors:file.createFolderFailed'), error as Error);
    }
  }
}

