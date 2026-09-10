import type { MouseEvent } from 'react';
import { useContextMenuStore } from '@/shared/context-menu-system/store/ContextMenuStore';
import { ContextType } from '@/shared/context-menu-system/types/context.types';
import type { MenuItem } from '@/shared/context-menu-system/types/menu.types';

export function showResourceMenu(event: MouseEvent<HTMLElement>, items: MenuItem[], atPointer = false) {
  event.preventDefault();
  event.stopPropagation();
  const target = event.currentTarget;
  const rect = target.getBoundingClientRect();
  const position = atPointer ? { x: event.clientX, y: event.clientY } : { x: rect.left, y: rect.bottom };
  useContextMenuStore.getState().showMenu(position, items, {
    type: ContextType.CUSTOM, customType: 'workspace-resources', data: {},
    event, targetElement: target, position, timestamp: Date.now(),
  });
}
