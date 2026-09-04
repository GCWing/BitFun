/**
 * Token usage indicator.
 * Shows session token usage as percentage and count.
 */

import React, { useMemo } from 'react';
import { useI18n } from '@/infrastructure/i18n';
import './TokenUsageIndicator.scss';

export interface TokenUsageIndicatorProps {
  currentTokens: number;    // Current token usage
  maxTokens: number;        // Max token capacity
  className?: string;
}

export const TokenUsageIndicator: React.FC<TokenUsageIndicatorProps> = ({
  currentTokens,
  maxTokens,
  className = ''
}) => {
  const { formatNumber } = useI18n();
  const percentage = useMemo(() => {
    if (!maxTokens || maxTokens <= 0) return 0;
    return Math.min(Math.round((currentTokens / maxTokens) * 100), 100);
  }, [currentTokens, maxTokens]);

  const getStatusClass = (percent: number): string => {
    if (percent >= 90) return 'critical';
    if (percent >= 70) return 'warning';
    return 'normal';
  };

  const statusClass = getStatusClass(percentage);

  return (
    <div
      data-openbitfun-component="token-usage-indicator"
      data-openbitfun-part="root"
      data-openbitfun-level={statusClass}
      className={`openbitfun-token-usage ${className} openbitfun-token-usage--${statusClass}`}
      title={`Token usage: ${formatNumber(currentTokens)} / ${formatNumber(maxTokens)}`}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={percentage}
    >
      <div data-openbitfun-component="token-usage-indicator" data-openbitfun-part="track" className="openbitfun-token-usage__progress-track">
        <div 
          data-openbitfun-component="token-usage-indicator"
          data-openbitfun-part="fill"
          className="openbitfun-token-usage__progress-fill"
          style={{ '--token-usage-progress': percentage / 100 } as React.CSSProperties}
        />
      </div>
      
      <div data-openbitfun-component="token-usage-indicator" data-openbitfun-part="hover" className="openbitfun-token-usage__hover-content">
        <span data-openbitfun-component="token-usage-indicator" data-openbitfun-part="percentage" className="openbitfun-token-usage__percentage">{percentage}%</span>
      </div>
    </div>
  );
};

export default TokenUsageIndicator;
