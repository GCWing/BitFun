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
import { flowChatSessionConfigForCurrentWorkspace } from '@/app/utils/projectSessionWorkspace';
import { notificationService } from '@/shared/notification-system';
import type {
  ContextItem,
  ImageContext,
  SessionReferenceContext,
} from '@/shared/types/context';
import { createLogger } from '@/shared/utils/logger';
import { formatContextForPrompt } from '@/shared/utils/contextPrompt';
import { buildImagePayload } from '../utils/imagePayload';
import {
  FLOWCHAT_MESSAGE_SUBMITTED_EVENT,
  type FlowChatMessageSubmittedRequest,
} from '../events/flowchatNavigation';
import {
  composerPresentationSessionReferences,
  type ComposerPresentation,
} from '../utils/composerPresentation';
import type {
  AgentDialogTurnExecution,
  SessionPermissionMode,
} from '@/infrastructure/api/service-api/AgentAPI';
import type { PendingLargePasteMap } from '../store/sessionComposerStore';

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
  /** Reconcile the composer after an explicit session-conflict retry succeeds. */
  onSessionConflictRetrySuccess?: (submission: {
    sessionId: string;
    message: string;
    contextIds: string[];
  }) => void;
  /** Capture composer state when the user explicitly starts a conflict retry. */
  onSessionConflictRetryStart?: (submission: {
    sessionId: string;
    message: string;
    contextIds: string[];
  }) => void;
  /**
   * One-off permission mode armed for the next submission only. It outranks the
   * session's own mode for that turn and is never persisted, so the session
   * returns to its own selection afterwards.
   */
  turnPermissionMode?: SessionPermissionMode | null;
  /** Disarms the one-off mode once a submission has carried it. */
  onTurnPermissionModeConsumed?: () => void;
}

interface UseMessageSenderReturn {
  /** Send a message */
  sendMessage: (
    message: string,
    options?: {
      displayMessage?: string;
      composerPresentation?: ComposerPresentation | null;
      composerDraft?: {
        value: string;
        pendingLargePastes: PendingLargePasteMap;
      };
      execution?: AgentDialogTurnExecution;
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
    onSessionConflictRetryStart,
    onSessionConflictRetrySuccess,
    turnPermissionMode,
    onTurnPermissionModeConsumed,
  } = props;

  const sendMessage = useCallback(async (
    message: string,
    options?: {
      displayMessage?: string;
      composerPresentation?: ComposerPresentation | null;
      composerDraft?: {
        value: string;
        pendingLargePastes: PendingLargePasteMap;
      };
      execution?: AgentDialogTurnExecution;
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
      if (options?.execution?.kind === 'fresh_external_subagent' && contexts.length > 0) {
        throw new Error('External subagent command delegation does not accept composer context');
      }

      if (!sessionId) {
        const agentType = currentAgentType || 'agentic';
        const sessionConfig = flowChatSessionConfigForCurrentWorkspace();

        sessionId = await flowChatManager.createChatSession(sessionConfig, agentType);
        agentTypeForSend =
          FlowChatManager.getInstance().getFlowChatState().sessions.get(sessionId)?.mode ||
          agentType;
        log.debug('Session created', { sessionId, agentType, effectiveAgentType: agentTypeForSend });
      } else {
        log.debug('Reusing existing session', { sessionId });
      }

      const imageContexts = contexts.filter(ctx => ctx.type === 'image') as ImageContext[];
      const presentationSessionReferences = options?.composerPresentation
        ? composerPresentationSessionReferences(options.composerPresentation)
        : [];
      const sessionReferenceContexts = presentationSessionReferences.length > 0
        ? presentationSessionReferences
        : contexts
        .filter((context): context is SessionReferenceContext => context.type === 'session-reference')
      const sessionReferences = sessionReferenceContexts.map((context) => ({
          sessionId: context.sessionId,
          workspacePath: context.workspacePath,
          remoteConnectionId: context.remoteConnectionId,
          remoteSshHost: context.remoteSshHost,
        }));
      const userMessageMetadata =
        options?.composerPresentation || sessionReferences.length > 0 || turnPermissionMode
          ? {
              ...(options?.composerPresentation
                ? { composerPresentation: options.composerPresentation }
                : {}),
              ...(sessionReferences.length > 0 ? { sessionReferences } : {}),
              // Read by the coordinator as the turn layer of
              // `turn -> session -> global default`.
              ...(turnPermissionMode ? { permission_mode: turnPermissionMode } : {}),
            }
          : undefined;
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
        const fullContextSection = contexts
          .filter(context => context.type !== 'session-reference')
          .map(formatContextForPrompt)
          .filter(Boolean)
          .join('\n');

        fullMessage = fullContextSection
          ? `${fullContextSection}\n\n${aiTrimmedMessage}`
          : aiTrimmedMessage;
      }
      // Always pass imageContexts to the backend; the coordinator decides
      // whether to pre-analyse via a vision model or attach directly.
      await flowChatManager.sendMessage(
        fullMessage,
        sessionId || undefined,
        displayMessage,
        agentTypeForSend,
        undefined,
        {
          ...(imagePayload ?? {}),
          pendingQueueDraft: {
            value: options?.composerDraft?.value ?? displayMessage,
            contexts: [...contexts],
            pendingLargePastes: { ...(options?.composerDraft?.pendingLargePastes ?? {}) },
          },
          ...(userMessageMetadata ? { userMessageMetadata } : {}),
          ...(options?.execution ? { execution: options.execution } : {}),
          onSessionConflictRetryStart: () => {
            onSessionConflictRetryStart?.({
              sessionId: sessionId!,
              message: displayMessage,
              contextIds: contexts.map(context => context.id),
            });
          },
          onSessionConflictRetrySuccess: () => {
            onSessionConflictRetrySuccess?.({
              sessionId: sessionId!,
              message: displayMessage,
              contextIds: contexts.map(context => context.id),
            });
          },
        }
      );

      onClearContexts();

      // The one-off mode belongs to the submission that just left, not to the
      // next one the user types.
      if (turnPermissionMode) {
        onTurnPermissionModeConsumed?.();
      }
      onExitTemplateMode?.();

      onSuccess?.(trimmedMessage);
      /*
       * The transcript may be showing a history window this Turn is not in, and
       * only the submission knows that showing it was asked for. Announced
       * rather than returned: the composer and the transcript are siblings, and
       * every host that sends through this hook gets the behaviour.
       */
      window.dispatchEvent(new CustomEvent<FlowChatMessageSubmittedRequest>(
        FLOWCHAT_MESSAGE_SUBMITTED_EVENT,
        { detail: { sessionId } },
      ));
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
  }, [
    currentSessionId,
    contexts,
    onClearContexts,
    onSuccess,
    onExitTemplateMode,
    currentAgentType,
    onSessionConflictRetryStart,
    onSessionConflictRetrySuccess,
    turnPermissionMode,
    onTurnPermissionModeConsumed,
  ]);

  return {
    sendMessage,
    isSending: false,
  };
}
