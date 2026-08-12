import React, { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { DropPosition, EditorGroupId, SplitMode } from '../types';
import './DropZone.scss';

/** MIME type for dragging a chat session from the center pane. */
export const CHAT_SESSION_DRAG_MIME = 'application/x-bitfun-chat-session';

/** Payload carried by chat-session drags (kept in sync with ChatPane). */
export interface ExternalChatSessionPayload {
  sessionId: string;
  title: string;
}

export interface DropZoneProps {
  groupId: EditorGroupId;
  isDragging: boolean;
  draggingFromGroupId: EditorGroupId | null;
  splitMode: SplitMode;
  onDrop: (position: DropPosition) => void;
  /** Called when an external chat session is dropped onto this zone. */
  onExternalChatDrop?: (payload: ExternalChatSessionPayload) => void;
  children: React.ReactNode;
}

interface ZoneConfig {
  position: DropPosition;
  label: string;
  show: boolean;
}

export const DropZone: React.FC<DropZoneProps> = ({
  groupId,
  isDragging,
  draggingFromGroupId,
  splitMode,
  onDrop,
  onExternalChatDrop,
  children,
}) => {
  const { t } = useTranslation('components');
  const [activeZone, setActiveZone] = useState<DropPosition | null>(null);
  const [showOverlay, setShowOverlay] = useState(false);
  const [isExternalDragging, setIsExternalDragging] = useState(false);

  const isFromSameGroup = draggingFromGroupId === groupId;
  const isFromDifferentGroup = draggingFromGroupId !== null && !isFromSameGroup;

  const hasExternalChatPayload = useCallback((e: React.DragEvent): boolean => {
    return Array.from(e.dataTransfer.types).includes(CHAT_SESSION_DRAG_MIME);
  }, []);

  useEffect(() => {
    if (isDragging) {
      const timer = setTimeout(() => setShowOverlay(true), 100);
      return () => clearTimeout(timer);
    }
    setShowOverlay(false);
    setActiveZone(null);
  }, [isDragging]);

  // Reset external-drag state when the drag ends (drop or cancel).
  useEffect(() => {
    if (!isDragging && !isExternalDragging) {
      return;
    }
    const handleDragEndGlobal = () => {
      setIsExternalDragging(false);
      setShowOverlay(false);
      setActiveZone(null);
    };
    window.addEventListener('dragend', handleDragEndGlobal);
    return () => window.removeEventListener('dragend', handleDragEndGlobal);
  }, [isDragging, isExternalDragging]);

  const getVisibleZones = useCallback((): ZoneConfig[] => {
    // External chat-session drag: every cell is a valid target (center).
    if (isExternalDragging) {
      return [{ position: 'center', label: t('canvas.dropHere'), show: true }];
    }

    if (!isDragging) return [];

    if (splitMode === 'none') {
      return [
        { position: 'left', label: t('canvas.dropLeft'), show: true },
        { position: 'right', label: t('canvas.dropRight'), show: true },
        { position: 'bottom', label: t('canvas.dropBottom'), show: true },
      ];
    }

    if (splitMode === 'horizontal') {
      const zones: ZoneConfig[] = [
        { position: 'center', label: t('canvas.dropCenter'), show: isFromDifferentGroup },
        { position: 'bottom', label: t('canvas.dropBottom'), show: true },
      ];
      if (isFromSameGroup) {
        zones.push(
          groupId === 'primary'
            ? { position: 'right', label: t('canvas.dropRight'), show: true }
            : { position: 'left', label: t('canvas.dropLeft'), show: true }
        );
      }
      // Cross-group drag onto a horizontal (2-row) split: left/right edges grow
      // the 2 rows into the 3x3 grid by adding a column — "drag top/bottom
      // first, then drag left/right" works in any order.
      if (isFromDifferentGroup) {
        zones.push(
          { position: 'left', label: t('canvas.dropAddCol'), show: true },
          { position: 'right', label: t('canvas.dropAddCol'), show: true }
        );
      }
      return zones.filter(z => z.show);
    }

    if (splitMode === 'vertical') {
      const zones: ZoneConfig[] = [
        { position: 'center', label: t('canvas.dropCenter'), show: isFromDifferentGroup },
      ];
      if (isFromSameGroup) {
        zones.push(
          groupId === 'primary'
            ? { position: 'bottom', label: t('canvas.dropBottom'), show: true }
            : { position: 'left', label: t('canvas.dropLeft'), show: true }
        );
      }
      return zones.filter(z => z.show);
    }

    if (splitMode === 'grid') {
      const zones: ZoneConfig[] = [
        { position: 'center', label: t('canvas.dropCenter'), show: true },
        // Expanding the 3-pane (left/right/bottom) into the 3x3 grid: dropping
        // below the bottom pane activates the first slot of row 2 (grid9).
        { position: 'bottom', label: t('canvas.dropExpand'), show: groupId === 'tertiary' },
      ];
      return zones.filter(z => z.show);
    }

    if (splitMode === 'grid9') {
      // grid9 with independent rows/columns: every cell offers edge zones
      // (left/right = grow columns, top/bottom = grow rows) plus a center
      // placement. This lets the user build the grid in any order — rows
      // first, columns first, or interleaved — up to 3x3.
      return [
        { position: 'left',   label: t('canvas.dropAddCol'), show: true },
        { position: 'right',  label: t('canvas.dropAddCol'), show: true },
        { position: 'top',    label: t('canvas.dropAddRow'), show: true },
        { position: 'bottom', label: t('canvas.dropAddRow'), show: true },
        { position: 'center', label: t('canvas.dropToSlot'), show: true },
      ];
    }

    return [];
  }, [isDragging, splitMode, isFromSameGroup, isFromDifferentGroup, groupId, t, isExternalDragging]);

  const zones = getVisibleZones();

  const handleDragEnter = useCallback((position: DropPosition) => (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setActiveZone(position);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX;
    const y = e.clientY;
    if (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom) {
      setActiveZone(null);
    }
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    // Enter external-drag state when a chat session payload is being dragged.
    if (!isExternalDragging && hasExternalChatPayload(e)) {
      setIsExternalDragging(true);
    }
  }, [hasExternalChatPayload, isExternalDragging]);

  const handleDrop = useCallback((position: DropPosition) => (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setActiveZone(null);
    setShowOverlay(false);

    // External chat session drop → forward payload, no store tab-drag involved.
    if (onExternalChatDrop && hasExternalChatPayload(e)) {
      try {
        const raw = e.dataTransfer.getData(CHAT_SESSION_DRAG_MIME);
        const payload = JSON.parse(raw) as ExternalChatSessionPayload;
        if (payload?.sessionId) {
          setIsExternalDragging(false);
          onExternalChatDrop(payload);
          return;
        }
      } catch {
        // fall through to the internal drop path
      }
    }

    onDrop(position);
  }, [onDrop, onExternalChatDrop, hasExternalChatPayload]);

  const getZoneStyle = (position: DropPosition): React.CSSProperties => {
    const base: React.CSSProperties = { position: 'absolute' };
    
    if (zones.length === 1 && position === 'center') {
      return { ...base, inset: 0 };
    }

    const configs: Record<DropPosition, React.CSSProperties> = {
      left: { left: 0, top: 0, bottom: 0, width: '25%' },
      right: { right: 0, top: 0, bottom: 0, width: '25%' },
      top: { top: 0, left: 0, right: 0, height: '25%' },
      bottom: { bottom: 0, left: 0, right: 0, height: '25%' },
      center: { left: '25%', right: '25%', top: '25%', bottom: '25%' },
    };

    return { ...base, ...configs[position] };
  };

  return (
    <div
      data-bf-component="content-canvas"
      data-bf-part="dropZone"
      data-bf-state={(showOverlay || isExternalDragging) ? 'dragging' : ''}
      className={`canvas-drop-zone-container ${(showOverlay || isExternalDragging) ? 'is-dragging' : ''}`}
      onDragOver={(e) => {
        // Accept the L0 chat-session drag over the whole cell so dropping
        // anywhere in the panel works, even when the cell already has tabs
        // (the overlay zones may not be mounted then).
        if (hasExternalChatPayload(e)) {
          e.preventDefault();
          e.dataTransfer.dropEffect = 'copy';
          if (!isExternalDragging) setIsExternalDragging(true);
        }
      }}
      onDrop={(e) => {
        // Container-level external chat drop: forward payload when the zone
        // overlay is not rendered (cell already has tabs → handleExternalChatDrop
        // adds the tab; splitMode is untouched).
        if (onExternalChatDrop && hasExternalChatPayload(e)) {
          e.preventDefault();
          e.stopPropagation();
          setActiveZone(null);
          setShowOverlay(false);
          try {
            const raw = e.dataTransfer.getData(CHAT_SESSION_DRAG_MIME);
            const payload = JSON.parse(raw) as ExternalChatSessionPayload;
            if (payload?.sessionId) {
              setIsExternalDragging(false);
              onExternalChatDrop(payload);
              return;
            }
          } catch {
            // Malformed external payload (types said chat-session but the data
            // is not JSON): consume the drop so it does not silently vanish,
            // and drop into the internal store path with the raw position.
            setActiveZone(null);
            setShowOverlay(false);
            setIsExternalDragging(false);
            onDrop('center');
            return;
          }
        }
      }}
    >
      <div className="canvas-drop-zone-container__content" data-bf-component="content-canvas" data-bf-part="dropContent">
        {children}
      </div>

      {(showOverlay || isExternalDragging) && zones.length > 0 && (
        <div className="canvas-drop-zone-overlay" data-bf-component="content-canvas" data-bf-part="dropOverlay" data-bf-state="dragging">
          {zones.filter(z => z.show).map(({ position, label }) => (
            <div
              key={position}
              data-bf-component="content-canvas"
              data-bf-part="dropTarget"
              data-bf-position={position}
              data-bf-state={activeZone === position ? 'active' : ''}
              className={`canvas-drop-zone canvas-drop-zone--${position} ${activeZone === position ? 'is-active' : ''}`}
              style={getZoneStyle(position)}
              onDragEnter={handleDragEnter(position)}
              onDragLeave={handleDragLeave}
              onDragOver={handleDragOver}
              onDrop={handleDrop(position)}
            >
              <div className="canvas-drop-zone__indicator" data-bf-component="content-canvas" data-bf-part="dropIndicator" data-bf-position={position}>
                <span>{label}</span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

DropZone.displayName = 'DropZone';

export default DropZone;
