/**
 * SplitHandle component.
 * Divider for adjusting split ratio.
 */

import React, { useState, useCallback, useRef, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import { LAYOUT_CONFIG, clampSplitRatio } from '../types';
import './SplitHandle.scss';

export interface SplitHandleProps {
  /** Split direction */
  direction: 'horizontal' | 'vertical';
  /** Current ratio */
  ratio: number;
  /** Ratio change callback */
  onRatioChange: (ratio: number) => void;
  /** Container ref */
  containerRef: React.RefObject<HTMLElement>;
}

export const SplitHandle: React.FC<SplitHandleProps> = ({
  direction,
  ratio,
  onRatioChange,
  containerRef,
}) => {
  const { t } = useTranslation('components');
  const [isDragging, setIsDragging] = useState(false);
  const startPosRef = useRef(0);
  const startRatioRef = useRef(ratio);

  // Handle mouse down
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
    startPosRef.current = direction === 'horizontal' ? e.clientX : e.clientY;
    startRatioRef.current = ratio;
  }, [direction, ratio]);

  // Handle mouse move
  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      if (!containerRef.current) return;

      const containerRect = containerRef.current.getBoundingClientRect();
      const containerSize = direction === 'horizontal' 
        ? containerRect.width 
        : containerRect.height;
      
      const currentPos = direction === 'horizontal' ? e.clientX : e.clientY;
      const delta = currentPos - startPosRef.current;
      const deltaRatio = delta / containerSize;
      
      const newRatio = clampSplitRatio(startRatioRef.current + deltaRatio);
      onRatioChange(newRatio);
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, direction, containerRef, onRatioChange]);

  // Double-click to reset
  const handleDoubleClick = useCallback(() => {
    onRatioChange(LAYOUT_CONFIG.DEFAULT_SPLIT_RATIO);
  }, [onRatioChange]);

  // Handle keyboard adjustments
  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    const step = e.shiftKey ? 0.1 : 0.02;
    
    if (direction === 'horizontal') {
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        onRatioChange(clampSplitRatio(ratio - step));
      } else if (e.key === 'ArrowRight') {
        e.preventDefault();
        onRatioChange(clampSplitRatio(ratio + step));
      }
    } else {
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        onRatioChange(clampSplitRatio(ratio - step));
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        onRatioChange(clampSplitRatio(ratio + step));
      }
    }
  }, [direction, ratio, onRatioChange]);

  return (
    <Tooltip content={t('canvas.dragToResize')}>
      <div
        className={`canvas-split-handle canvas-split-handle--${direction} ${
          isDragging ? 'is-dragging' : ''
        }`}
        onMouseDown={handleMouseDown}
        onDoubleClick={handleDoubleClick}
        onKeyDown={handleKeyDown}
        tabIndex={0}
        role="separator"
        aria-orientation={direction}
        aria-valuenow={Math.round(ratio * 100)}
        aria-valuemin={LAYOUT_CONFIG.MIN_SPLIT_RATIO * 100}
        aria-valuemax={LAYOUT_CONFIG.MAX_SPLIT_RATIO * 100}
      >
        <div className="canvas-split-handle__line" />
        <div className="canvas-split-handle__grip">
          {direction === 'horizontal' ? (
            <svg width="6" height="16" viewBox="0 0 6 16" fill="none">
              <circle cx="3" cy="4" r="1" fill="currentColor" />
              <circle cx="3" cy="8" r="1" fill="currentColor" />
              <circle cx="3" cy="12" r="1" fill="currentColor" />
            </svg>
          ) : (
            <svg width="16" height="6" viewBox="0 0 16 6" fill="none">
              <circle cx="4" cy="3" r="1" fill="currentColor" />
              <circle cx="8" cy="3" r="1" fill="currentColor" />
              <circle cx="12" cy="3" r="1" fill="currentColor" />
            </svg>
          )}
        </div>
      </div>
    </Tooltip>
  );
};

SplitHandle.displayName = 'SplitHandle';

export default SplitHandle;
