/**
 * Agentic event listener
 * Listens to backend agentic:// events and dispatches them to the frontend
 * 
 * Architecture:
 * - Uses unified agentAPI (based on ApiClient) for event listening
 * - ApiClient internally uses TransportAdapter, supporting multiple platforms
 */

import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import type {
  TextChunkEvent,
  ToolEvent,
  AgenticEvent,
  SessionTitleGeneratedEvent,
  SessionModelAutoMigratedEvent,
  ImageAnalysisEvent,
  SessionBackgroundActivityEvent,
} from '@/infrastructure/api/service-api/AgentAPI';
import { createLogger } from '@/shared/utils/logger';

type UnlistenFn = () => void;

const logger = createLogger('AgenticEventListener');

export interface AgenticEventCallbacks {
  onSessionCreated?: (event: AgenticEvent) => void;
  onSessionDeleted?: (event: AgenticEvent) => void;
  onSessionStateChanged?: (event: AgenticEvent) => void;
  onImageAnalysisStarted?: (event: ImageAnalysisEvent) => void;
  onImageAnalysisCompleted?: (event: ImageAnalysisEvent) => void;
  onDialogTurnStarted?: (event: AgenticEvent) => void;
  onModelRoundStarted?: (event: AgenticEvent) => void;
  onTextChunk?: (event: TextChunkEvent) => void;
  onToolEvent?: (event: ToolEvent) => void;
  onDialogTurnCompleted?: (event: AgenticEvent) => void;
  onDialogTurnFailed?: (event: AgenticEvent) => void;
  onDialogTurnCancelled?: (event: AgenticEvent) => void;
  onTokenUsageUpdated?: (event: AgenticEvent) => void;
  onContextCompressionStarted?: (event: AgenticEvent) => void;
  onContextCompressionCompleted?: (event: AgenticEvent) => void;
  onContextCompressionFailed?: (event: AgenticEvent) => void;
  onSessionTitleGenerated?: (event: SessionTitleGeneratedEvent) => void;
  onSessionModelAutoMigrated?: (event: SessionModelAutoMigratedEvent) => void;
  onSessionBackgroundActivityUpdated?: (event: SessionBackgroundActivityEvent) => void;
}

export class AgenticEventListener {
  private unlistenFunctions: UnlistenFn[] = [];
  private isListening = false;

  async startListening(callbacks: AgenticEventCallbacks): Promise<void> {
    if (this.isListening) {
      logger.warn('Event listener already running');
      return;
    }

    logger.info('Starting Agentic event listener');

    try {
      if (callbacks.onSessionCreated) {
        const unlisten = agentAPI.onSessionCreated((event) => {
          logger.debug('Session created:', event);
          callbacks.onSessionCreated?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onSessionDeleted) {
        const unlisten = agentAPI.onSessionDeleted((event) => {
          logger.debug('Session deleted:', event);
          callbacks.onSessionDeleted?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onSessionStateChanged) {
        const unlisten = agentAPI.onSessionStateChanged((event) => {
          logger.debug('Session state changed:', event);
          callbacks.onSessionStateChanged?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onImageAnalysisStarted) {
        const unlisten = agentAPI.onImageAnalysisStarted((event) => {
          logger.debug('Image analysis started:', event);
          callbacks.onImageAnalysisStarted?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onImageAnalysisCompleted) {
        const unlisten = agentAPI.onImageAnalysisCompleted((event) => {
          logger.debug('Image analysis completed:', event);
          callbacks.onImageAnalysisCompleted?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onDialogTurnStarted) {
        const unlisten = agentAPI.onDialogTurnStarted((event) => {
          logger.debug('Dialog turn started:', event);
          callbacks.onDialogTurnStarted?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onModelRoundStarted) {
        const unlisten = agentAPI.onModelRoundStarted((event) => {
          logger.debug('Model round started:', event);
          callbacks.onModelRoundStarted?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onTextChunk) {
        const unlisten = agentAPI.onTextChunk((event) => {
          callbacks.onTextChunk?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onToolEvent) {
        const unlisten = agentAPI.onToolEvent((event) => {
          callbacks.onToolEvent?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onDialogTurnCompleted) {
        const unlisten = agentAPI.onDialogTurnCompleted((event) => {
          logger.debug('Dialog turn completed:', event);
          callbacks.onDialogTurnCompleted?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onDialogTurnFailed) {
        const unlisten = agentAPI.onDialogTurnFailed((event) => {
          logger.error('Dialog turn failed:', event);
          callbacks.onDialogTurnFailed?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onDialogTurnCancelled) {
        const unlisten = agentAPI.onDialogTurnCancelled((event) => {
          logger.debug('Dialog turn cancelled:', event);
          callbacks.onDialogTurnCancelled?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onTokenUsageUpdated) {
        const unlisten = agentAPI.onTokenUsageUpdated((event) => {
          logger.debug('Token usage updated:', event);
          callbacks.onTokenUsageUpdated?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onContextCompressionStarted) {
        const unlisten = agentAPI.onContextCompressionStarted((event) => {
          logger.debug('Context compression started:', event);
          callbacks.onContextCompressionStarted?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onContextCompressionCompleted) {
        const unlisten = agentAPI.onContextCompressionCompleted((event) => {
          logger.debug('Context compression completed:', event);
          callbacks.onContextCompressionCompleted?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onContextCompressionFailed) {
        const unlisten = agentAPI.onContextCompressionFailed((event) => {
          logger.error('Context compression failed:', event);
          callbacks.onContextCompressionFailed?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onSessionTitleGenerated) {
        const unlisten = agentAPI.onSessionTitleGenerated((event) => {
          logger.debug('Session title generated:', event);
          callbacks.onSessionTitleGenerated?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onSessionModelAutoMigrated) {
        const unlisten = agentAPI.onSessionModelAutoMigrated((event) => {
          logger.info('Session model auto-migrated:', event);
          callbacks.onSessionModelAutoMigrated?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      if (callbacks.onSessionBackgroundActivityUpdated) {
        const unlisten = agentAPI.onSessionBackgroundActivityUpdated((event) => {
          logger.debug('Session background activity updated:', event);
          callbacks.onSessionBackgroundActivityUpdated?.(event);
        });
        this.unlistenFunctions.push(unlisten);
      }

      this.isListening = true;
      logger.info(`Registered ${this.unlistenFunctions.length} event listeners`);
    } catch (error) {
      logger.error('Failed to register event listeners:', error);
      await this.stopListening();
      throw error;
    }
  }

  async stopListening(): Promise<void> {
    if (!this.isListening) {
      return;
    }

    logger.info('Stopping Agentic event listener');

    for (const unlisten of this.unlistenFunctions) {
      try {
        unlisten();
      } catch (error) {
        logger.error('Failed to unlisten:', error);
      }
    }

    this.unlistenFunctions = [];
    this.isListening = false;
    logger.info('Stopped all event listeners');
  }

  getIsListening(): boolean {
    return this.isListening;
  }
}

export const agenticEventListener = new AgenticEventListener();

