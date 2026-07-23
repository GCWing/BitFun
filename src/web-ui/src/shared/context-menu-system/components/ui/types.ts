/**
 * UI-level context menu types.
 *
 * These are lightweight types used by the renderer components.
 */
import { ReactNode } from 'react';

export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: string | ReactNode;
  disabled?: boolean;
  shortcut?: string;
  separator?: boolean;
  submenu?: ContextMenuItem[];
  onClick?: (context?: any) => void | Promise<void>;
}

export interface ContextMenuPosition {
  x: number;
  y: number;
}

export interface ContextMenuContext {
  selectedText?: string;
  element?: HTMLElement;
  flowItem?: any;
  dialogTurn?: any;
  [key: string]: any;
}

export interface ContextMenuProps {
  items: ContextMenuItem[];
  position: ContextMenuPosition;
  visible: boolean;
  context?: ContextMenuContext;
  onClose: () => void;
  onItemClick?: (item: ContextMenuItem, context?: ContextMenuContext) => void;
}

export interface ContextMenuProviderProps {
  children: React.ReactNode;
  menuItems?: ContextMenuItem[];
  getMenuItems?: (context: ContextMenuContext) => ContextMenuItem[];
  onMenuAction?: (action: string, context: ContextMenuContext) => void;
}
