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
import type { AIModelConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('FlowChat');

function normalizeModelSelection(
  modelId: string | undefined,
  models: AIModelConfig[],
  defaultModels: DefaultModelsConfig,
): string {
  const value = modelId?.trim();
  if (!value || value === 'auto') return 'auto';

  if (value === 'primary' || value === 'fast') {
    const resolvedDefaultId = value === 'primary' ? defaultModels.primary : defaultModels.fast;
    const matchedModel = models.find(model => model.id === resolvedDefaultId);
    return matchedModel ? value : 'auto';
  }

  const matchedModel = models.find(model =>
    model.id === value || model.name === value || model.model_name === value,
  );
  return matchedModel ? value : 'auto';
}

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

      if (!sessionId) {
        const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
        const [agentModels, allModels, defaultModels] = await Promise.all([
          configManager.getConfig<Record<string, string>>('ai.agent_models') || {},
          configManager.getConfig<AIModelConfig[]>('ai.models') || [],
          configManager.getConfig<DefaultModelsConfig>('ai.default_models') || {},
        ]);
        const agentType = currentAgentType || 'agentic';
        const modelId = normalizeModelSelection(agentModels[agentType], allModels, defaultModels);

        sessionId = await flowChatManager.createChatSession({
          modelName: modelId || undefined
        }, agentType);
        log.debug('Session created', { sessionId, modelId, agentType });
      } else {
        log.debug('Reusing existing session', { sessionId });
      }

      const imageContexts = contexts.filter(ctx => ctx.type === 'image') as ImageContext[];
      const clipboardImages = imageContexts.filter(ctx => !ctx.isLocal && ctx.dataUrl);

      if (clipboardImages.length > 0) {
        try {
          const { api } = await import('@/infrastructure/api/service-api/ApiClient');
          const uploadData = {
            request: {
              images: clipboardImages.map(ctx => ({
                id: ctx.id,
                image_path: ctx.imagePath || null,
                data_url: ctx.dataUrl || null,
                mime_type: ctx.mimeType,
                image_name: ctx.imageName,
                file_size: ctx.fileSize,
                width: ctx.width || null,
                height: ctx.height || null,
                source: ctx.source,
              }))
            }
          };

          await api.invoke('upload_image_contexts', uploadData);
          log.debug('Clipboard images uploaded', {
            imageCount: clipboardImages.length,
            ids: clipboardImages.map(img => img.id),
          });
        } catch (error) {
          log.error('Failed to upload clipboard images', {
            imageCount: clipboardImages.length,
            error: (error as Error)?.message ?? 'unknown',
          });
          notificationService.error('Image upload failed. Please try again.', { duration: 3000 });
          throw error;
        }
      }

      let fullMessage = aiTrimmedMessage;
      const displayMessage = options?.displayMessage?.trim() || trimmedMessage;

      if (contexts.length > 0) {
        const fullContextSection = contexts.map(ctx => {
          switch (ctx.type) {
            case 'file':
              return `[File: ${ctx.relativePath || ctx.filePath}]`;
            case 'directory':
              return `[Directory: ${ctx.directoryPath}]`;
            case 'code-snippet':
              return `[Code Snippet: ${ctx.filePath}:${ctx.startLine}-${ctx.endLine}]`;
            case 'image':
              // Images are sent out-of-band via `imageContexts` so the backend can attach them
              // for multimodal models or convert to text placeholders for text-only models. Avoid embedding
              // "Image ID" references into the user prompt, which can cause redundant tool calls.
              return '';
            case 'terminal-command':
              return `[Command: ${ctx.command}]`;
            case 'mermaid-node':
              return `[Mermaid Node: ${ctx.nodeText}]`;
            case 'mermaid-diagram':
              return `[Mermaid Diagram${ctx.diagramTitle ? ': ' + ctx.diagramTitle : ''}]\n\`\`\`mermaid\n${ctx.diagramCode}\n\`\`\``;
            case 'git-ref':
              return `[Git Ref: ${ctx.refValue}]`;
            case 'url':
              return `[URL: ${ctx.url}]`;
            case 'web-element': {
              const attrStr = Object.entries(ctx.attributes)
                .map(([k, v]) => `${k}="${v}"`)
                .join(' ');
              const lines = [
                `[Web Element: <${ctx.tagName}${attrStr ? ' ' + attrStr : ''}>]`,
                `CSS Path: ${ctx.path}`,
              ];
              if (ctx.sourceUrl) lines.push(`Source URL: ${ctx.sourceUrl}`);
              if (ctx.textContent) lines.push(`Text Content: ${ctx.textContent}`);
              if (ctx.outerHTML) lines.push(`Outer HTML:\n\`\`\`html\n${ctx.outerHTML}\n\`\`\``);
              return lines.join('\n');
            }
            default:
              return '';
          }
        }).filter(Boolean).join('\n');

        fullMessage = `${fullContextSection}\n\n${aiTrimmedMessage}`;
      }

      // Always pass imageContexts to the backend; the coordinator decides
      // whether to pre-analyse via a vision model or attach directly.
      const imageContextsForBackend = imageContexts.length > 0
        ? {
            imageContexts: imageContexts.map(ctx => ({
              id: ctx.id,
              image_path: ctx.isLocal ? ctx.imagePath : undefined,
              data_url: undefined,
              mime_type: ctx.mimeType,
              metadata: {
                name: ctx.imageName,
                width: ctx.width,
                height: ctx.height,
                file_size: ctx.fileSize,
                source: ctx.source,
              },
            })),
            imageDisplayData: imageContexts.map(ctx => ({
              id: ctx.id,
              name: ctx.imageName || 'Image',
              dataUrl: ctx.dataUrl,
              imagePath: ctx.isLocal ? ctx.imagePath : undefined,
              mimeType: ctx.mimeType,
            })),
          }
        : undefined;

      await flowChatManager.sendMessage(
        fullMessage,
        sessionId || undefined,
        displayMessage,
        currentAgentType || 'agentic',
        undefined,
        imageContextsForBackend
      );

      onClearContexts();

      onExitTemplateMode?.();

      onSuccess?.(trimmedMessage);
      log.info('Message sent successfully', {
        sessionId,
        agentType: currentAgentType || 'agentic',
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
