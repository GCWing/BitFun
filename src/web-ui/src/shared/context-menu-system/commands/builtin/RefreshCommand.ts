 

import { BaseCommand } from '../BaseCommand';
import { CommandResult } from '../../types/command.types';
import { MenuContext } from '../../types/context.types';
import { globalEventBus } from '../../../../infrastructure/event-bus';
import { i18nService } from '@/infrastructure/i18n';

export class RefreshCommand extends BaseCommand {
  constructor() {
    const t = i18nService.getT();
    super({
      id: 'refresh',
      label: t('common:actions.refresh'),
      description: t('common:contextMenu.descriptions.refresh'),
      icon: 'RefreshCw',
      shortcut: 'F5',
      category: 'view'
    });
  }

  async execute(context: MenuContext): Promise<CommandResult> {
    try {
      const t = i18nService.getT();
      
      globalEventBus.emit('context-menu:refresh', { context });
      
      return this.success(t('common:contextMenu.status.refreshTriggered'));
    } catch (error) {
      const t = i18nService.getT();
      return this.failure(t('errors:contextMenu.refreshFailed'), error as Error);
    }
  }
}

