import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  AppWindow,
  AlertTriangle,
  Camera,
  ChevronsUpDown,
  Clipboard,
  Clock,
  ExternalLink,
  Info,
  Keyboard,
  Monitor,
  Move,
  MousePointer2,
  MousePointerClick,
  Settings,
  Terminal,
} from 'lucide-react';

import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import type { ToolCardProps } from '../types/flow-chat';
import { CompactToolCard, CompactToolCardHeader } from './CompactToolCard';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import './ComputerUseToolCard.scss';

const log = createLogger('ComputerUseToolCard');

interface ParsedComputerUseResult {
  action: string;
  appName: string | null;
  target: string | null;
  loopWarningSuggestion: string | null;
  errorMessage: string | null;
}

function firstNonEmptyString(...values: unknown[]): string | null {
  for (const value of values) {
    if (typeof value === 'string' && value.trim().length > 0) return value;
  }
  return null;
}

/**
 * Best-effort summary of a ComputerUse tool call, built from `toolCall.input`
 * (always present, backend-implementation-agnostic) plus the optional
 * `computer_use_context` / `loop_warning` envelope fields that most desktop
 * actions attach to their JSON result. Never throws on unknown shapes —
 * missing fields just fall back to `null` and are hidden in the UI.
 */
function parseComputerUseResult(toolItem: ToolCardProps['toolItem']): ParsedComputerUseResult {
  const input = (toolItem.toolCall?.input ?? {}) as Record<string, unknown>;
  const result = (toolItem.toolResult?.result ?? {}) as Record<string, unknown>;
  const action = firstNonEmptyString(input.action, result.action) ?? 'computer_use';

  const context = result.computer_use_context as Record<string, unknown> | undefined;
  const foreground = (context?.foreground_application ?? result.foreground_application) as
    | { name?: string | null; bundle_id?: string | null }
    | undefined;
  const appName = firstNonEmptyString(foreground?.name, foreground?.bundle_id);

  const keys = Array.isArray(input.keys)
    ? (input.keys as unknown[]).filter((k): k is string => typeof k === 'string')
    : null;
  const target = firstNonEmptyString(
    input.target_text,
    input.text_query,
    input.text,
    keys && keys.length > 0 ? keys.join(' + ') : null,
  );

  const loopWarning = result.loop_warning as { detected?: boolean; suggestion?: string } | undefined;

  return {
    action,
    appName,
    target,
    loopWarningSuggestion: loopWarning?.detected ? loopWarning.suggestion ?? null : null,
    errorMessage: toolItem.toolResult?.error ?? null,
  };
}

function isPermissionDeniedError(message: string | null): boolean {
  if (!message) return false;
  return message.includes('[PERMISSION_DENIED]')
    || /accessibility permission/i.test(message)
    || /screen capture permission/i.test(message)
    || /screen recording permission/i.test(message);
}

async function openComputerUseSettings(pane: 'accessibility' | 'screen_capture'): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('computer_use_open_system_settings', { request: { pane } });
}

/** Groups the ~40 ComputerUse actions into a handful of recognizable icons instead of one icon per action. */
function actionIcon(action: string): React.ReactNode {
  if (action.includes('screenshot') || action === 'describe_screen') return <Camera size={16} />;
  if (action.includes('click')) return <MousePointerClick size={16} />;
  if (action.includes('scroll')) return <ChevronsUpDown size={16} />;
  if (action === 'drag') return <Move size={16} />;
  if (action.includes('move') || action === 'locate') return <MousePointer2 size={16} />;
  if (action === 'key_chord' || action === 'type_text' || action === 'paste') return <Keyboard size={16} />;
  if (action === 'wait') return <Clock size={16} />;
  if (
    action === 'list_apps'
    || action === 'get_app_state'
    || action === 'get_app_shortcuts'
    || action === 'list_displays'
    || action === 'focus_display'
    || action.startsWith('app_')
    || action.startsWith('interactive_')
    || action.startsWith('build_')
    || action.startsWith('visual_')
  ) {
    return <AppWindow size={16} />;
  }
  if (action.startsWith('open_')) return <ExternalLink size={16} />;
  if (action.startsWith('clipboard_')) return <Clipboard size={16} />;
  if (action === 'run_script' || action === 'run_apple_script') return <Terminal size={16} />;
  if (action === 'get_os_info') return <Info size={16} />;
  return <Monitor size={16} />;
}

export const ComputerUseToolCard: React.FC<ToolCardProps> = ({ toolItem, onExpand }) => {
  const { t } = useTranslation('flow-chat');
  const { status } = toolItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const toolId = toolItem.id ?? toolItem.toolCall?.id;
  const { cardRootRef, applyExpandedState } = useToolCardHeightContract({
    toolId,
    toolName: toolItem.toolName,
  });

  const parsed = useMemo(() => parseComputerUseResult(toolItem), [toolItem]);
  const errorMessage = parsed.errorMessage || t('toolCards.computerUse.actionFailed');
  const permissionDenied = status === 'error' && isPermissionDeniedError(parsed.errorMessage);
  const isExpandable = status === 'completed' || status === 'error';

  const handleOpenSettings = async (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    try {
      const pane = parsed.errorMessage?.toLowerCase().includes('screen')
        ? 'screen_capture' as const
        : 'accessibility' as const;
      await openComputerUseSettings(pane);
    } catch (error) {
      log.error('computer_use_open_system_settings failed', { error });
      notificationService.error(t('toolCards.computerUse.openSettingsFailed'));
    }
  };

  const handleClick = () => {
    if (!isExpandable) return;
    applyExpandedState(isExpanded, !isExpanded, setIsExpanded, { onExpand });
  };

  const renderContent = () => {
    if (status === 'error') {
      if (permissionDenied) return t('toolCards.computerUse.permissionDenied');
      return errorMessage;
    }
    if (status === 'completed') {
      if (parsed.appName) return t('toolCards.computerUse.controllingApp', { app: parsed.appName });
      return parsed.target ?? t('toolCards.computerUse.done');
    }
    return parsed.appName
      ? t('toolCards.computerUse.runningOnApp', { app: parsed.appName })
      : t('toolCards.computerUse.running');
  };

  const renderExpandedContent = () => {
    if (status === 'error') {
      return (
        <div className="compact-result-content computer-use-tool-card__content">
          {permissionDenied ? (
            <div className="computer-use-tool-card__permission-denied">
              <p>{t('toolCards.computerUse.permissionDeniedHint')}</p>
              <button
                type="button"
                className="computer-use-tool-card__settings-button"
                onClick={(event) => void handleOpenSettings(event)}
              >
                <Settings size={12} />
                <span>{t('toolCards.computerUse.openSettings')}</span>
              </button>
            </div>
          ) : (
            <pre>{errorMessage}</pre>
          )}
        </div>
      );
    }

    return (
      <div className="computer-use-tool-card__expanded">
        <div className="computer-use-tool-card__row">
          <span className="computer-use-tool-card__row-label">{t('toolCards.computerUse.actionLabel')}</span>
          <code>{parsed.action}</code>
        </div>
        {parsed.appName && (
          <div className="computer-use-tool-card__row">
            <span className="computer-use-tool-card__row-label">{t('toolCards.computerUse.appLabel')}</span>
            <span>{parsed.appName}</span>
          </div>
        )}
        {parsed.target && (
          <div className="computer-use-tool-card__row">
            <span className="computer-use-tool-card__row-label">{t('toolCards.computerUse.targetLabel')}</span>
            <span>{parsed.target}</span>
          </div>
        )}
        {parsed.loopWarningSuggestion && (
          <div className="computer-use-tool-card__loop-warning">
            <AlertTriangle size={12} />
            <span>{parsed.loopWarningSuggestion}</span>
          </div>
        )}
      </div>
    );
  };

  return (
    <div ref={cardRootRef} data-tool-card-id={toolId ?? ''}>
      <CompactToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleClick}
        className="computer-use-tool-card"
        clickable={isExpandable}
        header={(
          <CompactToolCardHeader
            icon={(
              <ToolCardStatusSlot
                status={status}
                toolIcon={actionIcon(parsed.action)}
                defaultIcon={status === 'completed' || status === 'error' ? 'tool' : 'status'}
              />
            )}
            action={<code className="computer-use-tool-card__action-code">{parsed.action}</code>}
            content={renderContent()}
          />
        )}
        expandedContent={isExpandable ? renderExpandedContent() : undefined}
      />
    </div>
  );
};
