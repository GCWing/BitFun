 

import { IMenuProvider } from '../types/provider.types';
import { MenuItem } from '../types/menu.types';
import { MenuContext } from '../types/context.types';
import { i18nService } from '../../../infrastructure/i18n';

export class GlobalMenuProvider implements IMenuProvider {
  readonly id = 'global';
  readonly name = i18nService.t('common:contextMenu.globalMenu.name');
  readonly description = i18nService.t('common:contextMenu.globalMenu.description');
  readonly priority = -1000; 

  matches(_context: MenuContext): boolean {
    
    return true;
  }

  async getMenuItems(_context: MenuContext): Promise<MenuItem[]> {
    const items: MenuItem[] = [];

    
    

    return items;
  }

  isEnabled(): boolean {
    return true;
  }
}

