import React, { useEffect, useState } from 'react';
import { LoadingState } from '@bitfun/ui';
import './ConfigPageState.scss';

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

export interface ConfigLoadingStateProps {
  label: string;
  className?: string;
  /** Override the grace period; 0 paints immediately. */
  graceMs?: number;
}

export const ConfigLoadingState: React.FC<ConfigLoadingStateProps> = ({
  label,
  className = '',
  graceMs = LOADING_VISIBLE_DELAY_MS,
}) => {
  const [visible, setVisible] = useState(graceMs <= 0);

  useEffect(() => {
    if (graceMs <= 0) {
      setVisible(true);
      return;
    }

    setVisible(false);
    const timer = window.setTimeout(() => setVisible(true), graceMs);
    return () => window.clearTimeout(timer);
  }, [graceMs]);

  if (!visible) return null;

  return (
    <div
      className={['bitfun-config-loading-state', className].filter(Boolean).join(' ')}
      data-bf-component="config"
      data-bf-part="loadingState"
    >
      <LoadingState size="sm">{label}</LoadingState>
    </div>
  );
};
