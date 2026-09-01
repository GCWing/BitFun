import React from 'react';
import { Menu, MenuItem, MenuPopover, MenuSeparator, type MenuEntry, type MenuItemProps, type MenuProps, type MenuPopoverParts, type MenuSeparatorProps } from '@bitfun/ui';
import { createLogger } from '@/shared/utils/logger';
import type { ContextMenuProps, ContextMenuItem } from './types';

const log = createLogger('ContextMenu');

// Keep persisted appearance part IDs while the public components retain DOM ownership.
const menuParts: MenuPopoverParts = {
  root: React.forwardRef<HTMLDivElement, MenuProps>((props, ref) => <Menu {...props} ref={ref} data-bf-product-component="context-menu" data-bf-product-part="root" />),
  item: React.forwardRef<HTMLButtonElement, MenuItemProps>((props, ref) => <MenuItem {...props} ref={ref} data-bf-product-component="context-menu" data-bf-product-part="item" data-bf-state={[props.disabled && 'disabled', props['aria-expanded'] === true && 'submenu-active'].filter(Boolean).join(' ') || undefined} />),
  separator: React.forwardRef<HTMLDivElement, MenuSeparatorProps>((props, ref) => <MenuSeparator {...props} ref={ref} data-bf-product-component="context-menu" data-bf-product-part="separator" />),
  icon: props => <span {...props} data-bf-product-component="context-menu" data-bf-product-part="icon" />,
  label: props => <span {...props} data-bf-product-component="context-menu" data-bf-product-part="label" />,
  shortcut: props => <span {...props} data-bf-product-component="context-menu" data-bf-product-part="shortcut" />,
  submenuArrow: props => <span {...props} data-bf-product-component="context-menu" data-bf-product-part="submenuArrow" />,
  submenu: props => <div {...props} data-bf-product-component="context-menu" data-bf-product-part="submenu" />,
};

/** Product adapter: portable menus own UI; callbacks retain their product context. */
export const ContextMenu: React.FC<ContextMenuProps> = ({ items, position, visible, context, onClose, onItemClick }) => {
  const convert = (item: ContextMenuItem): MenuEntry => ({
    id: item.id,
    label: item.label,
    disabled: item.disabled,
    separator: item.separator,
    icon: typeof item.icon === 'string' ? <i className={item.icon} /> : item.icon,
    shortcut: item.shortcut,
    submenu: item.submenu?.map(convert),
    onSelect: () => {
      void (async () => {
        try {
          await item.onClick?.(context);
        } catch (error) {
          log.error('onClick handler failed', { itemId: item.id, error });
        }
        onItemClick?.(item, context);
      })();
    },
  });

  return <MenuPopover items={items.map(convert)} position={position} open={visible} onClose={onClose} parts={menuParts} />;
};

export default ContextMenu;
