import React from 'react';
import { Icon, type IconName } from '@bitfun/ui';
import {
  Archive,
  ArchiveRestore,
  Clipboard,
  Code,
  FileInput,
  FileOutput,
  FilePlus,
  FileText,
  FileType,
  FolderOpen,
  FolderPlus,
  Highlighter,
  Lightbulb,
  List,
  MessageSquarePlus,
  Navigation,
  PanelRightOpen,
  Scissors,
  type LucideIcon,
} from 'lucide-react';

const CONTEXT_MENU_CATALOG: Record<string, IconName> = {
  Copy: 'duplicate',
  Download: 'arrow-down',
  Trash2: 'delete',
  ChevronLeft: 'chevron-left',
  Edit: 'edit',
  ExternalLink: 'arrow-up-right',
  MessageSquare: 'side-chat',
  Pin: 'pin',
  Plus: 'plus',
  Search: 'search',
  RefreshCw: 'refresh',
  X: 'xmark',
};
import { ContextMenu } from './ui/ContextMenu';
import { useContextMenuStore } from '../store/ContextMenuStore';
import { MenuItem as SystemMenuItem } from '../types/menu.types';
import { ContextMenuItem as UIMenuItem } from './ui/types';

const CONTEXT_MENU_ICONS = {
  Archive,
  ArchiveRestore,
  Clipboard,
  Code,
  FileInput,
  FileOutput,
  FilePlus,
  FileText,
  FileType,
  FolderOpen,
  FolderPlus,
  Highlighter,
  Lightbulb,
  List,
  MessageSquarePlus,
  Navigation,
  PanelRightOpen,
  Scissors,
} satisfies Record<string, LucideIcon>;

function getIconComponent(icon: any): string | React.ReactNode | undefined {
  if (!icon) return undefined;

  if (React.isValidElement(icon)) {
    return icon;
  }

  if (typeof icon === 'string') {
    const catalogName = CONTEXT_MENU_CATALOG[icon];
    if (catalogName) {
      return React.createElement(Icon, { name: catalogName, size: 'md' });
    }

    const IconComponent = CONTEXT_MENU_ICONS[icon as keyof typeof CONTEXT_MENU_ICONS];
    if (IconComponent) {
      return React.createElement(IconComponent, { size: 16 });
    }

    return icon;
  }

  return undefined;
}

function convertMenuItem(item: SystemMenuItem): UIMenuItem {
  return {
    id: item.id,
    label: item.label,
    icon: getIconComponent(item.icon),
    disabled: typeof item.disabled === 'boolean' ? item.disabled : false,
    separator: item.separator,
    shortcut: item.shortcut,
    submenu: item.submenu?.map(convertMenuItem),
    onClick: item.onClick
  };
}

export const ContextMenuRenderer: React.FC = () => {
  const { visible, position, items, context, hideMenu } = useContextMenuStore();

  const uiItems = items.map(convertMenuItem);

  return (
    <ContextMenu
      items={uiItems}
      position={position || { x: 0, y: 0 }}
      visible={visible}
      context={context || undefined}
      onClose={hideMenu}
    />
  );
};

export default ContextMenuRenderer;
