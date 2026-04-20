/**
 * AssistantQuickInput — standalone input for the assistant detail page.
 *
 * Sends a message by:
 *   1. Creating a new session under the assistant workspace
 *   2. Sending the message as the first turn
 *   3. Navigating to the session scene
 *
 * Completely independent from the main ChatInput / FlowChat stores.
 */

import React, { useCallback, useState } from 'react';
import { ArrowUp, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { IconButton, Textarea } from '@/component-library';
import { ModelSelector } from '@/flow_chat/components/ModelSelector';
import { flowChatManager } from '@/flow_chat/services/FlowChatManager';
import { openMainSession } from '@/flow_chat/services/openBtwSession';
import { useImeEnterGuard } from '@/flow_chat/hooks/useImeEnterGuard';
import { useWorkspaceContext } from '@/infrastructure/contexts/WorkspaceContext';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import './AssistantQuickInput.scss';

const log = createLogger('AssistantQuickInput');

interface AssistantQuickInputProps {
  workspacePath: string;
  workspaceId?: string;
  assistantName?: string;
}

const AssistantQuickInput: React.FC<AssistantQuickInputProps> = ({
  workspacePath,
  workspaceId,
  assistantName,
}) => {
  const { t } = useTranslation('flow-chat');
  const { setActiveWorkspace } = useWorkspaceContext();
  const [value, setValue] = useState('');
  const [sending, setSending] = useState(false);
  const { isImeEnter, handleCompositionStart, handleCompositionEnd } = useImeEnterGuard();

  const handleChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setValue(e.target.value);
  }, []);

  const handleSend = useCallback(async () => {
    const text = value.trim();
    if (!text || sending || !workspacePath) return;

    setSending(true);
    try {
      // Switch to the assistant workspace first
      if (workspaceId) {
        await setActiveWorkspace(workspaceId);
      }

      // Create a new session
      const sessionId = await flowChatManager.createChatSession({ workspacePath });

      // Send the message
      await flowChatManager.sendMessage(text, sessionId);

      // Navigate to the session scene
      await openMainSession(sessionId, {
        workspaceId,
        activateWorkspace: workspaceId
          ? async (id: string) => { await setActiveWorkspace(id); }
          : undefined,
      });

      setValue('');
    } catch (err) {
      log.error('send quick message', err);
      notificationService.error(t('errors.sendFailed', { defaultValue: 'Failed to send message' }));
    } finally {
      setSending(false);
    }
  }, [value, sending, workspacePath, workspaceId, setActiveWorkspace, t]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      if (isImeEnter(e)) return;
      e.preventDefault();
      void handleSend();
    }
  }, [handleSend, isImeEnter]);

  const placeholder = assistantName
    ? t('input.assistantPlaceholder', { name: assistantName, defaultValue: `Message ${assistantName}…` })
    : t('input.placeholder', { defaultValue: 'Send a message…' });

  return (
    <div className="aqi">
      <div className="aqi__box">
        <Textarea
          className="aqi__embed"
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          onCompositionStart={handleCompositionStart}
          onCompositionEnd={handleCompositionEnd}
          placeholder={placeholder}
          rows={1}
          disabled={sending}
          autoResize
          variant="default"
        />
        <div className="aqi__footer">
          <div className="aqi__footer-left">
            <ModelSelector currentMode="Claw" className="aqi__model" />
            <span className="aqi__hint">
              {t('input.sendHint', { defaultValue: 'Enter to send · Shift+Enter for new line' })}
            </span>
          </div>
          <IconButton
            type="button"
            variant="success"
            size="small"
            isLoading={sending}
            disabled={!value.trim() || sending}
            onClick={() => { void handleSend(); }}
            aria-label={t('actions.send', { defaultValue: 'Send' })}
            className="aqi__send"
          >
            {sending
              ? <Loader2 size={14} />
              : <ArrowUp size={14} />}
          </IconButton>
        </div>
      </div>
    </div>
  );
};

export default AssistantQuickInput;
