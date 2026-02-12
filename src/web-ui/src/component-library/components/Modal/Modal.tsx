/**
 * Modal component
 */

import React, { useEffect, useState, useRef, useCallback } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import './Modal.scss';

export interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  children: React.ReactNode;
  size?: 'small' | 'medium' | 'large';
  showCloseButton?: boolean;
  draggable?: boolean;
  resizable?: boolean;
  className?: string;
  overlayClassName?: string;
}

export const Modal: React.FC<ModalProps> = ({
  isOpen,
  onClose,
  title,
  children,
  size = 'medium',
  showCloseButton = true,
  draggable = false,
  resizable = false,
  className = '',
  overlayClassName = '',
}) => {
  const { t } = useI18n('components');
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const [dimensions, setDimensions] = useState<{ width: number; height: number } | null>(null);
  const [isResizing, setIsResizing] = useState(false);
  const [resizeDirection, setResizeDirection] = useState<string>('');
  const [resizeStart, setResizeStart] = useState({ x: 0, y: 0, width: 0, height: 0 });
  const modalRef = useRef<HTMLDivElement>(null);
  const headerRef = useRef<HTMLDivElement>(null);
  
  useEffect(() => {
    if (isOpen) {
      document.body.style.overflow = 'hidden';
    } else {
      document.body.style.overflow = '';
    }

    return () => {
      document.body.style.overflow = '';
    };
  }, [isOpen]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    };

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, onClose]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!draggable || !modalRef.current || !headerRef.current) return;
    
    if (!headerRef.current.contains(e.target as Node)) return;
    
    if ((e.target as Element).closest('.modal__close')) return;

    setIsDragging(true);
    const rect = modalRef.current.getBoundingClientRect();
    setDragOffset({
      x: e.clientX - rect.left,
      y: e.clientY - rect.top
    });
    
    e.preventDefault();
  }, [draggable]);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isDragging || !draggable) return;
    
    const newX = e.clientX - dragOffset.x;
    const newY = e.clientY - dragOffset.y;
    
    const maxX = window.innerWidth - (modalRef.current?.offsetWidth || 0);
    const maxY = window.innerHeight - (modalRef.current?.offsetHeight || 0);
    
    setPosition({
      x: Math.max(0, Math.min(newX, maxX)),
      y: Math.max(0, Math.min(newY, maxY))
    });
  }, [isDragging, draggable, dragOffset]);

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  useEffect(() => {
    if (isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.body.style.userSelect = 'none';
      
      return () => {
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
        document.body.style.userSelect = '';
      };
    }
  }, [isDragging, handleMouseMove, handleMouseUp]);

  useEffect(() => {
    if (isOpen && (draggable || resizable) && modalRef.current) {
      const modalWidth = modalRef.current.offsetWidth;
      const modalHeight = modalRef.current.offsetHeight;
      const centerX = (window.innerWidth - modalWidth) / 2;
      const centerY = (window.innerHeight - modalHeight) / 2;
      
      setPosition({ 
        x: Math.max(0, centerX), 
        y: Math.max(0, centerY) 
      });
      
      if (resizable) {
        setDimensions({
          width: modalWidth,
          height: modalHeight
        });
      }
    } else if (!isOpen) {
      setPosition(null);
      setDimensions(null);
    }
  }, [isOpen, draggable, resizable]);

  const handleResizeStart = useCallback((e: React.MouseEvent, direction: string) => {
    if (!resizable || !modalRef.current) return;
    
    e.preventDefault();
    e.stopPropagation();
    
    setIsResizing(true);
    setResizeDirection(direction);
    
    const rect = modalRef.current.getBoundingClientRect();
    setResizeStart({
      x: e.clientX,
      y: e.clientY,
      width: rect.width,
      height: rect.height
    });
  }, [resizable]);

  const handleResizeMove = useCallback((e: MouseEvent) => {
    if (!isResizing || !resizable || !modalRef.current || !position) return;
    
    const deltaX = e.clientX - resizeStart.x;
    const deltaY = e.clientY - resizeStart.y;
    
    let newWidth = resizeStart.width;
    let newHeight = resizeStart.height;
    let newX = position.x;
    let newY = position.y;
    
    const minWidth = 300;
    const minHeight = 200;
    
    if (resizeDirection.includes('e')) {
      newWidth = Math.max(minWidth, resizeStart.width + deltaX);
    }
    if (resizeDirection.includes('w')) {
      const proposedWidth = Math.max(minWidth, resizeStart.width - deltaX);
      const widthDiff = resizeStart.width - proposedWidth;
      newWidth = proposedWidth;
      newX = position.x + widthDiff;
    }
    if (resizeDirection.includes('s')) {
      newHeight = Math.max(minHeight, resizeStart.height + deltaY);
    }
    if (resizeDirection.includes('n')) {
      const proposedHeight = Math.max(minHeight, resizeStart.height - deltaY);
      const heightDiff = resizeStart.height - proposedHeight;
      newHeight = proposedHeight;
      newY = position.y + heightDiff;
    }
    
    if (newX < 0) {
      newWidth += newX;
      newX = 0;
    }
    if (newY < 0) {
      newHeight += newY;
      newY = 0;
    }
    if (newX + newWidth > window.innerWidth) {
      newWidth = window.innerWidth - newX;
    }
    if (newY + newHeight > window.innerHeight) {
      newHeight = window.innerHeight - newY;
    }
    
    setDimensions({ width: newWidth, height: newHeight });
    setPosition({ x: newX, y: newY });
  }, [isResizing, resizable, resizeDirection, resizeStart, position]);

  const handleResizeEnd = useCallback(() => {
    setIsResizing(false);
    setResizeDirection('');
  }, []);

  useEffect(() => {
    if (isResizing) {
      document.addEventListener('mousemove', handleResizeMove);
      document.addEventListener('mouseup', handleResizeEnd);
      document.body.style.userSelect = 'none';
      
      return () => {
        document.removeEventListener('mousemove', handleResizeMove);
        document.removeEventListener('mouseup', handleResizeEnd);
        document.body.style.userSelect = '';
      };
    }
  }, [isResizing, handleResizeMove, handleResizeEnd]);

  if (!isOpen) return null;

  const appliedStyle = (draggable || resizable) && position ? {
    position: 'fixed' as const,
    top: position.y,
    left: position.x,
    transform: 'none',
    margin: 0,
    ...(dimensions && resizable ? { width: dimensions.width, height: dimensions.height } : {})
  } : {};

  return (
    <div className={`modal-overlay ${overlayClassName}`.trim()} onClick={onClose}>
      <div
        ref={modalRef}
        className={`modal modal--${size} ${draggable ? 'modal--draggable' : ''} ${isDragging ? 'modal--dragging' : ''} ${resizable ? 'modal--resizable' : ''} ${isResizing ? 'modal--resizing' : ''} ${className}`.trim()}
        onClick={(e) => e.stopPropagation()}
        onMouseDown={handleMouseDown}
        style={appliedStyle}
      >
        {(title || showCloseButton) && (
          <div 
            ref={headerRef}
            className={`modal__header ${draggable ? 'modal__header--draggable' : ''}`}
          >
            {title && <h2 className="modal__title">{title}</h2>}
            {showCloseButton && (
              <button
                className="modal__close"
                onClick={onClose}
                aria-label={t('modal.close')}
                type="button"
              >
                <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                  <line x1="3" y1="3" x2="11" y2="11" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
                  <line x1="11" y1="3" x2="3" y2="11" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
                </svg>
              </button>
            )}
          </div>
        )}
        
        <div className="modal__content">
          {children}
        </div>
        
        {resizable && (
          <>
            <div className="modal__resize-handle modal__resize-handle--n" onMouseDown={(e) => handleResizeStart(e, 'n')} />
            <div className="modal__resize-handle modal__resize-handle--s" onMouseDown={(e) => handleResizeStart(e, 's')} />
            <div className="modal__resize-handle modal__resize-handle--w" onMouseDown={(e) => handleResizeStart(e, 'w')} />
            <div className="modal__resize-handle modal__resize-handle--e" onMouseDown={(e) => handleResizeStart(e, 'e')} />
            <div className="modal__resize-handle modal__resize-handle--nw" onMouseDown={(e) => handleResizeStart(e, 'nw')} />
            <div className="modal__resize-handle modal__resize-handle--ne" onMouseDown={(e) => handleResizeStart(e, 'ne')} />
            <div className="modal__resize-handle modal__resize-handle--sw" onMouseDown={(e) => handleResizeStart(e, 'sw')} />
            <div className="modal__resize-handle modal__resize-handle--se" onMouseDown={(e) => handleResizeStart(e, 'se')} />
          </>
        )}
      </div>
    </div>
  );
};
