/**
 * Processing indicator.
 * After 1s of continuous processing, shows a 3×3 Rubik-style dot matrix and
 * rotating fun hint text together (matrix on the left).
 * reserveSpace keeps layout height even when hidden.
 */

import React, { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { DotMatrixLoader } from '@/component-library';
import './ProcessingIndicator.scss';

interface ProcessingIndicatorProps {
  visible: boolean;
  /** When true, preserve height to avoid layout jumps. */
  reserveSpace?: boolean;
}

export const ProcessingIndicator: React.FC<ProcessingIndicatorProps> = ({ visible, reserveSpace = false }) => {
  const { t } = useTranslation('flow-chat');
  const hints = t('processingHints', { returnObjects: true }) as string[];

  const [showHint, setShowHint] = useState(false);
  const [hintIndex, setHintIndex] = useState(0);

  const delayTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const rotateTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (visible) {
      const initialIndex = Math.floor(Math.random() * hints.length);
      setHintIndex(initialIndex);

      delayTimerRef.current = setTimeout(() => {
        setShowHint(true);
        rotateTimerRef.current = setInterval(() => {
          setHintIndex(prev => (prev + 1) % hints.length);
        }, 5000);
      }, 1000);
    } else {
      if (delayTimerRef.current) {
        clearTimeout(delayTimerRef.current);
        delayTimerRef.current = null;
      }
      if (rotateTimerRef.current) {
        clearInterval(rotateTimerRef.current);
        rotateTimerRef.current = null;
      }
      setShowHint(false);
    }

    return () => {
      if (delayTimerRef.current) clearTimeout(delayTimerRef.current);
      if (rotateTimerRef.current) clearInterval(rotateTimerRef.current);
    };
  }, [visible, hints.length]);

  const shouldRender = visible || reserveSpace;
  if (!shouldRender) return null;

  return (
    <div className="processing-indicator" aria-hidden={!visible}>
      <div
        className="processing-indicator__content"
        style={visible ? undefined : { visibility: 'hidden' as const }}
      >
        {showHint && hints.length > 0 && (
          <>
            <DotMatrixLoader size="medium" />
            <span key={hintIndex} className="processing-indicator__hint">
              {hints[hintIndex]}
            </span>
          </>
        )}
      </div>
    </div>
  );
};
