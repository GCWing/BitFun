/**
 * Modal component
 */

import React, { useEffect, useState, useRef, useCallback, useId } from 'react';
import { createPortal } from 'react-dom';
import { useI18n } from '@/infrastructure/i18n';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { isImeOwnedKeyboardEvent } from '@/shared/utils/ime';
import { PopupCloseButton } from '../PopupCloseButton';
import './Modal.scss';

// Keep in sync with modal-overlay-exit/modal-dialog-exit in Modal.scss.
const MODAL_EXIT_DURATION_MS = 180;

/**
 * Ref-counted `body` scroll lock, shared by every Modal instance.
 *
 * Modals stack — a dialog opening a confirmation on top of itself is routine —
 * and each one clearing `overflow` on its own would unlock the page while the
 * others are still open. The 180 ms exit delay widens that window: the inner
 * modal's cleanup lands well after it has visually gone.
 */
let bodyScrollLockCount = 0;

function lockBodyScroll(): () => void {
  bodyScrollLockCount += 1;
  document.body.style.overflow = 'hidden';
  let released = false;
  return () => {
    if (released) {
      return;
    }
    released = true;
    bodyScrollLockCount = Math.max(0, bodyScrollLockCount - 1);
    if (bodyScrollLockCount === 0) {
      document.body.style.overflow = '';
    }
  };
}

export interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  titleExtra?: React.ReactNode;
  children: React.ReactNode;
  size?: 'small' | 'medium' | 'large' | 'xlarge' | 'xxlarge';
  contentInset?: boolean;
  /** Extra class on `.modal__content` (e.g. flex layout for scroll regions inside children) */
  contentClassName?: string;
  showCloseButton?: boolean;
  /** When false, clicks on the backdrop do not call onClose. Default true. */
  closeOnOverlayClick?: boolean;
  /** Extra class on `.modal-overlay` for dialog-specific stacking and layout hooks. */
  overlayClassName?: string;
  draggable?: boolean;
  resizable?: boolean;
  placement?: 'center' | 'bottom-left' | 'bottom-right';
  testId?: string;
  titleTestId?: string;
  closeButtonTestId?: string;
  ariaLabel?: string;
  ariaLabelledBy?: string;
}

export const Modal: React.FC<ModalProps> = ({
  isOpen,
  onClose,
  title,
  titleExtra,
  children,
  size = 'medium',
  contentInset = false,
  contentClassName,
  showCloseButton = true,
  closeOnOverlayClick = true,
  overlayClassName,
  draggable = false,
  resizable = false,
  placement = 'center',
  testId,
  titleTestId,
  closeButtonTestId,
  ariaLabel,
  ariaLabelledBy,
}) => {
  const { t } = useI18n('components');
  const [isPresent, setIsPresent] = useState(isOpen);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const [dimensions, setDimensions] = useState<{ width: number; height: number } | null>(null);
  const [isResizing, setIsResizing] = useState(false);
  const [resizeDirection, setResizeDirection] = useState<string>('');
  const [resizeStart, setResizeStart] = useState({ x: 0, y: 0, width: 0, height: 0 });
  const modalRef = useRef<HTMLDivElement>(null);
  const headerRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const generatedTitleId = useId();
  const isExiting = !isOpen && isPresent;

  useEffect(() => {
    if (isOpen) {
      setIsPresent(true);
      return;
    }

    if (!isPresent) {
      return;
    }

    const exitTimer = window.setTimeout(() => {
      setIsPresent(false);
    }, MODAL_EXIT_DURATION_MS);

    return () => window.clearTimeout(exitTimer);
  }, [isOpen, isPresent]);

  useEffect(() => {
    if (!isOpen && !isPresent) {
      return;
    }

    return lockBodyScroll();
  }, [isOpen, isPresent]);

  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen && !isImeOwnedKeyboardEvent(e)) {
        onClose();
      }
    };

    document.addEventListener('keydown', handleEscape);
    return () => document.removeEventListener('keydown', handleEscape);
  }, [isOpen, onClose]);

  useEffect(() => {
    if (!isOpen) return;

    previousFocusRef.current = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const modal = modalRef.current;
    const focusableSelector = [
      'button:not([disabled])',
      '[href]',
      'input:not([disabled])',
      'select:not([disabled])',
      'textarea:not([disabled])',
      '[tabindex]:not([tabindex="-1"])',
    ].join(',');
    const focusable = modal?.querySelector<HTMLElement>(focusableSelector);
    (focusable ?? modal)?.focus();

    const trapFocus = (event: KeyboardEvent) => {
      if (event.key !== 'Tab' || !modal) return;
      const elements = Array.from(modal.querySelectorAll<HTMLElement>(focusableSelector));
      if (elements.length === 0) {
        event.preventDefault();
        modal.focus();
        return;
      }
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener('keydown', trapFocus);

    return () => {
      document.removeEventListener('keydown', trapFocus);
      previousFocusRef.current?.focus();
      previousFocusRef.current = null;
    };
  }, [isOpen]);

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
    } else if (!isOpen && !isPresent) {
      setPosition(null);
      setDimensions(null);
    }
  }, [isOpen, isPresent, draggable, resizable]);

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

  if (!isOpen && !isPresent) return null;

  const appliedStyle = (draggable || resizable) && position ? {
    position: 'fixed' as const,
    top: position.y,
    left: position.x,
    transform: 'none',
    margin: 0,
    ...(dimensions && resizable ? { width: dimensions.width, height: dimensions.height } : {})
  } : {};
  const dialogAppearanceState = [
    draggable && 'draggable',
    isDragging && 'dragging',
    resizable && 'resizable',
    isResizing && 'resizing',
  ].filter(Boolean).join(' ');

  return createPortal(
    <div
      className={[
        'modal-overlay',
        placement !== 'center' ? `modal-overlay--${placement}` : '',
        isExiting ? 'modal-overlay--exiting' : '',
        overlayClassName ?? '',
      ]
        .filter(Boolean)
        .join(' ')}
      onClick={closeOnOverlayClick ? onClose : undefined}
      data-bf-component="modal"
      data-bf-part="overlay"
      data-bf-placement={placement}
    >
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-hidden={isExiting || undefined}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy ?? (title ? generatedTitleId : undefined)}
        tabIndex={-1}
        className={[
          'modal',
          `modal--${size}`,
          draggable ? 'modal--draggable' : '',
          isDragging ? 'modal--dragging' : '',
          resizable ? 'modal--resizable' : '',
          isResizing ? 'modal--resizing' : '',
          isExiting ? 'modal--exiting' : '',
          contentInset ? 'modal--content-inset' : '',
          showCloseButton ? 'modal--with-close' : '',
        ]
          .filter(Boolean)
          .join(' ')}
        onClick={(e) => e.stopPropagation()}
        onMouseDown={handleMouseDown}
        style={appliedStyle}
        data-testid={testId}
        data-bf-component="modal"
        data-bf-part="dialog"
        data-bf-size={size}
        data-bf-placement={placement}
        data-bf-state={dialogAppearanceState || undefined}
      >
        {(title || showCloseButton) && (
          <div
            className={[
              'modal__header-shell',
              !title && showCloseButton && !draggable ? 'modal__header-shell--close-only' : '',
              title || draggable ? 'modal__header-shell--with-header' : '',
            ]
              .filter(Boolean)
              .join(' ')}
            data-bf-component="modal"
            data-bf-part="headerShell"
          >
            {(title || (draggable && showCloseButton)) && (
              <div
                ref={headerRef}
                className={[
                  'modal__header',
                  draggable ? 'modal__header--draggable' : '',
                  !title && showCloseButton ? 'modal__header--empty' : '',
                ]
                  .filter(Boolean)
                  .join(' ')}
                data-bf-component="modal"
                data-bf-part="header"
                data-bf-state={draggable ? 'draggable' : undefined}
              >
                {title && (
                  <div className="modal__title-group" data-bf-component="modal" data-bf-part="titleGroup">
                    <h2 id={generatedTitleId} className="modal__title" data-testid={titleTestId} data-bf-component="modal" data-bf-part="title">{title}</h2>
                    {titleExtra && <span className="modal__title-extra" data-bf-component="modal" data-bf-part="titleExtra">{titleExtra}</span>}
                  </div>
                )}
              </div>
            )}
            {showCloseButton && (
              <PopupCloseButton
                className="modal__close"
                onClick={onClose}
                aria-label={t('modal.close')}
                data-testid={closeButtonTestId}
                data-bf-component="modal"
                data-bf-part="close"
              />
            )}
          </div>
        )}
        
        <div
          className={[
            'modal__content',
            contentInset ? 'modal__content--inset' : '',
            contentClassName ?? '',
          ]
            .filter(Boolean)
            .join(' ')}
          data-bf-component="modal"
          data-bf-part="content"
          data-bf-state={contentInset ? 'contentInset' : undefined}
        >
          {children}
        </div>
        
        {resizable && (
          <>
            {(['n', 's', 'w', 'e', 'nw', 'ne', 'sw', 'se'] as const).map(direction => (
              <div
                key={direction}
                className={`modal__resize-handle modal__resize-handle--${direction}`}
                onMouseDown={(e) => handleResizeStart(e, direction)}
                data-bf-component="modal"
                data-bf-part="resizeHandle"
                data-bf-resize-direction={direction}
                data-bf-state={isResizing ? 'resizing' : 'resizable'}
              />
            ))}
          </>
        )}
      </div>
    </div>,
    getAppearanceOverlayHost()
  );
};
