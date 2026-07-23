 

import { MenuItem, MenuItemType } from '../types/menu.types';

 
export function createMenuItem(config: Partial<MenuItem> & { id: string; label: string }): MenuItem {
  return {
    type: MenuItemType.NORMAL,
    disabled: false,
    visible: true,
    ...config
  };
}

 
export function createSeparator(id?: string): MenuItem {
  return {
    id: id || `separator-${Date.now()}`,
    label: '',
    type: MenuItemType.SEPARATOR,
    separator: true
  };
}

 
export function createSubmenu(
  id: string,
  label: string,
  submenu: MenuItem[],
  config?: Partial<MenuItem>
): MenuItem {
  return {
    id,
    label,
    type: MenuItemType.SUBMENU,
    submenu,
    ...config
  };
}

 
export function createCheckbox(
  id: string,
  label: string,
  checked: boolean | ((context: any) => boolean),
  config?: Partial<MenuItem>
): MenuItem {
  return {
    id,
    label,
    type: MenuItemType.CHECKBOX,
    checked,
    ...config
  };
}

 
export function createRadio(
  id: string,
  label: string,
  checked: boolean | ((context: any) => boolean),
  config?: Partial<MenuItem>
): MenuItem {
  return {
    id,
    label,
    type: MenuItemType.RADIO,
    checked,
    ...config
  };
}

 
export function filterMenuItems(items: MenuItem[], predicate: (item: MenuItem) => boolean): MenuItem[] {
  return items
    .filter(predicate)
    .map(item => {
      if (item.submenu) {
        return {
          ...item,
          submenu: filterMenuItems(item.submenu, predicate)
        };
      }
      return item;
    });
}

 
export function groupMenuItems(items: MenuItem[]): MenuItem[] {
  const groups = new Map<string, MenuItem[]>();
  const ungrouped: MenuItem[] = [];

  
  items.forEach(item => {
    if (item.group) {
      const groupItems = groups.get(item.group) || [];
      groupItems.push(item);
      groups.set(item.group, groupItems);
    } else {
      ungrouped.push(item);
    }
  });

  
  const result: MenuItem[] = [];
  
  groups.forEach((groupItems, groupName) => {
    if (result.length > 0) {
      result.push(createSeparator(`group-${groupName}-sep`));
    }
    result.push(...groupItems);
  });

  if (ungrouped.length > 0 && result.length > 0) {
    result.push(createSeparator('ungrouped-sep'));
  }
  result.push(...ungrouped);

  return result;
}

