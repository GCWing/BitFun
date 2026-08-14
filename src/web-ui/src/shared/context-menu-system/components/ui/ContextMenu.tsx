 

import React, { useEffect, useLayoutEffect, useMemo, useRef, useCallback, useState } from 'react';
import { ContextMenuProps, ContextMenuItem } from './types';
import { createLogger } from '@/shared/utils/logger';
import './ContextMenu.scss';

const log = createLogger('ContextMenu');


const SUBMENU_OPEN_DELAY = 150;   
const SUBMENU_CLOSE_DELAY = 300;  
const SAFE_TRIANGLE_TOLERANCE = 50; 
const CONTEXT_MENU_EXIT_DURATION_MS = 100;

interface InternalContextMenuProps extends ContextMenuProps {
  autoFocusOnOpen?: boolean;
  onKeyboardBack?: () => void;
}

 
function isPointInTriangle(
  px: number, py: number,
  x1: number, y1: number,
  x2: number, y2: number,
  x3: number, y3: number
): boolean {
  const sign = (p1x: number, p1y: number, p2x: number, p2y: number, p3x: number, p3y: number) => {
    return (p1x - p3x) * (p2y - p3y) - (p2x - p3x) * (p1y - p3y);
  };

  const d1 = sign(px, py, x1, y1, x2, y2);
  const d2 = sign(px, py, x2, y2, x3, y3);
  const d3 = sign(px, py, x3, y3, x1, y1);

  const hasNeg = (d1 < 0) || (d2 < 0) || (d3 < 0);
  const hasPos = (d1 > 0) || (d2 > 0) || (d3 > 0);

  return !(hasNeg && hasPos);
}

export const ContextMenu: React.FC<InternalContextMenuProps> = ({
  items,
  position,
  visible,
  context,
  onClose,
  onItemClick,
  autoFocusOnOpen = true,
  onKeyboardBack,
}) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef(new Map<number, HTMLDivElement>());
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const wasVisibleRef = useRef(false);
  const exitTimerRef = useRef<number | null>(null);
  const enterFrameRef = useRef<number | null>(null);
  const submenuFocusFrameRef = useRef<number | null>(null);
  const [isPresent, setIsPresent] = useState(visible);
  const [motionPhase, setMotionPhase] = useState<'entering' | 'entered' | 'exiting'>(
    visible ? 'entering' : 'exiting',
  );
  
  
  const [activeSubmenuId, setActiveSubmenuId] = useState<string | null>(null);
  const submenuOpenTimerRef = useRef<number | null>(null);
  const submenuCloseTimerRef = useRef<number | null>(null);
  const focusableItemIndices = useMemo(
    () => items.reduce<number[]>((indices, item, index) => {
      if (!item.separator && !item.disabled) {
        indices.push(index);
      }
      return indices;
    }, []),
    [items],
  );
  const [focusedItemIndex, setFocusedItemIndex] = useState(
    () => focusableItemIndices[0] ?? -1,
  );
  
  
  const lastMousePosRef = useRef<{ x: number; y: number } | null>(null);
  const menuItemRectRef = useRef<DOMRect | null>(null);
  const submenuRectRef = useRef<DOMRect | null>(null);

  
  const clearAllTimers = useCallback(() => {
    if (submenuOpenTimerRef.current) {
      clearTimeout(submenuOpenTimerRef.current);
      submenuOpenTimerRef.current = null;
    }
    if (submenuCloseTimerRef.current) {
      clearTimeout(submenuCloseTimerRef.current);
      submenuCloseTimerRef.current = null;
    }
  }, []);

  const restorePreviousFocus = useCallback(() => {
    const previousFocus = previousFocusRef.current;
    if (previousFocus?.isConnected && !previousFocus.matches(':disabled')) {
      previousFocus.focus();
      return;
    }

    const activeElement = document.activeElement;
    if (activeElement instanceof window.HTMLElement && menuRef.current?.contains(activeElement)) {
      activeElement.blur();
    }
  }, []);

  const focusItemAtIndex = useCallback((index: number) => {
    setFocusedItemIndex(index);
    itemRefs.current.get(index)?.focus();
  }, []);

  useLayoutEffect(() => {
    if (visible && !wasVisibleRef.current) {
      previousFocusRef.current = document.activeElement instanceof window.HTMLElement
        ? document.activeElement
        : null;
      const firstItemIndex = focusableItemIndices[0] ?? -1;
      setFocusedItemIndex(firstItemIndex);

      if (autoFocusOnOpen) {
        if (firstItemIndex >= 0) {
          itemRefs.current.get(firstItemIndex)?.focus();
        } else {
          menuRef.current?.focus();
        }
      }
    } else if (!visible && wasVisibleRef.current) {
      const activeElement = document.activeElement;
      if (activeElement instanceof window.HTMLElement && menuRef.current?.contains(activeElement)) {
        restorePreviousFocus();
      }
    }

    wasVisibleRef.current = visible;
  }, [autoFocusOnOpen, focusableItemIndices, restorePreviousFocus, visible]);

  useEffect(() => {
    if (!focusableItemIndices.includes(focusedItemIndex)) {
      setFocusedItemIndex(focusableItemIndices[0] ?? -1);
    }
  }, [focusableItemIndices, focusedItemIndex]);

  useEffect(() => {
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
      exitTimerRef.current = null;
    }
    if (enterFrameRef.current !== null) {
      window.cancelAnimationFrame(enterFrameRef.current);
      enterFrameRef.current = null;
    }

    if (visible) {
      setIsPresent(true);
      setMotionPhase('entering');
      enterFrameRef.current = window.requestAnimationFrame(() => {
        enterFrameRef.current = window.requestAnimationFrame(() => {
          setMotionPhase('entered');
          enterFrameRef.current = null;
        });
      });
    } else {
      setMotionPhase('exiting');
      exitTimerRef.current = window.setTimeout(() => {
        setIsPresent(false);
        exitTimerRef.current = null;
      }, CONTEXT_MENU_EXIT_DURATION_MS);
    }

    return () => {
      if (exitTimerRef.current !== null) {
        window.clearTimeout(exitTimerRef.current);
        exitTimerRef.current = null;
      }
      if (enterFrameRef.current !== null) {
        window.cancelAnimationFrame(enterFrameRef.current);
        enterFrameRef.current = null;
      }
      if (submenuFocusFrameRef.current !== null) {
        window.cancelAnimationFrame(submenuFocusFrameRef.current);
        submenuFocusFrameRef.current = null;
      }
    };
  }, [visible]);

  
  const handleMenuItemMouseEnter = useCallback((
    item: ContextMenuItem,
    index: number,
    event: React.MouseEvent
  ) => {
    
    clearAllTimers();

    
    const target = event.currentTarget as HTMLElement;
    menuItemRectRef.current = target.getBoundingClientRect();

    const itemId = item.id || `item-${index}`;

    
    if (item.submenu && item.submenu.length > 0) {
      
      if (activeSubmenuId === itemId) {
        return;
      }

      
      if (activeSubmenuId) {
        setActiveSubmenuId(null);
      }

      
      submenuOpenTimerRef.current = window.setTimeout(() => {
        setActiveSubmenuId(itemId);
      }, SUBMENU_OPEN_DELAY);
    } else {
      
      if (activeSubmenuId) {
        setActiveSubmenuId(null);
      }
    }
  }, [activeSubmenuId, clearAllTimers]);

  
  const handleMenuItemMouseLeave = useCallback((
    item: ContextMenuItem,
    index: number,
    event: React.MouseEvent
  ) => {
    const itemId = item.id || `item-${index}`;
    
    
    if (activeSubmenuId !== itemId) {
      if (submenuOpenTimerRef.current) {
        clearTimeout(submenuOpenTimerRef.current);
        submenuOpenTimerRef.current = null;
      }
      return;
    }

    
    if (item.submenu && item.submenu.length > 0 && menuItemRectRef.current) {
      const mouseX = event.clientX;
      const mouseY = event.clientY;
      
      
      lastMousePosRef.current = { x: mouseX, y: mouseY };

      
      const submenuContainer = event.currentTarget.querySelector('.context-menu-submenu');
      if (submenuContainer) {
        submenuRectRef.current = submenuContainer.getBoundingClientRect();
      }

      
      submenuCloseTimerRef.current = window.setTimeout(() => {
        setActiveSubmenuId(null);
      }, SUBMENU_CLOSE_DELAY);
    }
  }, [activeSubmenuId]);

  
  const handleSubmenuMouseEnter = useCallback(() => {
    if (submenuCloseTimerRef.current) {
      clearTimeout(submenuCloseTimerRef.current);
      submenuCloseTimerRef.current = null;
    }
  }, []);

  
  const handleSubmenuMouseLeave = useCallback(() => {
    submenuCloseTimerRef.current = window.setTimeout(() => {
      setActiveSubmenuId(null);
    }, SUBMENU_CLOSE_DELAY);
  }, []);

  
  useEffect(() => {
    if (!activeSubmenuId || !visible) return;

    const handleMouseMove = (event: MouseEvent) => {
      const mouseX = event.clientX;
      const mouseY = event.clientY;

      
      if (lastMousePosRef.current && submenuRectRef.current && menuItemRectRef.current) {
        const itemRect = menuItemRectRef.current;
        const submenuRect = submenuRectRef.current;

        
        const isInSafeZone = isPointInTriangle(
          mouseX, mouseY,
          lastMousePosRef.current.x, lastMousePosRef.current.y,
          submenuRect.left, submenuRect.top - SAFE_TRIANGLE_TOLERANCE,
          submenuRect.left, submenuRect.bottom + SAFE_TRIANGLE_TOLERANCE
        );

        
        const isInMenuItem = (
          mouseX >= itemRect.left && mouseX <= itemRect.right &&
          mouseY >= itemRect.top && mouseY <= itemRect.bottom
        );
        const isInSubmenu = (
          mouseX >= submenuRect.left - 10 && mouseX <= submenuRect.right &&
          mouseY >= submenuRect.top && mouseY <= submenuRect.bottom
        );

        
        if (isInSafeZone || isInMenuItem || isInSubmenu) {
          if (submenuCloseTimerRef.current) {
            clearTimeout(submenuCloseTimerRef.current);
            submenuCloseTimerRef.current = null;
          }
        }
      }

      
      lastMousePosRef.current = { x: mouseX, y: mouseY };
    };

    document.addEventListener('mousemove', handleMouseMove);
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
    };
  }, [activeSubmenuId, visible]);

  
  const activateItem = useCallback(async (item: ContextMenuItem, restoreFocusAfter = false) => {
    if (item.disabled || item.separator) {
      return;
    }

    
    if (item.submenu && item.submenu.length > 0) {
      return;
    }

    
    if (item.onClick) {
      try {
        await Promise.resolve(item.onClick(context));
      } catch (error) {
        log.error('onClick handler failed', { itemId: item.id, error });
      }
    }

    if (onItemClick) {
      onItemClick(item, context);
    }

    if (restoreFocusAfter) {
      restorePreviousFocus();
    }
    onClose();
  }, [context, onItemClick, onClose, restorePreviousFocus]);

  const handleItemClick = useCallback((item: ContextMenuItem, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    void activateItem(item);
  }, [activateItem]);

  const openSubmenuFromKeyboard = useCallback((item: ContextMenuItem, index: number) => {
    if (!item.submenu?.length || item.disabled) {
      return;
    }

    clearAllTimers();
    setActiveSubmenuId(item.id || `item-${index}`);
    if (submenuFocusFrameRef.current !== null) {
      window.cancelAnimationFrame(submenuFocusFrameRef.current);
    }
    submenuFocusFrameRef.current = window.requestAnimationFrame(() => {
      const firstSubmenuItem = itemRefs.current.get(index)?.querySelector<HTMLElement>(
        '.context-menu-submenu [role="menu"] > [role="menuitem"]:not([aria-disabled="true"])',
      );
      firstSubmenuItem?.focus();
      submenuFocusFrameRef.current = null;
    });
  }, [clearAllTimers]);

  
  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    if (!visible) return;

    const activeElement = document.activeElement instanceof window.HTMLElement
      ? document.activeElement
      : null;
    const owningMenu = activeElement?.closest('[role="menu"]');
    if (owningMenu !== menuRef.current) {
      return;
    }

    const currentPosition = focusableItemIndices.indexOf(focusedItemIndex);
    const currentItem = focusedItemIndex >= 0 ? items[focusedItemIndex] : undefined;

    switch (event.key) {
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        restorePreviousFocus();
        onClose();
        break;
      case 'ArrowDown': {
        event.preventDefault();
        event.stopPropagation();
        if (focusableItemIndices.length > 0) {
          const nextPosition = currentPosition < 0
            ? 0
            : (currentPosition + 1) % focusableItemIndices.length;
          focusItemAtIndex(focusableItemIndices[nextPosition]);
        }
        break;
      }
      case 'ArrowUp': {
        event.preventDefault();
        event.stopPropagation();
        if (focusableItemIndices.length > 0) {
          const previousPosition = currentPosition <= 0
            ? focusableItemIndices.length - 1
            : currentPosition - 1;
          focusItemAtIndex(focusableItemIndices[previousPosition]);
        }
        break;
      }
      case 'Home':
        event.preventDefault();
        event.stopPropagation();
        if (focusableItemIndices.length > 0) {
          focusItemAtIndex(focusableItemIndices[0]);
        }
        break;
      case 'End':
        event.preventDefault();
        event.stopPropagation();
        if (focusableItemIndices.length > 0) {
          focusItemAtIndex(focusableItemIndices[focusableItemIndices.length - 1]);
        }
        break;
      case 'Enter':
      case ' ':
        event.preventDefault();
        event.stopPropagation();
        if (currentItem?.submenu?.length) {
          openSubmenuFromKeyboard(currentItem, focusedItemIndex);
        } else if (currentItem) {
          void activateItem(currentItem, true);
        }
        break;
      case 'ArrowRight':
        if (currentItem?.submenu?.length) {
          event.preventDefault();
          event.stopPropagation();
          openSubmenuFromKeyboard(currentItem, focusedItemIndex);
        }
        break;
      case 'ArrowLeft':
        if (onKeyboardBack) {
          event.preventDefault();
          event.stopPropagation();
          onKeyboardBack();
        }
        break;
    }
  }, [
    activateItem,
    focusItemAtIndex,
    focusedItemIndex,
    focusableItemIndices,
    items,
    onClose,
    onKeyboardBack,
    openSubmenuFromKeyboard,
    restorePreviousFocus,
    visible,
  ]);

  
  const handleClickOutside = useCallback((event: MouseEvent) => {
    if (!visible) return;
    
    
    
    const target = event.target as HTMLElement;
    const isMenuClick = target.closest('.context-menu') !== null;
    
    if (!isMenuClick) {
      onClose();
    }
  }, [visible, onClose]);

  
  const handlersRef = useRef<{
    keydown: (e: KeyboardEvent) => void;
    mousedown: (e: MouseEvent) => void;
    contextmenu: (e: MouseEvent) => void;
  } | null>(null);

  useEffect(() => {
    if (visible) {
      
      if (handlersRef.current) {
        document.removeEventListener('keydown', handlersRef.current.keydown, true);
        document.removeEventListener('mousedown', handlersRef.current.mousedown, true);
        document.removeEventListener('contextmenu', handlersRef.current.contextmenu, true);
      }
      
      
      handlersRef.current = {
        keydown: handleKeyDown,
        mousedown: handleClickOutside,
        contextmenu: handleClickOutside
      };
      
      
      document.addEventListener('keydown', handlersRef.current.keydown, true);
      document.addEventListener('mousedown', handlersRef.current.mousedown, true);
      document.addEventListener('contextmenu', handlersRef.current.contextmenu, true);

      return () => {
        if (handlersRef.current) {
          document.removeEventListener('keydown', handlersRef.current.keydown, true);
          document.removeEventListener('mousedown', handlersRef.current.mousedown, true);
          document.removeEventListener('contextmenu', handlersRef.current.contextmenu, true);
          handlersRef.current = null;
        }
      };
    } else {
      
      if (handlersRef.current) {
        document.removeEventListener('keydown', handlersRef.current.keydown, true);
        document.removeEventListener('mousedown', handlersRef.current.mousedown, true);
        document.removeEventListener('contextmenu', handlersRef.current.contextmenu, true);
        handlersRef.current = null;
      }
    }
  }, [visible, handleKeyDown, handleClickOutside]);

  
  useEffect(() => {
    if (!visible) {
      clearAllTimers();
      setActiveSubmenuId(null);
      lastMousePosRef.current = null;
      menuItemRectRef.current = null;
      submenuRectRef.current = null;
    }
    
    return () => {
      clearAllTimers();
    };
  }, [visible, clearAllTimers]);

  
  const adjustPosition = useCallback(() => {
    if (!menuRef.current || !isPresent) return;

    const menu = menuRef.current;
    const rect = menu.getBoundingClientRect();
    const viewport = {
      width: window.innerWidth,
      height: window.innerHeight
    };

    let { x, y } = position;

    
    if (x + rect.width > viewport.width) {
      x = viewport.width - rect.width - 8;
    }

    
    if (x < 8) {
      x = 8;
    }

    
    if (y + rect.height > viewport.height) {
      y = viewport.height - rect.height - 8;
    }

    
    if (y < 8) {
      y = 8;
    }

    menu.style.left = `${x}px`;
    menu.style.top = `${y}px`;
    const originX = Math.min(Math.max(position.x - x, 8), Math.max(rect.width - 8, 8));
    const originY = Math.min(Math.max(position.y - y, 8), Math.max(rect.height - 8, 8));
    menu.style.setProperty('--context-menu-transform-origin', `${originX}px ${originY}px`);
  }, [position, isPresent]);

  
  useEffect(() => {
    if (isPresent) {
      
      window.requestAnimationFrame(adjustPosition);
    }
  }, [isPresent, adjustPosition]);

  
  const renderMenuItem = (item: ContextMenuItem, index: number) => {
    if (item.separator) {
      return <div key={`separator-${index}`} className="context-menu-separator" data-bf-component="context-menu" data-bf-part="separator" role="separator" />;
    }

    const itemId = item.id || `item-${index}`;
    const hasSubmenu = item.submenu && item.submenu.length > 0;
    const isSubmenuActive = hasSubmenu && activeSubmenuId === itemId;

    return (
      <div
        ref={(element) => {
          if (element) {
            itemRefs.current.set(index, element);
          } else {
            itemRefs.current.delete(index);
          }
        }}
        key={itemId}
        className={`context-menu-item ${item.disabled ? 'disabled' : ''} ${isSubmenuActive ? 'submenu-active' : ''}`}
        onClick={(event) => handleItemClick(item, event)}
        onMouseEnter={(event) => handleMenuItemMouseEnter(item, index, event)}
        onMouseLeave={(event) => handleMenuItemMouseLeave(item, index, event)}
        onContextMenu={(event) => event.preventDefault()}
        data-bf-component="context-menu"
        data-bf-part="item"
        data-bf-state={[item.disabled && 'disabled', isSubmenuActive && 'submenu-active'].filter(Boolean).join(' ') || undefined}
        role="menuitem"
        aria-disabled={item.disabled || undefined}
        aria-haspopup={hasSubmenu ? 'menu' : undefined}
        aria-expanded={hasSubmenu ? isSubmenuActive : undefined}
        tabIndex={!item.disabled && index === focusedItemIndex ? 0 : -1}
        data-menu-index={index}
      >
        {item.icon && (
          <span className="context-menu-item-icon" data-bf-component="context-menu" data-bf-part="icon">
            {typeof item.icon === 'string' ? <i className={item.icon} /> : item.icon}
          </span>
        )}
        <span className="context-menu-item-label" data-bf-component="context-menu" data-bf-part="label">
          {item.label}
        </span>
        {item.shortcut && (
          <span className="context-menu-item-shortcut" data-bf-component="context-menu" data-bf-part="shortcut">
            {item.shortcut}
          </span>
        )}
        {hasSubmenu && (
          <>
            <span className="context-menu-submenu-arrow" data-bf-component="context-menu" data-bf-part="submenuArrow">▶</span>
            <div 
              className={`context-menu-submenu ${isSubmenuActive ? 'visible' : ''}`}
              onMouseEnter={handleSubmenuMouseEnter}
              onMouseLeave={handleSubmenuMouseLeave}
              data-bf-component="context-menu"
              data-bf-part="submenu"
            >
              <ContextMenu
                items={item.submenu!}
                position={{ x: 0, y: 0 }}
                visible={isSubmenuActive === true}
                context={context}
                onClose={onClose}
                onItemClick={onItemClick}
                autoFocusOnOpen={false}
                onKeyboardBack={() => {
                  setActiveSubmenuId(null);
                  focusItemAtIndex(index);
                }}
              />
            </div>
          </>
        )}
      </div>
    );
  };

  if (!isPresent) {
    return null;
  }

  return (
    <div
      ref={menuRef}
      className="context-menu"
      style={{
        left: position.x,
        top: position.y
      }}
      onContextMenu={(event) => event.preventDefault()}
      data-bf-component="context-menu"
      data-bf-part="root"
      data-motion="presence"
      data-state={motionPhase}
      role="menu"
      tabIndex={-1}
      aria-hidden={(!visible || motionPhase === 'exiting') || undefined}
      {...(!visible || motionPhase === 'exiting' ? { inert: '' } : {})}
    >
      {items.map(renderMenuItem)}
    </div>
  );
};

export default ContextMenu;
