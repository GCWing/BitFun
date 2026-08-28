/**
 * Compact display for the TerminalControl tool.
 */

import React, { useMemo } from 'react';
import { Terminal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ActivityItem } from '@bitfun/ui';
import type { ToolCardProps } from '../types/flow-chat';
import { ToolCardStatusSlot } from './ToolCardStatusSlot';

export const TerminalControlDisplay: React.FC<ToolCardProps> = React.memo(({
  toolItem,
}) => {
  const { t } = useTranslation('flow-chat');
  const { toolCall, status } = toolItem;

  const terminalSessionId = useMemo(() => {
    return toolCall?.input?.terminal_session_id as string | undefined;
  }, [toolCall?.input?.terminal_session_id]);

  const action = useMemo(() => {
    return (toolCall?.input?.action as string | undefined) ?? 'kill';
  }, [toolCall?.input?.action]);

  const renderContent = () => {
    const idLabel = terminalSessionId
      ? <span className="read-file-meta"> {terminalSessionId}</span>
      : null;

    const isInterrupt = action === 'interrupt';

    if (status === 'completed') {
      return (
        <>
          {isInterrupt
            ? t('toolCards.terminalControl.sessionInterrupted')
            : t('toolCards.terminalControl.sessionKilled')}
          {idLabel}
        </>
      );
    }
    if (status === 'running' || status === 'streaming') {
      return (
        <>
          {isInterrupt
            ? t('toolCards.terminalControl.interruptingSession')
            : t('toolCards.terminalControl.terminatingSession')}
          {idLabel}
          ...
        </>
      );
    }
    if (status === 'error') {
      return (
        <>
          {isInterrupt
            ? t('toolCards.terminalControl.interruptFailed')
            : t('toolCards.terminalControl.killFailed')}
          {idLabel}
        </>
      );
    }
    if (status === 'pending') {
      return (
        <>
          {isInterrupt
            ? t('toolCards.terminalControl.preparingInterrupt')
            : t('toolCards.terminalControl.preparingKill')}
          {idLabel}
        </>
      );
    }
    return null;
  };

  return (
    <ActivityItem
      appearance="inline"
      className="terminal-control-card"
      data-bf-status={status}
      leading={<ToolCardStatusSlot status={status} toolIcon={<Terminal size={16} />} />}
    >
      {renderContent()}
    </ActivityItem>
  );
});
