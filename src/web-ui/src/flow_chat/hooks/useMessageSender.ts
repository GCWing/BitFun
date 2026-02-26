/**
 * Message sending hook.
 * Encapsulates session creation, image uploads, and message assembly.
 */

import { useCallback } from 'react';
import { FlowChatManager } from '../services/FlowChatManager';
import { notificationService } from '@/shared/notification-system';
import type { ContextItem, ImageContext } from '@/shared/types/context';
import { createLogger } from '@/shared/utils/logger';

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
  sendMessage: (message: string) => Promise<void>;
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

  const sendMessage = useCallback(async (message: string) => {
    if (!message.trim()) {
      return;
    }

    const trimmedMessage = message.trim();
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
        const { getDefaultPrimaryModel } = await import('@/infrastructure/config/utils/modelConfigHelpers');
        const modelId = await getDefaultPrimaryModel();
        
        sessionId = await flowChatManager.createChatSession({
          modelName: modelId || undefined
        }, currentAgentType || 'agentic');
        log.debug('Session created', { sessionId, modelId });
      } else {
        log.debug('Reusing existing session', { sessionId });
      }
      
      // Upload clipboard images to temporary backend storage first.
      const clipboardImages = contexts.filter(ctx => 
        ctx.type === 'image' && !ctx.isLocal && ctx.dataUrl
      ) as ImageContext[];
      
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
      
      // Build both backend and display versions of the message.
      let fullMessage = trimmedMessage;
      const displayMessage = trimmedMessage;
      
      if (contexts.length > 0) {
        // Full version includes absolute details for the backend.
        const fullContextSection = contexts.map(ctx => {
          switch (ctx.type) {
            case 'file':
              return `[File: ${ctx.relativePath || ctx.filePath}]`;
            case 'directory':
              return `[Directory: ${ctx.directoryPath}]`;
            case 'code-snippet':
              return `[Code Snippet: ${ctx.filePath}:${ctx.startLine}-${ctx.endLine}]`;
            case 'image': {
              const imgName = ctx.imageName || 'Untitled image';
              const imgSize = ctx.fileSize ? ` (${(ctx.fileSize / 1024).toFixed(1)}KB)` : '';
              
              // Distinguish local files and clipboard images.
              if (ctx.isLocal && ctx.imagePath) {
                return `[Image: ${imgName}${imgSize}]\n` +
                       `Path: ${ctx.imagePath}\n` +
                       `Tip: You can use the AnalyzeImage tool with the image_path parameter.`;
              } else {
                return `[Image: ${imgName}${imgSize} (from clipboard)]\n` +
                       `Image ID: ${ctx.id}\n` +
                       `Tip: You can use the AnalyzeImage tool.\n` +
                       `Parameter: image_id="${ctx.id}"`;
              }
            }
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
            default:
              return '';
          }
        }).filter(Boolean).join('\n');
        
        fullMessage = `${fullContextSection}\n\n${trimmedMessage}`;
      }
      
      await flowChatManager.sendMessage(
        fullMessage, 
        sessionId || undefined, 
        displayMessage,
        currentAgentType || 'agentic'
      );
      
      onClearContexts();
      
      onExitTemplateMode?.();
      
      onSuccess?.(trimmedMessage);
      log.info('Message sent successfully', {
        sessionId,
        agentType: currentAgentType || 'agentic',
        contextCount: contexts.length,
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
  }, [currentSessionId, contexts, onClearContexts, onSuccess, onExitTemplateMode]);

  return {
    sendMessage,
    isSending: false,
  };
}
