import React, { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import type { DropPosition, EditorGroupId } from '../types';
import './DropZone.scss';

export interface DropZoneProps {
  groupId: EditorGroupId;
  isDragging: boolean;
  draggingFromGroupId: EditorGroupId | null;
  splitMode: 'none' | 'horizontal' | 'vertical' | 'grid';
  onDrop: (position: DropPosition) => void;
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
  children,
}) => {
  const { t } = useTranslation('components');
  const [activeZone, setActiveZone] = useState<DropPosition | null>(null);
  const [showOverlay, setShowOverlay] = useState(false);

  const isFromSameGroup = draggingFromGroupId === groupId;
  const isFromDifferentGroup = draggingFromGroupId !== null && !isFromSameGroup;

  useEffect(() => {
    if (isDragging) {
      const timer = setTimeout(() => setShowOverlay(true), 100);
      return () => clearTimeout(timer);
    }
    setShowOverlay(false);
    setActiveZone(null);
  }, [isDragging]);

  const getVisibleZones = useCallback((): ZoneConfig[] => {
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
      return [{ position: 'center', label: t('canvas.dropCenter'), show: true }];
    }

    return [];
  }, [isDragging, splitMode, isFromSameGroup, isFromDifferentGroup, groupId, t]);

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
  }, []);

  const handleDrop = useCallback((position: DropPosition) => (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setActiveZone(null);
    setShowOverlay(false);
    onDrop(position);
  }, [onDrop]);

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
    <div className={`canvas-drop-zone-container ${showOverlay ? 'is-dragging' : ''}`}>
      <div className="canvas-drop-zone-container__content">
        {children}
      </div>

      {showOverlay && zones.length > 0 && (
        <div className="canvas-drop-zone-overlay">
          {zones.filter(z => z.show).map(({ position, label }) => (
            <div
              key={position}
              className={`canvas-drop-zone canvas-drop-zone--${position} ${activeZone === position ? 'is-active' : ''}`}
              style={getZoneStyle(position)}
              onDragEnter={handleDragEnter(position)}
              onDragLeave={handleDragLeave}
              onDragOver={handleDragOver}
              onDrop={handleDrop(position)}
            >
              <div className="canvas-drop-zone__indicator">
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
