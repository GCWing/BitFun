 

import React, { useEffect, useRef, useCallback, useState } from 'react';
import { ContextMenuProps, ContextMenuItem } from './types';
import { createLogger } from '@/shared/utils/logger';
import './ContextMenu.scss';

const log = createLogger('ContextMenu');


const SUBMENU_OPEN_DELAY = 150;   
const SUBMENU_CLOSE_DELAY = 300;  
const SAFE_TRIANGLE_TOLERANCE = 50; 

 
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

export const ContextMenu: React.FC<ContextMenuProps> = ({
  items,
  position,
  visible,
  context,
  onClose,
  onItemClick
}) => {
  const menuRef = useRef<HTMLDivElement>(null);
  
  
  const [activeSubmenuId, setActiveSubmenuId] = useState<string | null>(null);
  const submenuOpenTimerRef = useRef<number | null>(null);
  const submenuCloseTimerRef = useRef<number | null>(null);
  
  
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

  
  const handleItemClick = useCallback(async (item: ContextMenuItem, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();

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

    
    onClose();
  }, [context, onItemClick, onClose]);

  
  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    if (!visible) return;

    switch (event.key) {
      case 'Escape':
        event.preventDefault();
        onClose();
        break;
      case 'ArrowDown':
        
        break;
      case 'ArrowUp':
        
        break;
      case 'Enter':
        
        break;
    }
  }, [visible, onClose]);

  
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
    if (!menuRef.current || !visible) return;

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
  }, [position, visible]);

  
  useEffect(() => {
    if (visible) {
      
      requestAnimationFrame(adjustPosition);
    }
  }, [visible, adjustPosition]);

  
  const renderMenuItem = (item: ContextMenuItem, index: number) => {
    if (item.separator) {
      return <div key={`separator-${index}`} className="context-menu-separator" />;
    }

    const itemId = item.id || `item-${index}`;
    const hasSubmenu = item.submenu && item.submenu.length > 0;
    const isSubmenuActive = hasSubmenu && activeSubmenuId === itemId;

    return (
      <div
        key={itemId}
        className={`context-menu-item ${item.disabled ? 'disabled' : ''} ${isSubmenuActive ? 'submenu-active' : ''}`}
        onClick={(event) => handleItemClick(item, event)}
        onMouseEnter={(event) => handleMenuItemMouseEnter(item, index, event)}
        onMouseLeave={(event) => handleMenuItemMouseLeave(item, index, event)}
        onContextMenu={(event) => event.preventDefault()}
      >
        {item.icon && (
          <span className="context-menu-item-icon">
            {typeof item.icon === 'string' ? <i className={item.icon} /> : item.icon}
          </span>
        )}
        <span className="context-menu-item-label">
          {item.label}
        </span>
        {item.shortcut && (
          <span className="context-menu-item-shortcut">
            {item.shortcut}
          </span>
        )}
        {hasSubmenu && (
          <>
            <span className="context-menu-submenu-arrow">▶</span>
            <div 
              className={`context-menu-submenu ${isSubmenuActive ? 'visible' : ''}`}
              onMouseEnter={handleSubmenuMouseEnter}
              onMouseLeave={handleSubmenuMouseLeave}
            >
              <ContextMenu
                items={item.submenu!}
                position={{ x: 0, y: 0 }}
                visible={isSubmenuActive === true}
                context={context}
                onClose={onClose}
                onItemClick={onItemClick}
              />
            </div>
          </>
        )}
      </div>
    );
  };

  if (!visible) {
    return null;
  }

  return (
    <div
      ref={menuRef}
      className={`context-menu ${visible ? 'visible' : ''}`}
      style={{
        left: position.x,
        top: position.y
      }}
      onContextMenu={(event) => event.preventDefault()}
    >
      {items.map(renderMenuItem)}
    </div>
  );
};

export default ContextMenu;
