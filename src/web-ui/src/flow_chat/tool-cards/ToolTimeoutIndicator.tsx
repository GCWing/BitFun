import React, { useRef, useEffect } from 'react';
import {
  AlertCircle,
  CheckCircle2,
  Timer,
  Infinity as InfinityIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useLiveElapsedTime } from '../hooks/useLiveElapsedTime';
import { useSubagentTimeoutControl } from '../hooks/useSubagentTimeoutControl';
import './ToolTimeoutIndicator.scss';

function formatDurationLive(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) {
    return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`;
  }
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return remainingMinutes > 0 ? `${hours}h ${remainingMinutes}m` : `${hours}h`;
}

function formatDurationPrecise(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = Math.round(seconds % 60);
  return `${minutes}m ${remainingSeconds}s`;
}

export interface ToolTimeoutIndicatorProps {
  startTime?: number;
  isRunning: boolean;
  timeoutMs?: number;
  showControls?: boolean;
  subagentSessionId?: string;
  completedDurationMs?: number;
  completedStatus?: 'success' | 'error' | 'cancelled';
  completedTooltip?: string;
  completedFailureReason?: string;
}

function renderCompletedDurationIcon(status: ToolTimeoutIndicatorProps['completedStatus']) {
  if (status === 'success') {
    return <CheckCircle2 size={13} strokeWidth={2.2} />;
  }
  if (status === 'error' || status === 'cancelled') {
    return <AlertCircle size={13} strokeWidth={2.2} />;
  }
  return <Timer size={13} strokeWidth={2} />;
}

export const ToolTimeoutIndicator: React.FC<ToolTimeoutIndicatorProps> = ({
  startTime,
  isRunning,
  timeoutMs,
  showControls = false,
  subagentSessionId,
  completedDurationMs,
  completedStatus,
  completedTooltip,
  completedFailureReason,
}) => {
  const { t } = useTranslation('flow-chat');
  const {
    isTimeoutDisabled,
    isToggling,
    isPopoverOpen,
    toggleTimeout,
    extendTimeout,
    closePopover,
    remainingAtDisable,
  } = useSubagentTimeoutControl(subagentSessionId, isRunning, timeoutMs, null);

  const { elapsedMs, remainingMs } = useLiveElapsedTime(
    startTime,
    isRunning,
    timeoutMs,
    isTimeoutDisabled,
  );

  const popoverRef = useRef<HTMLDivElement>(null);

  // Close popover on outside click.
  useEffect(() => {
    if (!isPopoverOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        closePopover();
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [isPopoverOpen, closePopover]);

  // Close popover on Escape.
  useEffect(() => {
    if (!isPopoverOpen) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closePopover();
    };
    document.addEventListener('keydown', handleKey);
    return () => document.removeEventListener('keydown', handleKey);
  }, [isPopoverOpen, closePopover]);

  // Completed state: show precise duration only.
  if (!isRunning && completedDurationMs != null) {
    const durationLabel = formatDurationPrecise(completedDurationMs);
    const completionLabel = completedTooltip || (
      completedStatus === 'success'
        ? t('toolCards.timeout.completedDurationTooltip', {
          duration: durationLabel,
          defaultValue: `Completed in ${durationLabel}`,
        })
        : completedStatus === 'error'
          ? completedFailureReason
            ? t('toolCards.timeout.failedDurationTooltipWithReason', {
              duration: durationLabel,
              reason: completedFailureReason,
              defaultValue: `Failed after ${durationLabel}: ${completedFailureReason}`,
            })
            : t('toolCards.timeout.failedDurationTooltip', {
              duration: durationLabel,
              defaultValue: `Failed after ${durationLabel}`,
            })
          : completedStatus === 'cancelled'
            ? t('toolCards.timeout.cancelledDurationTooltip', {
              duration: durationLabel,
              defaultValue: `Cancelled after ${durationLabel}`,
            })
            : t('toolCards.timeout.durationTooltip', {
              duration: durationLabel,
              defaultValue: `Duration ${durationLabel}`,
            })
    );

    return (
      <span
        className={`duration-text duration-text--completed${completedStatus ? ` duration-text--completed-${completedStatus}` : ''}`}
        title={completionLabel}
        aria-label={completionLabel}
      >
        {renderCompletedDurationIcon(completedStatus)}
        {durationLabel}
      </span>
    );
  }

  // Not running and no completed duration: nothing to show.
  if (!isRunning) return null;

  const hasTimeout = Boolean(timeoutMs && timeoutMs > 0);
  const displayRemaining = isTimeoutDisabled ? null : remainingMs;

  // Determine warning threshold: remaining < 20% of original timeout.
  const isWarning =
    displayRemaining != null &&
    timeoutMs != null &&
    timeoutMs > 0 &&
    displayRemaining < timeoutMs * 0.2;

  return (
    <span className="tool-timeout-indicator">
      <span className={`duration-text duration-text--live ${isWarning ? 'duration-text--warning' : ''}`}>
        <Timer size={13} strokeWidth={2} />
        <span className="duration-elapsed">{formatDurationLive(elapsedMs)}</span>
        {hasTimeout && (
          <>
            <span className="duration-separator">/</span>
            <span
              className={`duration-timeout ${isTimeoutDisabled ? 'duration-timeout--disabled' : ''} ${isWarning ? 'duration-timeout--warning' : ''}`}
            >
              {isTimeoutDisabled
                ? <InfinityIcon size={14} className="duration-timeout--infinity" />
                : displayRemaining != null
                  ? formatDurationLive(displayRemaining)
                  : formatDurationLive(timeoutMs!)}
            </span>
          </>
        )}
      </span>

      {showControls && hasTimeout && (
        <div className="timeout-control-wrapper" ref={popoverRef}>
          <button
            type="button"
            className={`timeout-ignore-btn ${isTimeoutDisabled ? 'is-active' : ''}`}
            onClick={(e) => {
              e.stopPropagation();
              toggleTimeout();
            }}
            disabled={isToggling}
            title={
              isTimeoutDisabled
                ? t('toolCards.timeout.enableTooltip')
                : t('toolCards.timeout.disableTooltip')
            }
          >
            <InfinityIcon size={12} />
            <span className="timeout-ignore-btn__label">
              {isTimeoutDisabled
                ? t('toolCards.timeout.enableLabel')
                : t('toolCards.timeout.disableLabel')}
            </span>
          </button>

          {isPopoverOpen && (
            <div className="timeout-extend-popover">
              {remainingAtDisable > 0 ? (
                <button
                  type="button"
                  className="timeout-extend-option timeout-extend-option--danger"
                  onClick={(e) => {
                    e.stopPropagation();
                    extendTimeout(remainingAtDisable);
                  }}
                >
                  {t('toolCards.timeout.restoreShort', { seconds: remainingAtDisable })}
                </button>
              ) : null}
              <button
                type="button"
                className="timeout-extend-option"
                onClick={(e) => {
                  e.stopPropagation();
                  extendTimeout(60);
                }}
              >
                +1m
              </button>
              <button
                type="button"
                className="timeout-extend-option"
                onClick={(e) => {
                  e.stopPropagation();
                  extendTimeout(300);
                }}
              >
                +5m
              </button>
              <button
                type="button"
                className="timeout-extend-option"
                onClick={(e) => {
                  e.stopPropagation();
                  extendTimeout(600);
                }}
              >
                +10m
              </button>
            </div>
          )}
        </div>
      )}
    </span>
  );
};
