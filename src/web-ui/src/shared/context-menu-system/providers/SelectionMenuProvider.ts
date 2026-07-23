 

import { IMenuProvider } from '../types/provider.types';
import { MenuItem } from '../types/menu.types';
import { MenuContext, ContextType, SelectionContext } from '../types/context.types';
import { commandExecutor } from '../commands/CommandExecutor';
import { i18nService } from '@/infrastructure/i18n';

export class SelectionMenuProvider implements IMenuProvider {
  readonly id = 'selection';
  readonly name = i18nService.t('common:contextMenu.selectionMenu.name');
  readonly description = i18nService.t('common:contextMenu.selectionMenu.description');
  readonly priority = 100;

  matches(context: MenuContext): boolean {
    return context.type === ContextType.SELECTION;
  }

  async getMenuItems(context: MenuContext): Promise<MenuItem[]> {
    const selectionContext = context as SelectionContext;
    const items: MenuItem[] = [];

    
    items.push({
      id: 'selection-copy',
      label: i18nService.t('common:actions.copy'),
      icon: 'Copy',
      shortcut: 'Ctrl+C',
      command: 'copy',
      onClick: async (ctx) => {
        await commandExecutor.execute('copy', ctx);
      }
    });

    
    if (selectionContext.isEditable) {
      items.push({
        id: 'selection-cut',
        label: i18nService.t('common:actions.cut'),
        icon: 'Scissors',
        shortcut: 'Ctrl+X',
        command: 'cut',
        onClick: async (ctx) => {
          await commandExecutor.execute('cut', ctx);
        }
      });
    }

    return items;
  }

  isEnabled(): boolean {
    return true;
  }
}

