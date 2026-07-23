 

import { MenuItem } from '../types/menu.types';
import { i18nService } from '../../../infrastructure/i18n';

 
export const DEFAULT_GLOBAL_MENUS: MenuItem[] = [
  {
    id: 'refresh',
    label: i18nService.t('common:actions.refresh'),
    icon: 'RefreshCw',
    shortcut: 'F5',
    command: 'refresh'
  }
];

 
export const DEFAULT_EDIT_MENUS: MenuItem[] = [
  {
    id: 'copy',
    label: i18nService.t('common:actions.copy'),
    icon: 'Copy',
    shortcut: 'Ctrl+C',
    command: 'copy'
  },
  {
    id: 'cut',
    label: i18nService.t('common:actions.cut'),
    icon: 'Scissors',
    shortcut: 'Ctrl+X',
    command: 'cut'
  },
  {
    id: 'paste',
    label: i18nService.t('common:actions.paste'),
    icon: 'Clipboard',
    shortcut: 'Ctrl+V',
    command: 'paste'
  },
  {
    id: 'separator-1',
    label: '',
    separator: true
  },
  {
    id: 'select-all',
    label: i18nService.t('common:actions.selectAll'),
    shortcut: 'Ctrl+A',
    command: 'select-all'
  }
];

 
export const DEFAULT_MENU_OPTIONS = {
  showIcons: true,
  showShortcuts: true,
  enableSearch: false,
  autoAdjustPosition: true,
  closeDelay: 0,
  submenuOpenDelay: 200,
  minWidth: 180
};

