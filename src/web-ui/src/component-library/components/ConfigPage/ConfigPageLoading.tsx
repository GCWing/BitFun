import React, { useEffect, useState } from 'react';
import './ConfigPage.scss';

/**
 * Grace period before a section admits it is loading.
 *
 * Config sections read their state through ConfigManager, which serves warm paths
 * from cache and cold ones over IPC. Painting the placeholder unconditionally means
 * a page full of 200px "loading" blocks appears and collapses within one or two
 * frames on the first open of a tab. Staying invisible for a beat lets fast reads
 * land silently; only genuinely slow ones surface a placeholder.
 */
const LOADING_VISIBLE_DELAY_MS = 200;

export interface ConfigPageLoadingProps {
  text: string;
  className?: string;
  /** Override the grace period; 0 paints immediately. */
  delayMs?: number;
}

export const ConfigPageLoading: React.FC<ConfigPageLoadingProps> = ({
  text,
  className = '',
  delayMs = LOADING_VISIBLE_DELAY_MS,
}) => {
  const [visible, setVisible] = useState(delayMs <= 0);

  useEffect(() => {
    if (delayMs <= 0) {
      setVisible(true);
      return;
    }

    setVisible(false);
    const timer = window.setTimeout(() => setVisible(true), delayMs);
    return () => window.clearTimeout(timer);
  }, [delayMs]);

  if (!visible) return null;

  return (
    <div className={`bitfun-config-page-loading ${className}`} data-bf-component="config-page" data-bf-part="loading">
      {text}
    </div>
  );
};
