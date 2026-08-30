import React, { useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { Menu, MenuItem } from '@bitfun/ui';
import {
  AlertCircle,
  CheckCircle2,
  Timer,
  Infinity as InfinityIcon,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
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
  defaultTimeoutDisabled?: boolean;
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
  defaultTimeoutDisabled = false,
}) => {
  const { t } = useTranslation('flow-chat');
  const remainingMsRef = useRef<number | null>(null);

  const {
    isTimeoutDisabled,
    isToggling,
    isPopoverOpen,
    toggleTimeout,
    extendTimeout,
    closePopover,
    remainingAtDisable,
  } = useSubagentTimeoutControl(
    subagentSessionId,
    isRunning,
    timeoutMs,
    remainingMsRef.current,
    defaultTimeoutDisabled,
  );

  const { elapsedMs, remainingMs } = useLiveElapsedTime(
    startTime,
    isRunning,
    timeoutMs,
    isTimeoutDisabled,
  );
  remainingMsRef.current = remainingMs;

  const controlRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const popoverLayout = useAnchoredPopoverPosition({
    open: isPopoverOpen,
    anchorRef: triggerRef,
    popoverRef,
    preferredPlacement: 'bottom',
    alignment: 'end',
    gap: 4,
    layoutRevision: remainingAtDisable,
  });

  // Close popover on outside click.
  useEffect(() => {
    if (!isPopoverOpen) return;
    const handleClick = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        !controlRef.current?.contains(target)
        && !popoverRef.current?.contains(target)
      ) {
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
        })
        : completedStatus === 'error'
          ? completedFailureReason
            ? t('toolCards.timeout.failedDurationTooltipWithReason', {
              duration: durationLabel,
              reason: completedFailureReason,
            })
            : t('toolCards.timeout.failedDurationTooltip', {
              duration: durationLabel,
            })
          : completedStatus === 'cancelled'
            ? t('toolCards.timeout.cancelledDurationTooltip', {
              duration: durationLabel,
            })
            : t('toolCards.timeout.durationTooltip', {
              duration: durationLabel,
            })
    );

    return (
      <span data-bf-component="tool-timeout-indicator" data-bf-part="root" data-bf-mode="completed"
        className={`duration-text duration-text--completed${completedStatus ? ` duration-text--completed-${completedStatus}` : ''}`}
        title={completionLabel}
        aria-label={completionLabel}
      >
        {renderCompletedDurationIcon(completedStatus)}
        <span data-bf-component="tool-timeout-indicator" data-bf-part="duration">{durationLabel}</span>
      </span>
    );
  }

  // Not running and no completed duration: nothing to show.
  if (!isRunning) return null;

  const hasTimeout = Boolean(timeoutMs && timeoutMs > 0);
  const canControlTimeout = showControls && hasTimeout && Boolean(subagentSessionId);
  const displayRemaining = isTimeoutDisabled ? null : remainingMs;

  // Determine warning threshold: remaining < 20% of original timeout.
  const isWarning =
    displayRemaining != null &&
    timeoutMs != null &&
    timeoutMs > 0 &&
    displayRemaining < timeoutMs * 0.2;

  return (
    <span data-bf-component="tool-timeout-indicator" data-bf-part="root" data-bf-mode="live" data-bf-state={[isWarning && 'warning', isTimeoutDisabled && 'disabled', isPopoverOpen && 'open'].filter(Boolean).join(' ')} className="tool-timeout-indicator">
      <span data-bf-component="tool-timeout-indicator" data-bf-part="duration" className={`duration-text duration-text--live ${isWarning ? 'duration-text--warning' : ''}`}>
        <Timer size={13} strokeWidth={2} />
        <span data-bf-component="tool-timeout-indicator" data-bf-part="elapsed" className="duration-elapsed">{formatDurationLive(elapsedMs)}</span>
        {hasTimeout && (
          <>
            <span className="duration-separator">/</span>
            <span
              data-bf-component="tool-timeout-indicator"
              data-bf-part="timeout"
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

      {canControlTimeout && (
        <div data-bf-component="tool-timeout-indicator" data-bf-part="controls" className="timeout-control-wrapper" ref={controlRef}>
          <button
            ref={triggerRef}
            type="button"
            data-bf-component="tool-timeout-indicator"
            data-bf-part="toggle"
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

          {isPopoverOpen && createPortal(
            <Menu
              ref={popoverRef}
              data-bf-component="tool-timeout-indicator"
              data-bf-part="popover"
              data-bf-placement={popoverLayout?.placement ?? 'bottom'}
              className="timeout-extend-popover"
              style={{
                top: `${popoverLayout?.top ?? 0}px`,
                left: `${popoverLayout?.left ?? 0}px`,
                visibility: popoverLayout ? 'visible' : 'hidden',
              }}
              aria-label={t('toolCards.timeout.disableTooltip')}
              autoFocusFirstItem
            >
              {remainingAtDisable > 0 ? (
                <MenuItem
                  data-bf-component="tool-timeout-indicator"
                  data-bf-part="option"
                  onClick={(e) => {
                    e.stopPropagation();
                    extendTimeout(remainingAtDisable);
                  }}
                >
                  {t('toolCards.timeout.restoreShort', { seconds: remainingAtDisable })}
                </MenuItem>
              ) : null}
              <MenuItem
                data-bf-component="tool-timeout-indicator"
                data-bf-part="option"
                onClick={(e) => {
                  e.stopPropagation();
                  extendTimeout(60);
                }}
              >
                +1m
              </MenuItem>
              <MenuItem
                data-bf-component="tool-timeout-indicator"
                data-bf-part="option"
                onClick={(e) => {
                  e.stopPropagation();
                  extendTimeout(300);
                }}
              >
                +5m
              </MenuItem>
              <MenuItem
                data-bf-component="tool-timeout-indicator"
                data-bf-part="option"
                onClick={(e) => {
                  e.stopPropagation();
                  extendTimeout(600);
                }}
              >
                +10m
              </MenuItem>
            </Menu>,
            getAppearanceOverlayHost(),
          )}
        </div>
      )}
    </span>
  );
};
