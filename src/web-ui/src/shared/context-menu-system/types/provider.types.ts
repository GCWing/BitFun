/**
 * Context menu provider types.
 *
 * Providers supply menu items for a given `MenuContext`.
 */
import { MenuItem } from './menu.types';
import { MenuContext, ContextMatcher } from './context.types';

/**
 * Provider interface used by the menu manager.
 */
export interface IMenuProvider {
   
  readonly id: string;
   
  readonly name: string;
   
  readonly description?: string;
   
  readonly priority: number;
   
  readonly scope?: string | string[];

   
  matches(context: MenuContext): boolean;

   
  getMenuItems(context: MenuContext): MenuItem[] | Promise<MenuItem[]>;

   
  isEnabled?(): boolean;
}

 
export interface MenuProviderConfig {
   
  id: string;
   
  name: string;
   
  description?: string;
   
  priority?: number;
   
  scope?: string | string[];
   
  matcher: ContextMatcher;
   
  menuBuilder: (context: MenuContext) => MenuItem[] | Promise<MenuItem[]>;
   
  enabled?: boolean | (() => boolean);
}

 
export interface ProviderRegistrationOptions {
   
  override?: boolean;
   
  enabled?: boolean;
}

 
export interface ProviderMetadata {
   
  id: string;
   
  name: string;
   
  description?: string;
   
  priority: number;
   
  scope?: string | string[];
   
  enabled: boolean;
   
  registeredAt: number;
   
  invocationCount: number;
}

 
export interface ProviderGroup {
   
  id: string;
   
  name: string;
   
  providers: string[];
   
  priority?: number;
}

 
export enum MenuMergeStrategy {
   
  APPEND = 'append',
   
  PREPEND = 'prepend',
   
  PRIORITY = 'priority',
   
  GROUP = 'group',
   
  CUSTOM = 'custom'
}

 
export interface MenuMergeConfig {
   
  strategy: MenuMergeStrategy;
   
  deduplicate?: boolean;
   
  keepSeparators?: boolean;
   
  customMerge?: (items: MenuItem[][]) => MenuItem[];
}
