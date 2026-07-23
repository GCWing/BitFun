import { AssistantEntry, AssistantListResponse, ChatMessageItemResponse, ChatMessageResponse, CommandStatusResponse, CreateSessionOptions, CreateSessionResponse, FileInfo, FileInfoResponse, InitialSyncResult, ModelCatalogResponse, PollSessionResponse, PollSessionResult, ReadFileChunkResponse, ReadFileResult, RecentWorkspaceEntry, RecentWorkspaceListResponse, RemoteCommand, RemoteDescriptor, RemoteImageContext, RemoteModelCatalog, RemoteQuestionAnswerPayload, RemoteSession, SendMessageResponse, SessionListResponse, SessionListResult, SessionMessagesResponse, SessionMessagesResult, SessionSummary, SetAssistantResponse, SetSessionModelResponse, SetWorkspaceResponse, WorkspaceInfo, WorkspaceInfoResponse } from '../model/RemoteModels';
import { Encoding } from './Encoding';
import { RelayHttpClient } from './RelayHttpClient';
import { RemoteCommandFactory } from './RemoteCommandFactory';
import { RemoteCrypto } from './RemoteCrypto';
import { RemoteLogger } from './RemoteLogger';
import { RemoteResponseMapper } from './RemoteResponseMapper';

export class RemoteSessionManager {
  private readonly client: RelayHttpClient = new RelayHttpClient();
  private crypto?: RemoteCrypto;
  private workspace?: WorkspaceInfo;

  reset(): void {
    this.crypto = undefined;
    this.workspace = undefined;
    this.client.reset();
  }

  async connect(descriptor: RemoteDescriptor, userId: string, deviceId: string): Promise<InitialSyncResult> {
    this.crypto = new RemoteCrypto();
    this.client.bind(descriptor, this.crypto);
    const initialSync = await this.client.pair(deviceId, userId);
    this.workspace = RemoteResponseMapper.workspaceFromInitialSync(initialSync);
    return {
      workspace: this.workspace,
      sessions: RemoteResponseMapper.sessions(initialSync.sessions || []),
      hasMoreSessions: initialSync.has_more_sessions || false,
      authenticatedUserId: initialSync.authenticated_user_id || ''
    };
  }

  async getWorkspaceInfo(): Promise<WorkspaceInfo> {
    const response = await this.send<WorkspaceInfoResponse>(RemoteCommandFactory.getWorkspaceInfo());
    return RemoteResponseMapper.workspaceFromResponse(response);
  }

  async listRecentWorkspaces(): Promise<RecentWorkspaceEntry[]> {
    const response = await this.send<RecentWorkspaceListResponse>(RemoteCommandFactory.listRecentWorkspaces());
    return RemoteResponseMapper.recentWorkspaces(response.workspaces || []);
  }

  async setWorkspace(path: string): Promise<void> {
    const response = await this.send<SetWorkspaceResponse>(RemoteCommandFactory.setWorkspace(path));
    if (response.success === false) {
      throw new Error(response.error || 'Failed to set workspace.');
    }
    this.workspace = await this.getWorkspaceInfo();
  }

  async listAssistants(): Promise<AssistantEntry[]> {
    const response = await this.send<AssistantListResponse>(RemoteCommandFactory.listAssistants());
    return response.assistants || [];
  }

  async setAssistant(path: string): Promise<void> {
    const response = await this.send<SetAssistantResponse>(RemoteCommandFactory.setAssistant(path));
    if (response.success === false) {
      throw new Error(response.error || 'Failed to set assistant workspace.');
    }
    this.workspace = await this.getWorkspaceInfo();
  }

  async listRecentSessions(): Promise<RemoteSession[]> {
    const result = await this.listSessions(8, 0, '', '');
    return result.sessions;
  }

  async listSessions(limit: number, offset: number, query: string, agentType: string): Promise<SessionListResult> {
    const workspacePath = this.workspace?.path || '';
    const normalizedAgentType = RemoteSessionManager.normalizedSessionFilter(agentType);
    const trimmedQuery = query.trim();
    if (!normalizedAgentType) {
      const command = RemoteCommandFactory.listSessions(workspacePath, limit, offset, trimmedQuery);
      const response = await this.send<SessionListResponse>(command);
      return {
        sessions: RemoteResponseMapper.sessions(response.sessions || []),
        hasMore: response.has_more || false
      };
    }

    const pageSize = Math.max(100, limit);
    const targetCount = offset + limit;
    const filteredSessions: RemoteSession[] = [];
    let pageOffset = 0;
    let hasMore = true;
    while (hasMore && filteredSessions.length < targetCount) {
      const command = RemoteCommandFactory.listSessions(workspacePath, pageSize, pageOffset, trimmedQuery);
      const response = await this.send<SessionListResponse>(command);
      const sessions = RemoteResponseMapper.sessions(response.sessions || []);
      for (const session of sessions) {
        if (RemoteSessionManager.agentMatchesFilter(session.agentType, normalizedAgentType)) {
          filteredSessions.push(session);
        }
      }
      hasMore = response.has_more || false;
      pageOffset += pageSize;
      if (sessions.length === 0) {
        break;
      }
    }
    return {
      sessions: filteredSessions.slice(offset, targetCount),
      hasMore: filteredSessions.length > targetCount || hasMore
    };
  }

  async createSession(options: CreateSessionOptions): Promise<SessionSummary> {
    const workspacePath = this.workspace?.path || '';
    const command = RemoteCommandFactory.createSession(options, workspacePath);
    const response = await this.send<CreateSessionResponse>(command);
    const sessionId = response.session_id || response.id || '';
    const title = response.title || `${options.agentType === 'cowork' ? 'Cowork' : 'Code'} Session`;
    let initialTurnId = '';
    if (options.instruction.trim().length > 0) {
      initialTurnId = await this.sendMessage(sessionId, options.instruction.trim(), options.agentType);
    }
    return {
      sessionId,
      title,
      workspacePath,
      agentType: options.agentType,
      initialTurnId
    };
  }

  async sendMessage(sessionId: string, text: string, agentType?: string): Promise<string> {
    const startedAt = Date.now();
    RemoteLogger.info(`send_message start session=${RemoteSessionManager.shortId(sessionId)} len=${text.length}`);
    const response = await this.send<SendMessageResponse>(RemoteCommandFactory.sendMessage(sessionId, text, agentType));
    RemoteLogger.info(`send_message done session=${RemoteSessionManager.shortId(sessionId)} turn=${response.turn_id || ''} ms=${Date.now() - startedAt}`);
    return response.turn_id || '';
  }

  async sendMessageWithImages(sessionId: string, text: string, agentType: string, imageContexts: RemoteImageContext[]): Promise<string> {
    const startedAt = Date.now();
    RemoteLogger.info(`send_message_images start session=${RemoteSessionManager.shortId(sessionId)} len=${text.length} images=${imageContexts.length}`);
    const response =
      await this.send<SendMessageResponse>(RemoteCommandFactory.sendMessage(sessionId, text, agentType, imageContexts));
    RemoteLogger.info(`send_message_images done session=${RemoteSessionManager.shortId(sessionId)} turn=${response.turn_id || ''} ms=${Date.now() - startedAt}`);
    return response.turn_id || '';
  }

  async getSessionMessages(sessionId: string): Promise<SessionMessagesResult> {
    const response = await this.send<SessionMessagesResponse>(RemoteCommandFactory.getSessionMessages(sessionId));
    const rawMessages = response.messages || [];
    const messages = rawMessages.map((item: ChatMessageResponse) => RemoteResponseMapper.chatMessage(item));
    const result: SessionMessagesResult = {
      messages,
      hasMore: false
    };
    return result;
  }

  async getModelCatalog(sessionId?: string): Promise<RemoteModelCatalog> {
    const response = await this.send<ModelCatalogResponse>(RemoteCommandFactory.getModelCatalog(sessionId));
    return response.catalog || {
      version: 0,
      models: [],
      default_models: {}
    };
  }

  async setSessionModel(sessionId: string, modelId: string): Promise<string> {
    const response = await this.send<SetSessionModelResponse>(RemoteCommandFactory.setSessionModel(sessionId, modelId));
    return response.model_id || modelId;
  }

  async pollSession(
    sessionId: string,
    sinceVersion: number,
    knownMessageCount: number,
    knownModelCatalogVersion: number
  ): Promise<PollSessionResult> {
    const startedAt = Date.now();
    const command =
      RemoteCommandFactory.pollSession(sessionId, sinceVersion, knownMessageCount, knownModelCatalogVersion);
    const response = await this.send<PollSessionResponse>(command);
    const rawMessages = response.new_messages || [];
    const newMessages = rawMessages.map((item: ChatMessageResponse) => RemoteResponseMapper.chatMessage(item));
    const result: PollSessionResult = {
      version: response.version || sinceVersion,
      changed: response.changed,
      sessionState: response.session_state || '',
      title: response.title || '',
      newMessages,
      totalMessageCount: response.total_msg_count || knownMessageCount,
      activeTurn: response.active_turn
        ? RemoteResponseMapper.activeTurnToMessage(response.active_turn, response.version || sinceVersion)
        : undefined,
      modelCatalog: response.model_catalog
    };
    const rawActive = response.active_turn;
    RemoteLogger.info(`poll_session done session=${RemoteSessionManager.shortId(sessionId)} changed=${result.changed ? '1' : '0'} state=${result.sessionState} active=${rawActive ? rawActive.turn_id : ''} activeStatus=${rawActive ? rawActive.status : ''} rawText=${rawActive && rawActive.text ? rawActive.text.length : 0} rawThinking=${rawActive && rawActive.thinking ? rawActive.thinking.length : 0} rawItems=${rawActive && rawActive.items ? rawActive.items.length : 0} rawItemText=${RemoteSessionManager.itemsContentLength(rawActive ? rawActive.items : undefined)} projectedText=${result.activeTurn ? result.activeTurn.text.length : 0} new=${newMessages.length} ms=${Date.now() - startedAt}`);
    return result;
  }

  async cancelTask(sessionId: string, turnId?: string): Promise<void> {
    await this.send<CommandStatusResponse>(RemoteCommandFactory.cancelTask(sessionId, turnId));
  }

  async renameSession(sessionId: string, title: string): Promise<void> {
    await this.send<CommandStatusResponse>(RemoteCommandFactory.renameSession(sessionId, title));
  }

  async deleteSession(sessionId: string): Promise<void> {
    await this.send<CommandStatusResponse>(RemoteCommandFactory.deleteSession(sessionId));
  }

  async confirmTool(toolId: string): Promise<void> {
    await this.send<CommandStatusResponse>(RemoteCommandFactory.confirmTool(toolId));
  }

  async rejectTool(toolId: string, reason: string): Promise<void> {
    await this.send<CommandStatusResponse>(RemoteCommandFactory.rejectTool(toolId, reason));
  }

  async cancelTool(toolId: string, reason: string): Promise<void> {
    await this.send<CommandStatusResponse>(RemoteCommandFactory.cancelTool(toolId, reason));
  }

  async answerQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): Promise<void> {
    await this.send<CommandStatusResponse>(RemoteCommandFactory.answerQuestion(toolId, answers));
  }

  async getFileInfo(path: string, sessionId?: string): Promise<FileInfo> {
    const response = await this.send<FileInfoResponse>(RemoteCommandFactory.getFileInfo(path, sessionId));
    return {
      name: response.name || RemoteResponseMapper.basename(path),
      size: response.size || 0,
      mimeType: response.mime_type || 'application/octet-stream'
    };
  }

  async readFile(path: string, sessionId?: string, onProgress?: (downloaded: number, total: number) => void): Promise<ReadFileResult> {
    const chunkSize = 3 * 1024 * 1024;
    let offset = 0;
    let fileName = RemoteResponseMapper.basename(path);
    let mimeType = 'application/octet-stream';
    let totalSize = 0;
    const chunks: string[] = [];
    while (true) {
      const command = RemoteCommandFactory.readFileChunk(path, offset, chunkSize, sessionId);
      const response = await this.send<ReadFileChunkResponse>(command);
      const chunk = response.chunk_base64 || '';
      const readSize = response.chunk_size || 0;
      chunks.push(chunk);
      fileName = response.name || fileName;
      mimeType = response.mime_type || mimeType;
      totalSize = response.total_size || totalSize;
      offset += readSize;
      if (onProgress) {
        onProgress(offset, totalSize);
      }
      if (readSize === 0 || offset >= totalSize) {
        break;
      }
    }
    return {
      name: fileName,
      contentBase64: chunks.join(''),
      mimeType,
      size: totalSize
    };
  }

  async ping(): Promise<boolean> {
    await this.send<CommandStatusResponse>(RemoteCommandFactory.ping());
    return true;
  }

  private async send<T extends CommandStatusResponse>(command: RemoteCommand, readTimeoutMs: number = 30000): Promise<T> {
    command._request_id = `req_${Date.now()}_${Encoding.randomId('harmony').slice(0, 20)}`;
    return this.client.sendCommand<T>(command, readTimeoutMs);
  }

  private static normalizedSessionFilter(agentType: string): string {
    const normalized = (agentType || '').toLowerCase();
    if (normalized === 'code' || normalized === 'agentic') {
      return 'code';
    }
    if (normalized === 'cowork') {
      return 'cowork';
    }
    return '';
  }

  private static agentMatchesFilter(agentType: string, filter: string): boolean {
    const normalized = (agentType || '').toLowerCase();
    if (filter === 'code') {
      return normalized === 'code' || normalized === 'agentic';
    }
    if (filter === 'cowork') {
      return normalized === 'cowork';
    }
    return true;
  }

  private static shortId(value: string): string {
    if (value.length <= 8) {
      return value;
    }
    return value.slice(0, 4) + '...' + value.slice(value.length - 4);
  }

  private static itemsContentLength(items?: ChatMessageItemResponse[]): number {
    if (!items || items.length === 0) {
      return 0;
    }
    let total = 0;
    items.forEach((item: ChatMessageItemResponse) => {
      total += item.content ? item.content.length : 0;
      total += RemoteSessionManager.itemsContentLength(item.subItems);
    });
    return total;
  }

}
