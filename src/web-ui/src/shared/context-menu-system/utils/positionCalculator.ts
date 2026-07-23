 

import { MenuPosition } from '../types/menu.types';

 
export function calculateMenuPosition(
  mouseX: number,
  mouseY: number,
  menuWidth: number,
  menuHeight: number,
  padding: number = 10
): MenuPosition {
  const viewportWidth = window.innerWidth;
  const viewportHeight = window.innerHeight;

  let x = mouseX;
  let y = mouseY;

  
  if (x + menuWidth > viewportWidth - padding) {
    x = viewportWidth - menuWidth - padding;
  }
  x = Math.max(padding, x);

  
  if (y + menuHeight > viewportHeight - padding) {
    y = viewportHeight - menuHeight - padding;
  }
  y = Math.max(padding, y);

  return { x, y };
}

 
export function calculateSubmenuPosition(
  parentRect: DOMRect,
  submenuWidth: number,
  submenuHeight: number
): MenuPosition {
  const viewportWidth = window.innerWidth;
  const viewportHeight = window.innerHeight;

  
  let x = parentRect.right;
  let y = parentRect.top;

  
  if (x + submenuWidth > viewportWidth - 10) {
    x = parentRect.left - submenuWidth;
  }

  
  if (x < 10) {
    x = 10;
  }

  
  if (y + submenuHeight > viewportHeight - 10) {
    y = viewportHeight - submenuHeight - 10;
  }
  y = Math.max(10, y);

  return { x, y };
}

 
export function isPointInRect(x: number, y: number, rect: DOMRect): boolean {
  return x >= rect.left && 
         x <= rect.right && 
         y >= rect.top && 
         y <= rect.bottom;
}

 
export function getElementViewportRect(element: HTMLElement): DOMRect {
  return element.getBoundingClientRect();
}

