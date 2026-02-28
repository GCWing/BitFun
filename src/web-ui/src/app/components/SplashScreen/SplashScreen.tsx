/**
 * SplashScreen — full-screen loading overlay shown on app start.
 *
 * Idle:    logo breathes softly; loading dots bounce.
 * Exiting: logo scales up and fades; backdrop dissolves.
 */

import React, { useEffect, useCallback } from 'react';
import './SplashScreen.scss';

interface SplashScreenProps {
  isExiting: boolean;
  onExited: () => void;
}

const SplashScreen: React.FC<SplashScreenProps> = ({ isExiting, onExited }) => {
  const handleExited = useCallback(() => {
    onExited();
  }, [onExited]);

  // Remove from DOM after exit animation completes (~650 ms).
  useEffect(() => {
    if (!isExiting) return;
    const timer = window.setTimeout(handleExited, 650);
    return () => window.clearTimeout(timer);
  }, [isExiting, handleExited]);

  return (
    <div
      className={`splash-screen${isExiting ? ' splash-screen--exiting' : ''}`}
      aria-hidden="true"
    >
      {/* Center: logo + loading dots */}
      <div className="splash-screen__center">
        <div className="splash-screen__logo-wrap">
          <img
            src="/Logo-ICON.png"
            alt="BitFun"
            className="splash-screen__logo"
            draggable={false}
          />
        </div>

        <div className="splash-screen__dots" aria-hidden="true">
          <span className="splash-screen__dot" />
          <span className="splash-screen__dot" />
          <span className="splash-screen__dot" />
        </div>
      </div>
    </div>
  );
};

export default SplashScreen;
