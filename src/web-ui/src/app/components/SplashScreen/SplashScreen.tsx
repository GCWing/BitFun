/**
 * SplashScreen — full-screen loading overlay shown on app start.
 *
 * Idle:    logo larger, soft fade in/out.
 * Exiting: logo scales up and fades; backdrop dissolves.
 */

import React, { useEffect, useCallback, useState } from 'react';
import './SplashScreen.scss';

const DEFAULT_LOADING_MESSAGE_DELAY_MS = 1800;

interface SplashScreenProps {
  isExiting: boolean;
  onExited: () => void;
  delayedMessage?: string;
  delayedMessageMs?: number;
}

const SplashScreen: React.FC<SplashScreenProps> = ({
  isExiting,
  onExited,
  delayedMessage,
  delayedMessageMs = DEFAULT_LOADING_MESSAGE_DELAY_MS,
}) => {
  const [showDelayedMessage, setShowDelayedMessage] = useState(false);
  const handleExited = useCallback(() => {
    onExited();
  }, [onExited]);

  useEffect(() => {
    setShowDelayedMessage(false);

    if (!delayedMessage || isExiting) {
      return;
    }

    const timer = window.setTimeout(() => {
      setShowDelayedMessage(true);
    }, delayedMessageMs);
    return () => window.clearTimeout(timer);
  }, [delayedMessage, delayedMessageMs, isExiting]);

  // Remove from DOM after exit animation completes (~650 ms).
  useEffect(() => {
    if (!isExiting) return;
    const timer = window.setTimeout(handleExited, 650);
    return () => window.clearTimeout(timer);
  }, [isExiting, handleExited]);

  return (
    <div
      className={`splash-screen${isExiting ? ' splash-screen--exiting' : ''}`}
      aria-hidden={!showDelayedMessage}
    >
      <div className="splash-screen__center">
        <div className="splash-screen__logo-wrap">
          <img
            src="/Logo-ICON-128.png"
            alt="BitFun"
            className="splash-screen__logo"
            draggable={false}
            decoding="async"
          />
        </div>
        {showDelayedMessage && delayedMessage && !isExiting && (
          <div
            className="splash-screen__message splash-screen__message--visible"
            role="status"
            aria-live="polite"
          >
            {delayedMessage}
          </div>
        )}
      </div>
    </div>
  );
};

export default SplashScreen;
