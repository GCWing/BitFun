/**
 * Processing indicator.
 * After 1s of continuous processing, shows the logo frame animation and
 * rotating fun hint text together (animation on the left).
 * reserveSpace keeps layout height even when hidden.
 */

import React, { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { processingHintsZh, processingHintsEn } from '../../constants/processingHints';
import { LogoFrameAnimation } from './LogoFrameAnimation';
import './ProcessingIndicator.scss';

interface ProcessingIndicatorProps {
  visible: boolean;
  /** When true, preserve height to avoid layout jumps. */
  reserveSpace?: boolean;
}

export const ProcessingIndicator: React.FC<ProcessingIndicatorProps> = ({ visible, reserveSpace = false }) => {
  const { i18n } = useTranslation();
  const hints = i18n.language.startsWith('zh') ? processingHintsZh : processingHintsEn;

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
            <LogoFrameAnimation size={30} className="processing-indicator__logo" />
            <span key={hintIndex} className="processing-indicator__hint">
              {hints[hintIndex]}
            </span>
          </>
        )}
      </div>
    </div>
  );
};
