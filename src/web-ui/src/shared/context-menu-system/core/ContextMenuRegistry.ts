 

import {
  IMenuProvider,
  MenuProviderConfig,
  ProviderRegistrationOptions,
  ProviderMetadata,
  ProviderGroup
} from '../types/provider.types';
import { MenuContext } from '../types/context.types';
import { MenuItem } from '../types/menu.types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ContextMenuRegistry');

 
class SimpleMenuProvider implements IMenuProvider {
  readonly id: string;
  readonly name: string;
  readonly description?: string;
  readonly priority: number;
  readonly scope?: string | string[];
  
  private matcher: (context: MenuContext) => boolean;
  private menuBuilder: (context: MenuContext) => MenuItem[] | Promise<MenuItem[]>;
  private enabled: boolean | (() => boolean);

  constructor(config: MenuProviderConfig) {
    this.id = config.id;
    this.name = config.name;
    this.description = config.description;
    this.priority = config.priority || 0;
    this.scope = config.scope;
    this.matcher = config.matcher;
    this.menuBuilder = config.menuBuilder;
    this.enabled = config.enabled ?? true;
  }

  matches(context: MenuContext): boolean {
    return this.matcher(context);
  }

  async getMenuItems(context: MenuContext): Promise<MenuItem[]> {
    const items = await this.menuBuilder(context);
    return items;
  }

  isEnabled(): boolean {
    return typeof this.enabled === 'function' ? this.enabled() : this.enabled;
  }
}

 
export class ContextMenuRegistry {
  private providers: Map<string, IMenuProvider>;
  private metadata: Map<string, ProviderMetadata>;
  private groups: Map<string, ProviderGroup>;

  constructor() {
    this.providers = new Map();
    this.metadata = new Map();
    this.groups = new Map();
  }

   
  register(
    provider: IMenuProvider | MenuProviderConfig,
    options: ProviderRegistrationOptions = {}
  ): void {
    const actualProvider = this.isConfig(provider)
      ? new SimpleMenuProvider(provider)
      : provider;

    
    if (this.providers.has(actualProvider.id) && !options.override) {
      throw new Error(`Menu provider with id "${actualProvider.id}" already exists`);
    }

    
    this.providers.set(actualProvider.id, actualProvider);

    
    this.metadata.set(actualProvider.id, {
      id: actualProvider.id,
      name: actualProvider.name,
      description: actualProvider.description,
      priority: actualProvider.priority,
      scope: actualProvider.scope,
      enabled: options.enabled ?? true,
      registeredAt: Date.now(),
      invocationCount: 0
    });
  }

   
  unregister(providerId: string): boolean {
    const deleted = this.providers.delete(providerId);
    this.metadata.delete(providerId);
    
    
    this.groups.forEach(group => {
      const index = group.providers.indexOf(providerId);
      if (index > -1) {
        group.providers.splice(index, 1);
      }
    });

    return deleted;
  }

   
  getProvider(providerId: string): IMenuProvider | undefined {
    return this.providers.get(providerId);
  }

   
  getAllProviders(): IMenuProvider[] {
    return Array.from(this.providers.values());
  }

   
  getMetadata(providerId: string): ProviderMetadata | undefined {
    return this.metadata.get(providerId);
  }

   
  findMatchingProviders(context: MenuContext): IMenuProvider[] {
    const matching: IMenuProvider[] = [];

    for (const provider of this.providers.values()) {
      try {
        
        if (provider.isEnabled && !provider.isEnabled()) {
          continue;
        }

        const meta = this.metadata.get(provider.id);
        if (meta && !meta.enabled) {
          continue;
        }

        
        if (provider.scope && !this.matchesScope(provider.scope, context)) {
          continue;
        }

        
        const matches = provider.matches(context);
        
        if (matches) {
          matching.push(provider);
          
          
          if (meta) {
            meta.invocationCount++;
          }
        }
      } catch (error) {
        log.error('Error matching provider', { providerId: provider.id, error });
      }
    }

    
    matching.sort((a, b) => b.priority - a.priority);

    return matching;
  }

   
  setProviderEnabled(providerId: string, enabled: boolean): void {
    const meta = this.metadata.get(providerId);
    if (meta) {
      meta.enabled = enabled;
    }
  }

   
  createGroup(group: ProviderGroup): void {
    this.groups.set(group.id, group);
  }

   
  getGroup(groupId: string): ProviderGroup | undefined {
    return this.groups.get(groupId);
  }

   
  addToGroup(groupId: string, providerId: string): void {
    const group = this.groups.get(groupId);
    if (group && !group.providers.includes(providerId)) {
      group.providers.push(providerId);
    }
  }

   
  removeFromGroup(groupId: string, providerId: string): void {
    const group = this.groups.get(groupId);
    if (group) {
      const index = group.providers.indexOf(providerId);
      if (index > -1) {
        group.providers.splice(index, 1);
      }
    }
  }

   
  getProvidersInGroup(groupId: string): IMenuProvider[] {
    const group = this.groups.get(groupId);
    if (!group) {
      return [];
    }

    return group.providers
      .map(id => this.providers.get(id))
      .filter(Boolean) as IMenuProvider[];
  }

   
  clear(): void {
    this.providers.clear();
    this.metadata.clear();
    this.groups.clear();
  }

   
  getStats() {
    return {
      totalProviders: this.providers.size,
      enabledProviders: Array.from(this.metadata.values()).filter(m => m.enabled).length,
      totalGroups: this.groups.size,
      metadata: Array.from(this.metadata.values())
    };
  }

   
  private isConfig(obj: any): obj is MenuProviderConfig {
    return 'matcher' in obj && 'menuBuilder' in obj;
  }

   
  private matchesScope(scope: string | string[], context: MenuContext): boolean {
    const scopes = Array.isArray(scope) ? scope : [scope];
    
    
    const contextArea = context.metadata?.area;
    if (contextArea) {
      return scopes.includes(contextArea);
    }

    return true; 
  }
}

 
export const contextMenuRegistry = new ContextMenuRegistry();

