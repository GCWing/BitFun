/**
 * Context menu item and configuration types.
 */
import { ReactNode } from 'react';
import { MenuContext } from './context.types';

 
export type MenuItemId = string;

 
export enum MenuItemType {
   
  NORMAL = 'normal',
   
  SEPARATOR = 'separator',
   
  SUBMENU = 'submenu',
   
  CHECKBOX = 'checkbox',
   
  RADIO = 'radio',
   
  CUSTOM = 'custom'
}

/**
 * A single menu item, optionally dynamic based on the current context.
 */
export interface MenuItem {
   
  id: MenuItemId;
   
  label: string;
   
  type?: MenuItemType;
   
  separator?: boolean;
   
  icon?: ReactNode | string;
   
  shortcut?: string;
   
  disabled?: boolean | ((context: MenuContext) => boolean);
   
  visible?: boolean | ((context: MenuContext) => boolean);
   
  checked?: boolean | ((context: MenuContext) => boolean);
   
  submenu?: MenuItem[];
   
  group?: string;
   
  command?: string;
   
  onClick?: (context: MenuContext) => void | Promise<void>;
   
  render?: (item: MenuItem, context: MenuContext) => ReactNode;
   
  className?: string;
   
  style?: React.CSSProperties;
   
  tooltip?: string;
   
  data?: any;
}

 
export type MenuItemBuilder = (context: MenuContext) => MenuItem | MenuItem[] | null;

 
export interface MenuPosition {
  x: number;
  y: number;
}

 
export interface MenuConfig {
   
  items: MenuItem[];
   
  position: MenuPosition;
   
  context: MenuContext;
   
  visible: boolean;
   
  maxHeight?: number;
   
  maxWidth?: number;
  /** z-index */
  zIndex?: number;
   
  animation?: {
    enter?: string;
    exit?: string;
  };
}

 
export interface MenuGroup {
   
  id: string;
   
  label?: string;
   
  items: MenuItem[];
   
  priority?: number;
}

 
export interface MenuTheme {
   
  background?: string;
   
  color?: string;
   
  hoverBackground?: string;
   
  disabledColor?: string;
   
  borderColor?: string;
   
  boxShadow?: string;
   
  borderRadius?: string;
}

 
export interface MenuOptions {
   
  theme?: MenuTheme;
   
  showIcons?: boolean;
   
  showShortcuts?: boolean;
   
  enableSearch?: boolean;
   
  searchPlaceholder?: string;
   
  minWidth?: number;
   
  autoAdjustPosition?: boolean;
   
  closeDelay?: number;
   
  submenuOpenDelay?: number;
}

 
export interface MenuEvents {
   
  onBeforeShow?: (context: MenuContext) => void | Promise<void>;
   
  onAfterShow?: (context: MenuContext) => void;
   
  onBeforeHide?: () => void | Promise<void>;
   
  onAfterHide?: () => void;
   
  onItemClick?: (item: MenuItem, context: MenuContext) => void;
   
  onItemHover?: (item: MenuItem, context: MenuContext) => void;
}

 
export interface MenuState {
   
  visible: boolean;
   
  position: MenuPosition | null;
   
  items: MenuItem[];
   
  context: MenuContext | null;
   
  activeSubmenuId: string | null;
   
  focusedIndex: number;
   
  searchQuery: string;
}
