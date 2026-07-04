import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Check, MessageSquareX, ShieldCheck, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { IconButton } from '@/component-library';
import { configManager } from '@/infrastructure/config';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { AcpPermissionActions } from '../tool-cards/AcpPermissionActions';
import { hasAcpPermissionOptions } from '../tool-cards/AcpPermissionActions.utils';
import type { FlowToolItem, ToolRejectOptions } from '../types/flow-chat';
import './ToolApprovalBar.scss';

const log = createLogger('ToolApprovalBar');

interface ToolApprovalBarProps {
  toolItem: FlowToolItem;
  onConfirm?: (updatedInput?: any, permissionOptionId?: string, approve?: boolean) => void;
  onReject?: (options?: ToolRejectOptions) => void;
}

function hasPendingToolConfirmation(toolItem: FlowToolItem): boolean {
  return toolItem.status === 'pending_confirmation';
}

function formatRemainingConfirmationTime(remainingMs: number): string {
  if (remainingMs < 1000) return '1s';
  const totalSeconds = Math.ceil(remainingMs / 1000);
  if (totalSeconds < 60) return `${totalSeconds}s`;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return seconds > 0 ? `${minutes}m ${seconds}s` : `${minutes}m`;
}

export const ToolApprovalBar: React.FC<ToolApprovalBarProps> = ({
  toolItem,
  onConfirm,
  onReject,
}) => {
  const { t } = useTranslation('flow-chat');
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [showInstructionInput, setShowInstructionInput] = useState(false);
  const [instruction, setInstruction] = useState('');
  const [isEnablingAutoExecute, setIsEnablingAutoExecute] = useState(false);
  const instructionInputRef = useRef<HTMLInputElement | null>(null);
  const input = toolItem.toolCall?.input;
  const hasPermissionOptions = hasAcpPermissionOptions(toolItem);
  const canConfirm = useMemo(() => {
    if (toolItem.toolName === 'Bash') {
      const command = typeof input?.command === 'string' ? input.command : '';
      return Boolean(command.trim());
    }

    if (toolItem.toolName === 'ExecCommand') {
      const command = typeof input?.cmd === 'string' ? input.cmd : '';
      return Boolean(command.trim());
    }

    return true;
  }, [input, toolItem.toolName]);
  const confirmationTimeoutAt = toolItem.confirmationTimeoutAt;
  const remainingConfirmationMs = useMemo(() => {
    if (typeof confirmationTimeoutAt !== 'number') {
      return null;
    }
    return Math.max(0, confirmationTimeoutAt - nowMs);
  }, [confirmationTimeoutAt, nowMs]);
  const confirmationCountdownLabel = useMemo(() => {
    if (remainingConfirmationMs == null) {
      return null;
    }
    if (remainingConfirmationMs > 10 * 60 * 1000) {
      return null;
    }
    return formatRemainingConfirmationTime(remainingConfirmationMs);
  }, [remainingConfirmationMs]);

  useEffect(() => {
    if (typeof confirmationTimeoutAt !== 'number') {
      return undefined;
    }

    const tick = () => setNowMs(Date.now());
    tick();
    const handle = window.setInterval(tick, 1000);
    return () => window.clearInterval(handle);
  }, [confirmationTimeoutAt]);

  if (!hasPendingToolConfirmation(toolItem)) {
    return null;
  }

  const handleConfirm = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();

    if (!canConfirm) {
      return;
    }

    onConfirm?.(input);
  };

  const handleReject = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    onReject?.();
  };

  const handleEnableAutoExecute = async (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();

    if (isEnablingAutoExecute) {
      return;
    }

    setIsEnablingAutoExecute(true);
    try {
      await configManager.setConfig('ai.skip_tool_confirmation', true);
      notificationService.success(t('toolCards.approval.autoExecuteEnabled'), { duration: 2000 });
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to enable tool auto execute', { error });
      notificationService.error(t('toolCards.approval.autoExecuteEnableFailed'));
    } finally {
      setIsEnablingAutoExecute(false);
    }
  };

  const handleRejectWithInstruction = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();

    const trimmedInstruction = (instructionInputRef.current?.value ?? instruction).trim();
    onReject?.(trimmedInstruction ? { instruction: trimmedInstruction } : undefined);
  };

  const handleToggleInstructionInput = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    setShowInstructionInput((value) => !value);
  };

  const handleInstructionKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    event.stopPropagation();

    if (event.key === 'Enter') {
      event.preventDefault();
      const trimmedInstruction = event.currentTarget.value.trim();
      onReject?.(trimmedInstruction ? { instruction: trimmedInstruction } : undefined);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      setShowInstructionInput(false);
      setInstruction('');
    }
  };

  return (
    <div className="tool-approval-bar" role="group" aria-label={t('toolCards.approval.ariaLabel')}>
      <div className="tool-approval-bar__main">
        <span className="tool-approval-bar__message">
          {t('toolCards.approval.waiting')}
          {confirmationCountdownLabel ? (
            <span className="tool-approval-bar__countdown">
              {' '}
              · {t('toolCards.approval.remaining', { time: confirmationCountdownLabel })}
            </span>
          ) : null}
        </span>
        <span className="tool-approval-bar__actions">
          {hasPermissionOptions ? (
            <AcpPermissionActions
              toolItem={toolItem}
              input={input}
              disabled={!canConfirm}
              presentation="text"
              className="tool-approval-bar__permission-actions"
              onConfirm={onConfirm}
              onReject={onReject}
            />
          ) : (
            <>
              <IconButton
                className="tool-approval-bar__icon-button"
                variant="success"
                size="xs"
                onClick={handleConfirm}
                disabled={!canConfirm}
                tooltip={
                  canConfirm
                    ? t('toolCards.approval.confirmTooltip')
                    : t('toolCards.approval.emptyInputTooltip')
                }
                aria-label={t('toolCards.approval.confirm')}
              >
                <Check size={13} />
              </IconButton>
              <IconButton
                className="tool-approval-bar__icon-button"
                variant="danger"
                size="xs"
                onClick={handleReject}
                tooltip={t('toolCards.approval.rejectTooltip')}
                aria-label={t('toolCards.approval.reject')}
              >
                <X size={13} />
              </IconButton>
              <IconButton
                className="tool-approval-bar__icon-button"
                variant="danger"
                size="xs"
                onClick={handleToggleInstructionInput}
                tooltip={t('toolCards.approval.rejectWithInstructionTooltip')}
                aria-label={t('toolCards.approval.rejectWithInstruction')}
              >
                <MessageSquareX size={13} />
              </IconButton>
              <span className="tool-approval-bar__divider" aria-hidden="true" />
              <IconButton
                className="tool-approval-bar__icon-button tool-approval-bar__auto-execute-button"
                variant="default"
                size="xs"
                onClick={handleEnableAutoExecute}
                disabled={isEnablingAutoExecute}
                tooltip={t('toolCards.approval.enableAutoExecuteTooltip')}
                aria-label={t('toolCards.approval.enableAutoExecute')}
              >
                <ShieldCheck size={13} />
              </IconButton>
            </>
          )}
        </span>
      </div>
      {showInstructionInput && !hasPermissionOptions && (
        <div className="tool-approval-bar__instruction">
          <input
            ref={instructionInputRef}
            className="tool-approval-bar__instruction-input"
            value={instruction}
            onChange={(event) => setInstruction(event.target.value)}
            onClick={(event) => event.stopPropagation()}
            onKeyDown={handleInstructionKeyDown}
            placeholder={t('toolCards.approval.rejectInstructionPlaceholder')}
            aria-label={t('toolCards.approval.rejectInstructionLabel')}
          />
          <button
            type="button"
            className="tool-approval-bar__instruction-submit"
            onClick={handleRejectWithInstruction}
          >
            {t('toolCards.approval.rejectWithInstructionSubmit')}
          </button>
        </div>
      )}
    </div>
  );
};

export default ToolApprovalBar;
