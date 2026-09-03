import { AssistantEntry, AssistantListResponse, ChatMessageItemResponse, ChatMessageResponse, CommandStatusResponse, CreateSessionOptions, CreateSessionResponse, FileInfo, FileInfoResponse, InitialSyncResult, ModelCatalogResponse, PermissionModeResponse, PollSessionResponse, PollSessionResult, ReadFileChunkResponse, ReadFileChunkResult, ReadFileResult, RecentWorkspaceEntry, RecentWorkspaceListResponse, RemoteCommand, RemoteDescriptor, RemoteImageContext, RemoteModelCatalog, RemotePermissionMode, RemoteQuestionAnswerPayload, RemoteSession, SendMessageResponse, SessionListResponse, SessionListResult, SessionMessagesResponse, SessionMessagesResult, SessionSummary, SetAssistantResponse, SetSessionModelResponse, SetWorkspaceResponse, SteerTurnResponse, SteerTurnResult, WorkspaceInfo, WorkspaceInfoResponse } from '../model/RemoteModels';
import { Encoding } from './Encoding';
import { PairIdentity, PeerDeviceProvisionOutcome, RelayHttpClient } from './RelayHttpClient';
import { CloudAccountClient, CloudAccountRequestError, CloudAccountSession } from './CloudAccountClient';
import { RemoteI18n } from '../i18n/RemoteI18n';
import {
  AccountDeviceCommandTransport,
  INTERACTIVE_DIRECTORY_TIMEOUT_MS,
  RemoteCommandTransport,
  RoomRemoteCommandTransport
} from './RemoteCommandTransport';
import { RemoteCommandFactory } from './RemoteCommandFactory';
import { RemoteCrypto } from './RemoteCrypto';
import { RemoteChatCommandClient } from './RemoteChatCommandController';
import { RemoteFileDownloadClient } from './RemoteFileDownloadController';
import { RemoteLogger } from './RemoteLogger';
import { RemoteModelClient } from './RemoteModelController';
import { RemoteResponseMapper } from './RemoteResponseMapper';
import { stampRemoteSessionsWorkspace } from './RemoteSessionIdentity';
import { TranscriptIntegrityPolicy } from './TranscriptIntegrityPolicy';
import { RemoteSessionClient } from './RemoteSessionController';
import { RemoteToolActionClient } from './RemoteToolActionController';

export interface DelegatedAccountSession {
  relayUrl: string;
  session: CloudAccountSession;
}

/** A credential minted for a peer device, paired with the relay it belongs to. */
export interface ProvisionedPeerDevice {
  relayUrl: string;
  token: string;
  userId: string;
  masterKeyBase64: string;
  deviceId: string;
}

/**
 * How long the first command of an account-device connect may take.
 *
 * A desktop that is actually running answers `get_workspace_info` from memory,
 * so anything past a few seconds means it is not there. The relay waits 130 s
 * before giving up on it, and the phone tears the previous connection down
 * before dialling — so without a deadline of our own, tapping a session on a
 * desktop that has gone away costs two minutes of a blank screen. Only the
 * handshake is bounded this tightly; once the desktop has answered once, the
 * longer transport default applies to everything after it.
 */
const ACCOUNT_HANDSHAKE_TIMEOUT_MS: number = 15000;

export class RemoteSessionManager implements RemoteChatCommandClient, RemoteFileDownloadClient, RemoteModelClient, RemoteSessionClient, RemoteToolActionClient {
  private readonly roomClient: RelayHttpClient = new RelayHttpClient();
  private transport?: RemoteCommandTransport;
  private transportGeneration: number = 0;
  private crypto?: RemoteCrypto;
  private workspace?: WorkspaceInfo;
  private roomRelayUrl: string = '';

  reset(): void {
    this.transportGeneration += 1;
    this.crypto = undefined;
    this.workspace = undefined;
    this.roomRelayUrl = '';
    this.transport?.reset();
    this.transport = undefined;
    this.roomClient.reset();
  }

  async connect(descriptor: RemoteDescriptor, identity: PairIdentity, deviceId: string): Promise<InitialSyncResult> {
    this.transportGeneration += 1;
    this.crypto = new RemoteCrypto();
    this.roomRelayUrl = descriptor.relayUrl;
    this.roomClient.bind(descriptor, this.crypto);
    this.transport?.reset();
    this.transport = new RoomRemoteCommandTransport(this.roomClient);
    const initialSync = await this.roomClient.pair(deviceId, identity);
    // Account inheritance is optional for room pairing. A desktop without an
    // account, or an older desktop build, must not make normal QR control fail.
    await this.roomClient.requestDelegatedIdentity();
    this.workspace = RemoteResponseMapper.workspaceFromInitialSync(initialSync);
    return {
      workspace: this.workspace,
      sessions: RemoteResponseMapper.sessions(initialSync.sessions || []),
      hasMoreSessions: initialSync.has_more_sessions || false,
      authenticatedUserId: initialSync.authenticated_user_id || '',
      capabilities: initialSync.capabilities || []
    };
  }

  delegatedAccountSession(): DelegatedAccountSession | undefined {
    if (!this.roomClient.hasDelegatedIdentity() || this.roomClient.delegatedUserId.trim().length === 0 ||
      this.roomRelayUrl.trim().length === 0) {
      return undefined;
    }
    return {
      relayUrl: this.roomRelayUrl,
      session: {
        token: this.roomClient.delegatedToken,
        userId: this.roomClient.delegatedUserId,
        masterKey: Encoding.base64ToBytes(this.roomClient.delegatedMasterKey)
      }
    };
  }

  /**
   * True when a QR-paired room channel is live. Provisioning a peer device is
   * only answerable there, so the caller uses this to decide whether the
   * feature is offerable at all rather than letting the user find out after a
   * 45-second wait.
   */
  hasRoomChannel(): boolean {
    return this.roomRelayUrl.trim().length > 0;
  }

  /**
   * Relay a watch's provisioning request to the paired desktop. Goes straight
   * to the room client rather than through `send()`, because the account-device
   * transport reaches a different desktop handler that cannot mint credentials.
   */
  async provisionPeerDevice(
    deviceId: string,
    deviceName: string,
    requestId: string
  ): Promise<PeerDeviceProvisionOutcome> {
    if (!this.hasRoomChannel()) {
      throw new Error('Remote room channel is not connected.');
    }
    return this.roomClient.provisionPeerDevice(deviceId, deviceName, requestId);
  }

  /** Relay the provisioned credential belongs to — the one this room lives on. */
  roomRelayEndpoint(): string {
    return this.roomRelayUrl;
  }

  async connectAccountDevice(
    accountClient: CloudAccountClient,
    relayUrl: string,
    session: CloudAccountSession,
    targetDeviceId: string
  ): Promise<InitialSyncResult> {
    this.transportGeneration += 1;
    this.transport?.reset();
    this.transport = new AccountDeviceCommandTransport(accountClient, relayUrl, session, targetDeviceId);
    let workspaceResponse: WorkspaceInfoResponse;
    try {
      workspaceResponse = await this.send<WorkspaceInfoResponse>(
        RemoteCommandFactory.getWorkspaceInfo(),
        ACCOUNT_HANDSHAKE_TIMEOUT_MS
      );
    } catch (err) {
      throw RemoteSessionManager.handshakeFailure(err);
    }
    this.workspace = RemoteResponseMapper.workspaceFromResponse(workspaceResponse);
    const sessions = await this.listSessions(50, 0, '', '');
    return {
      workspace: this.workspace,
      sessions: sessions.sessions,
      hasMoreSessions: sessions.hasMore,
      authenticatedUserId: session.userId,
      capabilities: workspaceResponse.capabilities || []
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

  async listSessionsForWorkspace(workspacePath: string, limit: number = 50): Promise<RemoteSession[]> {
    const path = workspacePath.trim();
    if (path.length === 0) {
      return [];
    }
    const response = await this.send<SessionListResponse>(
      RemoteCommandFactory.listSessions(path, limit, 0, ''),
      INTERACTIVE_DIRECTORY_TIMEOUT_MS
    );
    return stampRemoteSessionsWorkspace(RemoteResponseMapper.sessions(response.sessions || []), path);
  }

  async listSessions(limit: number, offset: number, query: string, agentType: string): Promise<SessionListResult> {
    const workspacePath = this.workspace?.path || '';
    const normalizedAgentType = RemoteSessionManager.normalizedSessionFilter(agentType);
    const trimmedQuery = query.trim();
    const pageSize = normalizedAgentType ? Math.max(100, limit) : Math.max(1, limit);
    const targetCount = offset + limit;
    const filteredSessions: RemoteSession[] = [];
    let pageOffset = 0;
    let hasMore = true;
    while (hasMore && filteredSessions.length < targetCount) {
      const command = RemoteCommandFactory.listSessions(workspacePath, pageSize, pageOffset, trimmedQuery);
      const response = await this.send<SessionListResponse>(command);
      const sessions = RemoteResponseMapper.allSessions(response.sessions || []);
      for (const session of sessions) {
        if (RemoteResponseMapper.isVisibleSession(session) &&
          (!normalizedAgentType || RemoteSessionManager.agentMatchesFilter(session.agentType, normalizedAgentType))) {
          filteredSessions.push(session);
        }
      }
      hasMore = response.has_more || false;
      pageOffset += sessions.length;
      if (sessions.length === 0) {
        break;
      }
    }
    return {
      sessions: filteredSessions.slice(offset, targetCount),
      hasMore: filteredSessions.length > targetCount || hasMore
    };
  }

  async createSession(
    options: CreateSessionOptions,
    onAllocated?: (session: SessionSummary) => Promise<void>
  ): Promise<SessionSummary> {
    const workspacePath = options.workspacePath?.trim() || this.workspace?.path || '';
    const command = RemoteCommandFactory.createSession(options, workspacePath);
    const response = await this.send<CreateSessionResponse>(command);
    const sessionId = response.session_id || response.id || '';
    const normalizedAgentType = options.agentType.toLowerCase();
    const fallbackTitle = normalizedAgentType === 'claw' || normalizedAgentType === 'assistant' || normalizedAgentType === 'chat' ?
      'Assistant Session' : normalizedAgentType === 'cowork' ? 'Cowork Session' : 'Code Session';
    const title = response.title || fallbackTitle;
    if (onAllocated) {
      await onAllocated({
        sessionId,
        title,
        workspacePath,
        agentType: options.agentType
      });
    }
    const modelId = options.modelId?.trim() || '';
    if (modelId.length > 0) {
      await this.setSessionModel(sessionId, modelId);
    }
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

  async sendMessage(sessionId: string, text: string, agentType?: string, turnId?: string): Promise<string> {
    const startedAt = Date.now();
    RemoteLogger.info(`send_message start session=${RemoteSessionManager.shortId(sessionId)} len=${text.length}`);
    const response = await this.send<SendMessageResponse>(
      RemoteCommandFactory.sendMessage(sessionId, text, agentType, undefined, turnId)
    );
    RemoteLogger.info(`send_message done session=${RemoteSessionManager.shortId(sessionId)} turn=${response.turn_id || ''} ms=${Date.now() - startedAt}`);
    return response.turn_id || '';
  }

  async sendMessageWithImages(
    sessionId: string,
    text: string,
    agentType: string,
    imageContexts: RemoteImageContext[],
    turnId?: string
  ): Promise<string> {
    const startedAt = Date.now();
    RemoteLogger.info(`send_message_images start session=${RemoteSessionManager.shortId(sessionId)} len=${text.length} images=${imageContexts.length}`);
    const response =
      await this.send<SendMessageResponse>(
        RemoteCommandFactory.sendMessage(sessionId, text, agentType, imageContexts, turnId)
      );
    RemoteLogger.info(`send_message_images done session=${RemoteSessionManager.shortId(sessionId)} turn=${response.turn_id || ''} ms=${Date.now() - startedAt}`);
    return response.turn_id || '';
  }

  async steerTurn(
    sessionId: string,
    turnId: string,
    text: string,
    displayContent: string,
    imageContexts: RemoteImageContext[]
  ): Promise<SteerTurnResult> {
    const startedAt = Date.now();
    RemoteLogger.info(
      `steer_turn start session=${RemoteSessionManager.shortId(sessionId)} ` +
        `turn=${RemoteSessionManager.shortId(turnId)} len=${text.length} images=${imageContexts.length}`
    );
    const response = await this.send<SteerTurnResponse>(
      RemoteCommandFactory.steerTurn(sessionId, turnId, text, displayContent, imageContexts)
    );
    const result: SteerTurnResult = {
      sessionId: response.session_id || sessionId,
      turnId: response.turn_id || turnId,
      steeringId: response.steering_id || ''
    };
    if (result.steeringId.length === 0) {
      throw new Error('Remote host accepted steering without a steering id.');
    }
    RemoteLogger.info(
      `steer_turn done session=${RemoteSessionManager.shortId(result.sessionId)} ` +
        `turn=${RemoteSessionManager.shortId(result.turnId)} ms=${Date.now() - startedAt}`
    );
    return result;
  }

  async buildPlan(sessionId: string, planFilePath: string, planName: string, agentType: string): Promise<string> {
    const startedAt = Date.now();
    RemoteLogger.info(
      `build_plan start session=${RemoteSessionManager.shortId(sessionId)} path_len=${planFilePath.length}`
    );
    const response = await this.send<SendMessageResponse>(
      RemoteCommandFactory.buildPlan(sessionId, planFilePath, planName, agentType)
    );
    RemoteLogger.info(
      `build_plan done session=${RemoteSessionManager.shortId(sessionId)} ` +
        `turn=${response.turn_id || ''} ms=${Date.now() - startedAt}`
    );
    return response.turn_id || '';
  }

  async getSessionMessages(sessionId: string): Promise<SessionMessagesResult> {
    const startedAt = Date.now();
    const response = await this.send<SessionMessagesResponse>(RemoteCommandFactory.getSessionMessages(sessionId));
    const rawMessages = response.messages || [];
    const messages = rawMessages.map((item: ChatMessageResponse) => RemoteResponseMapper.chatMessage(item));
    RemoteLogger.info(
      `get_session_messages done session=${RemoteSessionManager.shortId(sessionId)} ` +
        `raw=${rawMessages.length} ${TranscriptIntegrityPolicy.describeMessages(messages)} ` +
        `ms=${Date.now() - startedAt}`
    );
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
    const rawSnapshot = response.message_snapshot;
    const messageSnapshot = rawSnapshot ?
      rawSnapshot.map((item: ChatMessageResponse) => RemoteResponseMapper.chatMessage(item)) : undefined;
    const result: PollSessionResult = {
      version: response.version || sinceVersion,
      changed: response.changed,
      sessionState: response.session_state || '',
      title: response.title || '',
      newMessages,
      totalMessageCount: response.total_msg_count || knownMessageCount,
      messageSnapshot,
      activeTurn: response.active_turn
        ? RemoteResponseMapper.activeTurnToMessage(response.active_turn, response.version || sinceVersion)
        : undefined,
      modelCatalog: response.model_catalog
    };
    const rawActive = response.active_turn;
    RemoteLogger.info(`poll_session done session=${RemoteSessionManager.shortId(sessionId)} changed=${result.changed ? '1' : '0'} state=${result.sessionState} active=${rawActive ? rawActive.turn_id : ''} activeStatus=${rawActive ? rawActive.status : ''} rawText=${rawActive && rawActive.text ? rawActive.text.length : 0} rawThinking=${rawActive && rawActive.thinking ? rawActive.thinking.length : 0} rawItems=${rawActive && rawActive.items ? rawActive.items.length : 0} rawItemText=${RemoteSessionManager.itemsContentLength(rawActive ? rawActive.items : undefined)} projectedText=${result.activeTurn ? result.activeTurn.text.length : 0} new=${newMessages.length} snapshot=${messageSnapshot ? messageSnapshot.length : 0} ${newMessages.length > 0 ? TranscriptIntegrityPolicy.describeMessages(newMessages) + ' ' : ''}ms=${Date.now() - startedAt}`);
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

  async getPermissionMode(): Promise<RemotePermissionMode> {
    const response = await this.send<PermissionModeResponse>(RemoteCommandFactory.getPermissionMode());
    return response.mode || 'ask';
  }

  async setPermissionMode(mode: RemotePermissionMode): Promise<RemotePermissionMode> {
    const response = await this.send<PermissionModeResponse>(RemoteCommandFactory.setPermissionMode(mode));
    return response.mode || mode;
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

  async readFileChunk(
    path: string,
    offset: number,
    limit: number,
    sessionId?: string
  ): Promise<ReadFileChunkResult> {
    const response = await this.send<ReadFileChunkResponse>(
      RemoteCommandFactory.readFileChunk(path, offset, limit, sessionId)
    );
    return {
      name: response.name || RemoteResponseMapper.basename(path),
      contentBase64: response.chunk_base64 || '',
      offset: response.offset || offset,
      chunkSize: response.chunk_size || 0,
      totalSize: response.total_size || 0,
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
      const response = await this.readFileChunk(path, offset, chunkSize, sessionId);
      const readSize = response.chunkSize;
      chunks.push(response.contentBase64);
      fileName = response.name || fileName;
      mimeType = response.mimeType || mimeType;
      totalSize = response.totalSize || totalSize;
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

  /**
   * [readTimeoutMs] is left undefined so each transport applies its own
   * default: a room command and an account command do not travel the same
   * number of hops and never shared a sensible number.
   */
  private async send<T extends CommandStatusResponse>(command: RemoteCommand, readTimeoutMs?: number): Promise<T> {
    command._request_id = `req_${Date.now()}_${Encoding.randomId('harmony').slice(0, 20)}`;
    if (!this.transport) {
      throw new Error('Remote transport is not connected.');
    }
    const generation = this.transportGeneration;
    const transport = this.transport;
    const response = await transport.send<T>(command, readTimeoutMs);
    if (generation !== this.transportGeneration || transport !== this.transport) {
      throw new Error('Remote target changed while the request was in flight.');
    }
    return response;
  }

  /**
   * Keeps a handshake failure readable.
   *
   * Everything but a relay status here is the SDK's own errno text for "nobody
   * answered", which is both English and meaningless to the person holding the
   * phone. A status still carries something the caller acts on — 401 expires
   * the account session — so those pass through untouched.
   */
  private static handshakeFailure(err: Object): Error {
    if (err instanceof CloudAccountRequestError) {
      return err;
    }
    RemoteLogger.warn(`account device handshake failed: ${String(err)}`);
    return new Error(RemoteI18n.t('remote.settings.deviceUnavailable'));
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
