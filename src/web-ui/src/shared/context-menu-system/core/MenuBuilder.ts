 

import { MenuItem, MenuItemType } from '../types/menu.types';
import { MenuContext } from '../types/context.types';
import { IMenuProvider, MenuMergeStrategy, MenuMergeConfig } from '../types/provider.types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('MenuBuilder');

 
export class MenuBuilder {
  private mergeConfig: MenuMergeConfig;

  constructor(mergeConfig?: Partial<MenuMergeConfig>) {
    this.mergeConfig = {
      strategy: MenuMergeStrategy.PRIORITY,
      deduplicate: true,
      keepSeparators: true,
      ...mergeConfig
    };
  }

   
  async build(providers: IMenuProvider[], context: MenuContext): Promise<MenuItem[]> {
    
    const itemCollections = await Promise.all(
      providers.map(async (provider) => {
        try {
          const items = await provider.getMenuItems(context);
          return { provider, items };
        } catch (error) {
          log.error('Failed to get menu items from provider', { providerId: provider.id, error });
          return { provider, items: [] };
        }
      })
    );

    
    const processedCollections = itemCollections.map(({ provider, items }) => ({
      provider,
      items: this.processItems(items, context)
    }));

    
    const allItems = processedCollections.map(c => c.items);

    
    const mergedItems = this.mergeItems(allItems);

    
    const finalItems = this.postProcess(mergedItems);

    return finalItems;
  }

   
  private processItems(items: MenuItem[], context: MenuContext): MenuItem[] {
    return items
      .map(item => this.processItem(item, context))
      .filter((item): item is MenuItem => item !== null);
  }

   
  private processItem(item: MenuItem, context: MenuContext): MenuItem | null {
    
    const visible = typeof item.visible === 'function' 
      ? item.visible(context) 
      : item.visible ?? true;

    if (!visible) {
      return null;
    }

    
    const disabled = typeof item.disabled === 'function'
      ? item.disabled(context)
      : item.disabled ?? false;

    
    const checked = typeof item.checked === 'function'
      ? item.checked(context)
      : item.checked;

    
    const submenu = item.submenu 
      ? this.processItems(item.submenu, context)
      : undefined;

    return {
      ...item,
      disabled,
      checked,
      submenu
    };
  }

   
  private mergeItems(itemCollections: MenuItem[][]): MenuItem[] {
    if (itemCollections.length === 0) {
      return [];
    }

    if (itemCollections.length === 1) {
      return itemCollections[0];
    }

    
    switch (this.mergeConfig.strategy) {
      case MenuMergeStrategy.APPEND:
        return this.mergeAppend(itemCollections);
      
      case MenuMergeStrategy.PREPEND:
        return this.mergePrepend(itemCollections);
      
      case MenuMergeStrategy.PRIORITY:
        return this.mergePriority(itemCollections);
      
      case MenuMergeStrategy.GROUP:
        return this.mergeByGroup(itemCollections);
      
      case MenuMergeStrategy.CUSTOM:
        return this.mergeConfig.customMerge
          ? this.mergeConfig.customMerge(itemCollections)
          : this.mergeAppend(itemCollections);
      
      default:
        return this.mergeAppend(itemCollections);
    }
  }

   
  private mergeAppend(collections: MenuItem[][]): MenuItem[] {
    const result: MenuItem[] = [];
    
    collections.forEach((items, index) => {
      if (items.length > 0) {
        if (result.length > 0 && this.mergeConfig.keepSeparators) {
          
          result.push(this.createSeparator(`separator-${index}`));
        }
        result.push(...items);
      }
    });

    return result;
  }

   
  private mergePrepend(collections: MenuItem[][]): MenuItem[] {
    return this.mergeAppend([...collections].reverse());
  }

   
  private mergePriority(collections: MenuItem[][]): MenuItem[] {
    return this.mergeAppend(collections);
  }

   
  private mergeByGroup(collections: MenuItem[][]): MenuItem[] {
    const groups = new Map<string, MenuItem[]>();
    const ungrouped: MenuItem[] = [];

    
    collections.forEach(items => {
      items.forEach(item => {
        if (item.group) {
          const groupItems = groups.get(item.group) || [];
          groupItems.push(item);
          groups.set(item.group, groupItems);
        } else {
          ungrouped.push(item);
        }
      });
    });

    
    const result: MenuItem[] = [];

    
    groups.forEach((items, groupName) => {
      if (result.length > 0 && this.mergeConfig.keepSeparators) {
        result.push(this.createSeparator(`group-${groupName}-sep`));
      }
      result.push(...items);
    });

    
    if (ungrouped.length > 0) {
      if (result.length > 0 && this.mergeConfig.keepSeparators) {
        result.push(this.createSeparator('ungrouped-sep'));
      }
      result.push(...ungrouped);
    }

    return result;
  }

   
  private postProcess(items: MenuItem[]): MenuItem[] {
    let result = [...items];

    
    if (this.mergeConfig.deduplicate) {
      result = this.deduplicateItems(result);
    }

    
    result = this.cleanupSeparators(result);

    return result;
  }

   
  private deduplicateItems(items: MenuItem[]): MenuItem[] {
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

   
  private cleanupSeparators(items: MenuItem[]): MenuItem[] {
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

   
  private createSeparator(id: string): MenuItem {
    return {
      id,
      label: '',
      type: MenuItemType.SEPARATOR,
      separator: true
    };
  }

   
  setMergeConfig(config: Partial<MenuMergeConfig>): void {
    this.mergeConfig = { ...this.mergeConfig, ...config };
  }

   
  getMergeConfig(): MenuMergeConfig {
    return { ...this.mergeConfig };
  }
}

 
export const menuBuilder = new MenuBuilder();
