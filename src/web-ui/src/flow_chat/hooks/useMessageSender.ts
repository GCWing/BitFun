/**
 * Message sending hook.
 * Encapsulates session creation, image uploads, and message assembly.
 *
 * Image handling is fully delegated to the backend coordinator which
 * decides whether to pre-analyse via a vision model or attach images
 * directly.  The frontend only uploads clipboard images and passes
 * ImageContextData[] through to the backend.
 */

import { useCallback } from 'react';
import { FlowChatManager } from '../services/FlowChatManager';
import { notificationService } from '@/shared/notification-system';
import type { ContextItem, ImageContext } from '@/shared/types/context';
import { createLogger } from '@/shared/utils/logger';
import { formatContextForPrompt } from '@/shared/utils/contextPrompt';
import { buildImagePayload } from '../utils/imagePayload';

const log = createLogger('FlowChat');

interface UseMessageSenderProps {
  /** Current session ID */
  currentSessionId?: string;
  /** Context items */
  contexts: ContextItem[];
  /** Clear contexts callback */
  onClearContexts: () => void;
  /** Success callback */
  onSuccess?: (message: string) => void;
  /** Exit template mode callback */
  onExitTemplateMode?: () => void;
  /** Selected agent type (mode) */
  currentAgentType?: string;
}

interface UseMessageSenderReturn {
  /** Send a message */
  sendMessage: (
    message: string,
    options?: {
      displayMessage?: string;
    }
  ) => Promise<void>;
  /** Whether a send is in progress */
  isSending: boolean;
}

export function useMessageSender(props: UseMessageSenderProps): UseMessageSenderReturn {
  const {
    currentSessionId,
    contexts,
    onClearContexts,
    onSuccess,
    onExitTemplateMode,
    currentAgentType,
  } = props;

  const sendMessage = useCallback(async (
    message: string,
    options?: {
      displayMessage?: string;
    }
  ) => {
    if (!message.trim()) {
      return;
    }

    const trimmedMessage = message.trim();
    // Strip inline `#img:<name>` tags from the AI-bound text. The rich text
    // editor inserts these when an image is pasted, but the named file does
    // not exist on disk; image bytes are sent out-of-band via `imageContexts`
    // below. Leaving the placeholder in the prompt misleads the model into
    // looking up a non-existent file. The display message keeps the tag so
    // the UI can still render the inline pill.
    const stripImageTags = (text: string): string =>
      text
        .replace(/#img:[^\s\n]+\s?/g, '')
        .replace(/[ \t]+\n/g, '\n')
        .replace(/\n{3,}/g, '\n\n')
        .trim();
    const aiTrimmedMessage = stripImageTags(trimmedMessage);
    let sessionId = currentSessionId;
    log.debug('Send message initiated', {
      textLength: trimmedMessage.length,
      contextCount: contexts.length,
      hasSession: !!sessionId,
      agentType: currentAgentType || 'agentic',
    });

    try {
      const flowChatManager = FlowChatManager.getInstance();
      let agentTypeForSend = currentAgentType || 'agentic';

      if (!sessionId) {
        const agentType = currentAgentType || 'agentic';

        sessionId = await flowChatManager.createChatSession({}, agentType);
        agentTypeForSend =
          FlowChatManager.getInstance().getFlowChatState().sessions.get(sessionId)?.mode ||
          agentType;
        log.debug('Session created', { sessionId, agentType, effectiveAgentType: agentTypeForSend });
      } else {
        log.debug('Reusing existing session', { sessionId });
      }

      const imageContexts = contexts.filter(ctx => ctx.type === 'image') as ImageContext[];
      let imagePayload: Awaited<ReturnType<typeof buildImagePayload>>;
      try {
        imagePayload = await buildImagePayload(imageContexts);
        log.debug('Image payload prepared', {
          imageCount: imageContexts.length,
          ids: imageContexts.map(img => img.id),
          pathCount: imagePayload?.imageContexts.filter(img => img.image_path).length ?? 0,
        });
      } catch (error) {
        log.error('Failed to upload clipboard images', {
          imageCount: imageContexts.filter(ctx => !ctx.isLocal && ctx.dataUrl).length,
          error: (error as Error)?.message ?? 'unknown',
        });
        notificationService.error('Image upload failed. Please try again.', { duration: 3000 });
        throw error;
      }

      let fullMessage = aiTrimmedMessage;
      const displayMessage = options?.displayMessage?.trim() || trimmedMessage;

      if (contexts.length > 0) {
        const fullContextSection = contexts.map(formatContextForPrompt).filter(Boolean).join('\n');

        fullMessage = `${fullContextSection}\n\n${aiTrimmedMessage}`;
      }

      // Always pass imageContexts to the backend; the coordinator decides
      // whether to pre-analyse via a vision model or attach directly.
      await flowChatManager.sendMessage(
        fullMessage,
        sessionId || undefined,
        displayMessage,
        agentTypeForSend,
        undefined,
        imagePayload
      );

      onClearContexts();

      onExitTemplateMode?.();

      onSuccess?.(trimmedMessage);
      log.info('Message sent successfully', {
        sessionId,
        agentType: agentTypeForSend,
        contextCount: contexts.length,
        imageCount: imageContexts.length,
      });
    } catch (error) {
      log.error('Failed to send message', {
        sessionId,
        agentType: currentAgentType || 'agentic',
        contextCount: contexts.length,
        error: (error as Error)?.message ?? 'unknown',
      });
      throw error;
    }
  }, [currentSessionId, contexts, onClearContexts, onSuccess, onExitTemplateMode, currentAgentType]);

  return {
    sendMessage,
    isSending: false,
  };
}
