/**
 * SplashScreen — full-screen loading overlay shown on app start.
 *
 * Idle:    logo larger, soft fade in/out.
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
      <div className="splash-screen__center">
        <div className="splash-screen__logo-wrap">
          <img
            src="/logo-dark-transparent.png"
            alt="BitFun"
            className="splash-screen__logo splash-screen__logo--dark"
            draggable={false}
          />
          <img
            src="/logo-light-transparent.png"
            alt=""
            className="splash-screen__logo splash-screen__logo--light"
            draggable={false}
          />
        </div>
      </div>
    </div>
  );
};

export default SplashScreen;
