 

import { MenuItem, MenuItemType } from '../types/menu.types';

 
export function mergeMenuItems(itemArrays: MenuItem[][], withSeparators: boolean = true): MenuItem[] {
  const result: MenuItem[] = [];

  itemArrays.forEach((items, index) => {
    if (items.length > 0) {
      if (result.length > 0 && withSeparators) {
        result.push({
          id: `separator-${index}`,
          label: '',
          type: MenuItemType.SEPARATOR,
          separator: true
        });
      }
      result.push(...items);
    }
  });

  return result;
}

 
export function deduplicateMenuItems(items: MenuItem[]): MenuItem[] {
  const seen = new Set<string>();
  const result: MenuItem[] = [];

  for (const item of items) {
    if (item.type === MenuItemType.SEPARATOR) {
      result.push(item);
      continue;
    }

    if (!seen.has(item.id)) {
      seen.add(item.id);
      result.push(item);
    }
  }

  return result;
}

 
export function cleanupSeparators(items: MenuItem[]): MenuItem[] {
  const result: MenuItem[] = [];
  let lastWasSeparator = true;

  for (const item of items) {
    const isSeparator = item.type === MenuItemType.SEPARATOR || item.separator;

    if (isSeparator) {
      if (!lastWasSeparator) {
        result.push(item);
        lastWasSeparator = true;
      }
    } else {
      result.push(item);
      lastWasSeparator = false;
    }
  }

  
  while (result.length > 0) {
    const last = result[result.length - 1];
    if (last.type === MenuItemType.SEPARATOR || last.separator) {
      result.pop();
    } else {
      break;
    }
  }

  return result;
}

 
export function sortMenuItemsByPriority(items: MenuItem[]): MenuItem[] {
  return [...items].sort((a, b) => {
    const priorityA = (a as any).priority || 0;
    const priorityB = (b as any).priority || 0;
    return priorityB - priorityA;
  });
}

