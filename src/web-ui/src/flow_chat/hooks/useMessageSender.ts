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

type ImageInputStrategy = 'vision-preanalysis' | 'direct-attach';

interface StrategyDecision {
  strategy: ImageInputStrategy;
  modelId: string | null;
  supportsImageUnderstanding: boolean;
  reason: string;
}

interface ImageAnalysisResult {
  image_id: string;
  summary: string;
  detailed_description: string;
  detected_elements: string[];
  confidence: number;
  analysis_time_ms: number;
}

const ENABLE_DIRECT_ATTACH_WHEN_SUPPORTED = true;

async function resolveSessionModelId(
  flowChatManager: FlowChatManager,
  sessionId: string | undefined,
  agentType?: string
): Promise<string | null> {
  const state = flowChatManager.getFlowChatState();
  const session = sessionId ? state.sessions.get(sessionId) : undefined;
  const configuredModel = session?.config?.modelName || null;
  const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
  const defaultModels = await configManager.getConfig<Record<string, string>>('ai.default_models') || {};
  const agentModels = await configManager.getConfig<Record<string, string>>('ai.agent_models') || {};

  const resolveAlias = (modelId: string | null): string | null => {
    if (!modelId) return null;
    if (modelId === 'primary') {
      return defaultModels.primary || null;
    }
    if (modelId === 'fast') {
      return defaultModels.fast || defaultModels.primary || null;
    }
    if (modelId === 'default') {
      return defaultModels.primary || null;
    }
    return modelId;
  };

  const effectiveAgentType = (agentType || session?.mode || 'agentic').trim();
  const configuredFromAgentModels = resolveAlias(
    effectiveAgentType ? (agentModels[effectiveAgentType] ?? null) : null
  );
  if (configuredFromAgentModels) {
    return configuredFromAgentModels;
  }

  // Backward-compatibility fallback for historical sessions.
  const resolvedConfigured = resolveAlias(configuredModel);
  if (resolvedConfigured) {
    return resolvedConfigured;
  }

  const { getDefaultPrimaryModel } = await import('@/infrastructure/config/utils/modelConfigHelpers');
  return getDefaultPrimaryModel();
}

async function modelSupportsImageUnderstanding(modelId: string | null): Promise<boolean> {
  if (!modelId) return false;

  const { configManager } = await import('@/infrastructure/config/services/ConfigManager');
  const allModels = await configManager.getConfig<any[]>('ai.models') || [];
  const model = allModels.find(
    m => m.id === modelId || m.name === modelId || m.model_name === modelId
  );
  if (!model || model.enabled === false) return false;

  const capabilities = Array.isArray(model?.capabilities) ? model.capabilities : [];
  const category = typeof model?.category === 'string' ? model.category : '';
  return capabilities.includes('image_understanding') || category === 'multimodal';
}

async function chooseImageInputStrategy(
  flowChatManager: FlowChatManager,
  sessionId: string | undefined,
  agentType?: string
): Promise<StrategyDecision> {
  const modelId = await resolveSessionModelId(flowChatManager, sessionId, agentType);
  const supportsImageUnderstanding = await modelSupportsImageUnderstanding(modelId);

  if (supportsImageUnderstanding && ENABLE_DIRECT_ATTACH_WHEN_SUPPORTED) {
    return {
      strategy: 'direct-attach',
      modelId,
      supportsImageUnderstanding,
      reason: 'model_supports_image_understanding',
    };
  }

  return {
    strategy: 'vision-preanalysis',
    modelId,
    supportsImageUnderstanding,
    reason: supportsImageUnderstanding
      ? 'direct_attach_disabled_by_feature_flag'
      : 'primary_model_is_text_only',
  };
}

async function analyzeImagesBeforeSend(
  imageContexts: ImageContext[],
  sessionId: string,
  userMessage: string
): Promise<ImageAnalysisResult[]> {
  if (imageContexts.length === 0) return [];

  const { imageAnalysisAPI } = await import('@/infrastructure/api/service-api/ImageAnalysisAPI');
  return imageAnalysisAPI.analyzeImages({
    session_id: sessionId,
    user_message: userMessage,
    images: imageContexts.map(ctx => ({
      id: ctx.id,
      image_path: ctx.isLocal ? ctx.imagePath : undefined,
      data_url: !ctx.isLocal ? ctx.dataUrl : undefined,
      mime_type: ctx.mimeType,
      metadata: {
        name: ctx.imageName,
        width: ctx.width,
        height: ctx.height,
        file_size: ctx.fileSize,
        source: ctx.source,
      },
    })),
  });
}

function formatImageContextLine(
  ctx: ImageContext,
  analysis?: ImageAnalysisResult,
  strategy?: ImageInputStrategy
): string {
  const imgName = ctx.imageName || 'Untitled image';
  const imgSize = ctx.fileSize ? ` (${(ctx.fileSize / 1024).toFixed(1)}KB)` : '';
  const sourceLine = ctx.isLocal
    ? `Path: ${ctx.imagePath}`
    : `Image ID: ${ctx.id}`;

  if (strategy === 'direct-attach') {
    return `[Image: ${imgName}${imgSize}]\n${sourceLine}\nAttached as multimodal image input.`;
  }

  if (!analysis) {
    return `[Image: ${imgName}${imgSize}]\n${sourceLine}\nTip: You can use the view_image tool (${ctx.isLocal ? 'image_path' : 'image_id'}).`;
  }

  const topElements = (analysis.detected_elements || []).slice(0, 5).join(', ');
  const keyElementsLine = topElements ? `\nPre-analysis key elements: ${topElements}` : '';

  return `[Image: ${imgName}${imgSize}]\n${sourceLine}\nPre-analysis summary: ${analysis.summary}${keyElementsLine}`;
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

      let strategyDecision: StrategyDecision = {
        strategy: 'vision-preanalysis',
        modelId: null,
        supportsImageUnderstanding: false,
        reason: 'fallback_default_preanalysis',
      };
      try {
        strategyDecision = await chooseImageInputStrategy(
          flowChatManager,
          sessionId,
          currentAgentType || undefined
        );
      } catch (error) {
        log.warn('Failed to resolve image input strategy, using pre-analysis fallback', {
          sessionId,
          error: (error as Error)?.message ?? 'unknown',
        });
      }

      log.debug('Image input strategy selected', {
        sessionId,
        strategy: strategyDecision.strategy,
        modelId: strategyDecision.modelId,
        supportsImageUnderstanding: strategyDecision.supportsImageUnderstanding,
        reason: strategyDecision.reason,
      });

      let imageAnalyses: ImageAnalysisResult[] = [];
      if (imageContexts.length > 0) {
        if (strategyDecision.strategy === 'vision-preanalysis') {
          try {
            imageAnalyses = await analyzeImagesBeforeSend(imageContexts, sessionId!, trimmedMessage);
            log.debug('Image pre-analysis completed', {
              sessionId,
              imageCount: imageContexts.length,
              analysisCount: imageAnalyses.length,
            });
          } catch (error) {
            log.warn('Image pre-analysis failed, continuing with context hints only', {
              sessionId,
              imageCount: imageContexts.length,
              error: (error as Error)?.message ?? 'unknown',
            });
          }
        }
      }

      let fullMessage = trimmedMessage;
      const displayMessage = trimmedMessage;

      if (contexts.length > 0) {
        const analysisByImageId = new Map(imageAnalyses.map(result => [result.image_id, result]));

        const fullContextSection = contexts.map(ctx => {
          switch (ctx.type) {
            case 'file':
              return `[File: ${ctx.relativePath || ctx.filePath}]`;
            case 'directory':
              return `[Directory: ${ctx.directoryPath}]`;
            case 'code-snippet':
              return `[Code Snippet: ${ctx.filePath}:${ctx.startLine}-${ctx.endLine}]`;
            case 'image': {
              return formatImageContextLine(ctx, analysisByImageId.get(ctx.id), strategyDecision.strategy);
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
        currentAgentType || 'agentic',
        undefined,
        strategyDecision.strategy === 'direct-attach'
          ? {
              imageContexts: imageContexts.map(ctx => ({
                id: ctx.id,
                image_path: ctx.isLocal ? ctx.imagePath : undefined,
                // Clipboard images are uploaded first and referenced by image_id only
                // to avoid sending large base64 payloads in the turn request.
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
            }
          : undefined
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
  }, [currentSessionId, contexts, onClearContexts, onSuccess, onExitTemplateMode, currentAgentType]);

  return {
    sendMessage,
    isSending: false,
  };
}
