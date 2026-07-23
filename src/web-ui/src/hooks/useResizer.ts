import { useState, useCallback, useRef, useEffect, RefObject } from 'react';

export type ResizerDirection = 'horizontal' | 'vertical';

interface UseResizerOptions {
  direction: ResizerDirection;
  currentRatio: number;
  onRatioChange: (ratio: number) => void;
  containerRef: RefObject<HTMLElement | null>;
  minRatio?: number;
  maxRatio?: number;
  resetRatio?: number;
}

interface UseResizerReturn {
  handleMouseDown: (e: React.MouseEvent) => void;
  handleDoubleClick: () => void;
  isDragging: boolean;
}

export const useResizer = ({
  direction,
  currentRatio,
  onRatioChange,
  containerRef,
  minRatio = 0.2,
  maxRatio = 0.8,
  resetRatio = 0.5,
}: UseResizerOptions): UseResizerReturn => {
  const [isDragging, setIsDragging] = useState(false);
  const animationFrameRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    if (!containerRef.current) return;

    const isHorizontal = direction === 'horizontal';
    const startPos = isHorizontal ? e.clientX : e.clientY;
    const containerSize = isHorizontal 
      ? containerRef.current.offsetWidth 
      : containerRef.current.offsetHeight;
    const startRatio = currentRatio;

    setIsDragging(true);
    document.body.style.cursor = isHorizontal ? 'col-resize' : 'row-resize';
    document.body.style.userSelect = 'none';

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }

      animationFrameRef.current = requestAnimationFrame(() => {
        const currentPos = isHorizontal ? moveEvent.clientX : moveEvent.clientY;
        const deltaPos = currentPos - startPos;
        const deltaRatio = deltaPos / containerSize;
        const newRatio = Math.min(maxRatio, Math.max(minRatio, startRatio + deltaRatio));
        onRatioChange(newRatio);
        animationFrameRef.current = null;
      });
    };

    const handleMouseUp = () => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      
      setIsDragging(false);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }, [direction, currentRatio, onRatioChange, containerRef, minRatio, maxRatio]);

  const handleDoubleClick = useCallback(() => {
    onRatioChange(resetRatio);
  }, [onRatioChange, resetRatio]);

  return {
    handleMouseDown,
    handleDoubleClick,
    isDragging,
  };
};

export default useResizer;
