import { describe, it, expect } from '@ohos/hypium';
import { RemoteI18n } from '../main/ets/i18n/RemoteI18n';
import { ChatSessionController, ChatSessionSnapshot } from '../main/ets/services/ChatSessionController';
import { ChatComposerPolicy } from '../main/ets/services/ChatComposerPolicy';
import { ChatTimelineItem, ChatTimelineProjector } from '../main/ets/services/ChatTimelineProjector';
import { ChatTimelineStore } from '../main/ets/services/ChatTimelineStore';
import { Encoding } from '../main/ets/services/Encoding';
import {
  GeneralChatDraftStore,
  GeneralChatLocalStore,
  GeneralChatSendResult,
  GeneralChatStreamCallbacks,
  GeneralChatStreamObserver
} from '../main/ets/services/general-chat/GeneralChatPort';
import { GeneralChatApiClient } from '../main/ets/services/general-chat/GeneralChatApiClient';
import { GeneralChatBootstrapController } from '../main/ets/services/general-chat/GeneralChatBootstrapController';
import { GeneralChatCommandClient, GeneralChatCommandController } from '../main/ets/services/general-chat/GeneralChatCommandController';
import {
  GeneralChatConfigSnapshot,
  GeneralChatConfigStore,
  GeneralChatConfigValidator
} from '../main/ets/services/general-chat/GeneralChatConfigStore';
import { GeneralChatDraftController, GeneralChatDraftScheduler } from '../main/ets/services/general-chat/GeneralChatDraftController';
import { GeneralChatDraftLifecycleController } from '../main/ets/services/general-chat/GeneralChatDraftLifecycleController';
import { GeneralChatEventMapper } from '../main/ets/services/general-chat/GeneralChatEventMapper';
import { GeneralChatExportFormatter } from '../main/ets/services/general-chat/GeneralChatExportFormatter';
import {
  GeneralChatHttpHeaders,
  GeneralChatHttpMethod,
  GeneralChatHttpResponse,
  GeneralChatHttpTransport,
  GeneralChatTokenProvider
} from '../main/ets/services/general-chat/GeneralChatHttpTransport';
import { GeneralChatRepository } from '../main/ets/services/general-chat/GeneralChatRepository';
import {
  GeneralChatServiceState,
  GeneralChatServiceStatus
} from '../main/ets/services/general-chat/GeneralChatServiceState';
import { GeneralChatStreamCoordinator } from '../main/ets/services/general-chat/GeneralChatStreamCoordinator';
import { GeneralChatStreamLifecycleController } from '../main/ets/services/general-chat/GeneralChatStreamLifecycleController';
import {
  ModelProviderGeneralChatAdapter,
  ModelProviderMessage,
  ModelProviderProtocol
} from '../main/ets/services/general-chat/ModelProviderGeneralChatAdapter';
import { ModelProviderSseParser } from '../main/ets/services/general-chat/ModelProviderSseParser';
import { MockGeneralChatAdapter } from '../main/ets/services/general-chat/MockGeneralChatAdapter';
import { MarkdownParser } from '../main/ets/services/MarkdownParser';
import { RemoteCommandFactory } from '../main/ets/services/RemoteCommandFactory';
import { RemoteCrypto, RemoteCryptoCipher } from '../main/ets/services/RemoteCrypto';
import { RemoteDescriptorParser } from '../main/ets/services/RemoteDescriptorParser';
import { RemoteActivityLifecycleController } from '../main/ets/services/RemoteActivityLifecycleController';
import { RemoteChatCommandClient, RemoteChatCommandController } from '../main/ets/services/RemoteChatCommandController';
import { RemoteChatPollingLifecycleController } from '../main/ets/services/RemoteChatPollingLifecycleController';
import {
  RemoteFileDownloadClient,
  RemoteFileDownloadController,
  RemoteFileDownloadScheduler
} from '../main/ets/services/RemoteFileDownloadController';
import { RemoteHeartbeatController, RemoteHeartbeatScheduler } from '../main/ets/services/RemoteHeartbeatController';
import {
  RemoteModelClient,
  RemoteModelController,
  RemoteModelPreferenceStore
} from '../main/ets/services/RemoteModelController';
import { RemotePairingPolicy } from '../main/ets/services/RemotePairingPolicy';
import { RemoteResponseMapper } from '../main/ets/services/RemoteResponseMapper';
import { RemoteSessionClient, RemoteSessionController } from '../main/ets/services/RemoteSessionController';
import { RemoteSessionManager } from '../main/ets/services/RemoteSessionManager';
import { RemoteToolActionClient, RemoteToolActionController } from '../main/ets/services/RemoteToolActionController';
import { RemoteUiState } from '../main/ets/services/RemoteUiState';
import {
  VoiceInputLifecycleCallbacks,
  VoiceInputLifecycleController,
  VoiceInputRouteSnapshot
} from '../main/ets/services/VoiceInputLifecycleController';
import { VoiceInputCallbacks, VoiceInputService } from '../main/ets/services/VoiceInputService';
import { AppShellState } from '../main/ets/pages/state/AppShellState';
import { GeneralChatPageState } from '../main/ets/pages/state/GeneralChatPageState';
import { RemotePageState } from '../main/ets/pages/state/RemotePageState';
import {
  GENERAL_CHAT_COMPOSER_CAPABILITIES,
  REMOTE_CHAT_COMPOSER_CAPABILITIES
} from '../main/ets/pages/components/ChatComposerCapabilities';
import { ChatSurface } from '../main/ets/pages/components/ChatSurface';
import { ConversationViewContract } from '../main/ets/pages/components/ConversationViewContract';
import {
  AppNavigationBackAction,
  AppNavigationPathSpec,
  AppRoute,
  AppRouteContract
} from '../main/ets/pages/navigation/AppRouteContract';
import { TimeFormat } from '../main/ets/services/TimeFormat';
import { X25519 } from '../main/ets/services/X25519';
import {
  ChatMessage,
  FileInfo,
  ImageAttachment,
  PollSessionResult,
  RecentWorkspaceEntry,
  AssistantEntry,
  CreateSessionOptions,
  RemoteImageContext,
  RemoteQuestionAnswerPayload,
  RemoteModelCatalog,
  ReadFileResult,
  RemoteSession,
  SelectedImageAttachment,
  SessionListResult,
  SessionMessagesResult,
  SessionSummary
} from '../main/ets/model/RemoteModels';

interface ToolInputFixture {
  safe: boolean;
}

interface CryptoImageFixture {
  id: string;
  data_url: string;
  mime_type: string;
}

interface CryptoCommandFixture {
  cmd: string;
  session_id: string;
  content: string;
  agent_type: string;
  image_contexts: CryptoImageFixture[];
}

interface CryptoSimpleCommandFixture {
  cmd: string;
}

interface GeneralChatRecordedRequest {
  url: string;
  method: GeneralChatHttpMethod;
  authorization: string;
  body: string;
}

class FakeRemoteCryptoCipher implements RemoteCryptoCipher {
  async encrypt(plainBytes: Uint8Array, key: Uint8Array, nonce: Uint8Array): Promise<Uint8Array> {
    const out = new Uint8Array(plainBytes.length + 16);
    for (let index = 0; index < plainBytes.length; index++) {
      out[index] = plainBytes[index] ^ key[index % key.length] ^ nonce[index % nonce.length];
    }
    for (let index = 0; index < 16; index++) {
      out[plainBytes.length + index] = key[index] ^ nonce[index % nonce.length] ^ 165;
    }
    return out;
  }

  async decrypt(encryptedBytes: Uint8Array, key: Uint8Array, nonce: Uint8Array): Promise<Uint8Array> {
    if (encryptedBytes.length < 16) {
      throw new Error('Encrypted payload is too short.');
    }
    const plainLength = encryptedBytes.length - 16;
    const out = new Uint8Array(plainLength);
    for (let index = 0; index < plainLength; index++) {
      out[index] = encryptedBytes[index] ^ key[index % key.length] ^ nonce[index % nonce.length];
    }
    return out;
  }
}

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let index = 0; index < bytes.length; index++) {
    bytes[index] = Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

function bytesToHex(bytes: Uint8Array): string {
  const alphabet = '0123456789abcdef';
  let text = '';
  for (let index = 0; index < bytes.length; index++) {
    const value = bytes[index];
    text += alphabet.charAt((value >> 4) & 15) + alphabet.charAt(value & 15);
  }
  return text;
}

function fixedRandomBytes(size: number): Uint8Array {
  const bytes = new Uint8Array(size);
  for (let index = 0; index < size; index++) {
    bytes[index] = (index * 17 + 29) & 255;
  }
  return bytes;
}

function expectCommandJson(command: Object, expected: string): void {
  expect(JSON.stringify(command)).assertEqual(expected);
}

function connectedPeers(): RemoteCrypto[] {
  const cipher = new FakeRemoteCryptoCipher();
  const alicePrivate = hexToBytes('77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a');
  const bobPrivate = hexToBytes('5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb');
  const alice = new RemoteCrypto({
    cipher,
    keyPair: {
      privateKey: alicePrivate,
      publicKey: X25519.scalarMultBase(alicePrivate)
    },
    randomBytes: fixedRandomBytes
  });
  const bob = new RemoteCrypto({
    cipher,
    keyPair: {
      privateKey: bobPrivate,
      publicKey: X25519.scalarMultBase(bobPrivate)
    },
    randomBytes: fixedRandomBytes
  });
  alice.deriveSharedKey(bob.getPublicKeyBase64());
  bob.deriveSharedKey(alice.getPublicKeyBase64());
  return [alice, bob];
}

function chatMessage(id: string, role: string, text: string, status: string = 'completed'): ChatMessage {
  return {
    id,
    role,
    text,
    status,
    detail: ''
  };
}

function remoteSession(id: string, title: string, status: string = 'active'): RemoteSession {
  return {
    id,
    title,
    agentType: 'code',
    status,
    updatedAt: '2026-07-21T00:00:00.000Z',
    createdAt: '2026-07-21T00:00:00.000Z',
    messageCount: 1
  };
}

function selectedImage(id: string): SelectedImageAttachment {
  return {
    id,
    uri: `file:///tmp/${id}.png`,
    name: `${id}.png`,
    data_url: 'data:image/png;base64,AA==',
    mime_type: 'image/png',
    size: 2,
    original_size: 2,
    compressed: false
  };
}

function remoteModelCatalog(sessionModelId: string = ''): RemoteModelCatalog {
  return {
    version: 2,
    models: [{
      id: 'model-a',
      name: 'Model A',
      provider: 'local',
      base_url: 'https://model.example.com',
      model_name: 'model-a',
      enabled: true,
      capabilities: ['code']
    }, {
      id: 'model-b',
      name: 'Model B',
      provider: 'local',
      base_url: 'https://model.example.com',
      model_name: 'model-b',
      enabled: true,
      capabilities: ['code']
    }],
    default_models: { primary: 'model-a' },
    session_model_id: sessionModelId
  };
}

function activeChatMessage(turnId: string, text: string, status: string = 'active', renderVersion: number = 0): ChatMessage {
  const message = chatMessage(`active-${turnId}`, 'assistant', text, status);
  message.turnId = turnId;
  message.renderVersion = renderVersion;
  return message;
}

function chatMessageWithImage(id: string, role: string, text: string, imageName: string, imageDataUrl: string): ChatMessage {
  return {
    id,
    role,
    text,
    status: 'completed',
    detail: '',
    images: [imageAttachment(imageName, imageDataUrl)]
  };
}

function imageAttachment(name: string, dataUrl: string): ImageAttachment {
  return {
    name,
    data_url: dataUrl
  };
}

function pollResult(options: PollSessionResult): PollSessionResult {
  return options;
}

function delay(ms: number): Promise<void> {
  return new Promise<void>((resolve: () => void) => {
    setTimeout(() => {
      resolve();
    }, ms);
  });
}

class FakePollSessionManager extends RemoteSessionManager {
  results: PollSessionResult[] = [];

  async pollSession(
    _sessionId: string,
    _sinceVersion: number,
    _knownMessageCount: number,
    _knownModelCatalogVersion: number
  ): Promise<PollSessionResult> {
    const next = this.results.shift();
    if (!next) {
      throw new Error('Missing fake poll result.');
    }
    return next;
  }
}

class FakeGeneralChatLocalStore implements GeneralChatLocalStore, GeneralChatDraftStore {
  sessions: RemoteSession[] = [];
  messagesBySession: Map<string, ChatMessage[]> = new Map<string, ChatMessage[]>();
  drafts: Map<string, string> = new Map<string, string>();

  async init(_context: Context): Promise<void> {}

  async listSessions(): Promise<RemoteSession[]> {
    return this.sessions.slice();
  }

  async loadMessages(sessionId: string): Promise<ChatMessage[]> {
    return (this.messagesBySession.get(sessionId) || []).slice();
  }

  async upsertSession(session: RemoteSession): Promise<void> {
    this.sessions = [session].concat(this.sessions.filter((item: RemoteSession) => item.id !== session.id));
  }

  async replaceMessages(sessionId: string, messages: ChatMessage[]): Promise<void> {
    this.messagesBySession.set(sessionId, messages.slice());
  }

  async deleteSession(sessionId: string): Promise<void> {
    this.sessions = this.sessions.filter((item: RemoteSession) => item.id !== sessionId);
    this.messagesBySession.delete(sessionId);
    this.drafts.delete(sessionId);
  }

  async loadDraft(draftId: string): Promise<string> {
    return this.drafts.get(draftId) || '';
  }

  async draft(draftId: string): Promise<string> {
    return this.loadDraft(draftId);
  }

  async saveDraft(draftId: string, text: string): Promise<void> {
    this.drafts.set(draftId, text);
  }

  async deleteDraft(draftId: string): Promise<void> {
    this.drafts.delete(draftId);
  }

  async clearDraft(draftId: string): Promise<void> {
    await this.deleteDraft(draftId);
  }
}

class FailingGeneralChatAdapter extends MockGeneralChatAdapter {
  async sendMessage(
    _sessionId: string,
    text: string,
    _images: ImageAttachment[] = [],
    _history: ChatMessage[] = [],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult> {
    const userMessage = chatMessage('failed-user', 'user', text, 'pending');
    if (observer) {
      await observer.onUserMessage(userMessage);
    }
    throw new Error('Expected test failure.');
  }
}

class PartialFailingGeneralChatAdapter extends MockGeneralChatAdapter {
  async sendMessage(
    _sessionId: string,
    text: string,
    _images: ImageAttachment[] = [],
    _history: ChatMessage[] = [],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult> {
    const userMessage = chatMessage('partial-user', 'user', text, 'pending');
    if (observer) {
      await observer.onUserMessage(userMessage);
      observer.onDelta('partial');
    }
    throw new Error('Expected interrupted stream.');
  }
}

class FakeGeneralChatTokenProvider implements GeneralChatTokenProvider {
  token: string = 'test-token';

  async accessToken(): Promise<string> {
    return this.token;
  }
}

class FakeGeneralChatHttpTransport implements GeneralChatHttpTransport {
  responses: GeneralChatHttpResponse[] = [];
  requests: GeneralChatRecordedRequest[] = [];

  async request(
    url: string,
    method: GeneralChatHttpMethod,
    headers: GeneralChatHttpHeaders,
    body?: string
  ): Promise<GeneralChatHttpResponse> {
    this.requests.push({
      url,
      method,
      authorization: headers.authorization,
      body: body || ''
    });
    const response = this.responses.shift();
    if (!response) {
      throw new Error('Missing fake general chat HTTP response.');
    }
    return response;
  }
}

class FakeRemoteHeartbeatScheduler implements RemoteHeartbeatScheduler {
  callbacks: Map<number, () => void> = new Map<number, () => void>();
  intervals: Map<number, number> = new Map<number, number>();
  clearedIds: number[] = [];
  nextId: number = 1;

  setInterval(callback: () => void, intervalMs: number): number {
    const timerId = this.nextId;
    this.nextId += 1;
    this.callbacks.set(timerId, callback);
    this.intervals.set(timerId, intervalMs);
    return timerId;
  }

  clearInterval(timerId: number): void {
    this.clearedIds.push(timerId);
    this.callbacks.delete(timerId);
    this.intervals.delete(timerId);
  }

  tick(timerId: number): void {
    const callback = this.callbacks.get(timerId);
    if (callback) {
      callback();
    }
  }
}

class FakeGeneralChatDraftScheduler implements GeneralChatDraftScheduler {
  callbacks: Map<number, () => void> = new Map<number, () => void>();
  delays: Map<number, number> = new Map<number, number>();
  clearedIds: number[] = [];
  nextId: number = 1;

  setTimeout(callback: () => void, delayMs: number): number {
    const timerId = this.nextId;
    this.nextId += 1;
    this.callbacks.set(timerId, callback);
    this.delays.set(timerId, delayMs);
    return timerId;
  }

  clearTimeout(timerId: number): void {
    this.clearedIds.push(timerId);
    this.callbacks.delete(timerId);
    this.delays.delete(timerId);
  }

  tick(timerId: number): void {
    const callback = this.callbacks.get(timerId);
    if (callback) {
      callback();
      this.callbacks.delete(timerId);
      this.delays.delete(timerId);
    }
  }
}

class FakeVoiceInputLifecycleService extends VoiceInputService {
  startRequests: number = 0;
  stopRequests: number = 0;
  cancelRequests: number = 0;
  shouldFailStart: boolean = false;
  shouldFailStop: boolean = false;
  fakeListening: boolean = false;
  fakeCallbacks?: VoiceInputCallbacks;

  isListening(): boolean {
    return this.fakeListening;
  }

  async start(_context: Context, callbacks: VoiceInputCallbacks): Promise<void> {
    this.startRequests += 1;
    if (this.shouldFailStart) {
      throw new Error('Expected voice start failure.');
    }
    this.fakeCallbacks = callbacks;
    this.fakeListening = true;
    callbacks.onState?.('listening');
  }

  async stop(): Promise<void> {
    this.stopRequests += 1;
    if (this.shouldFailStop) {
      throw new Error('Expected voice stop failure.');
    }
    this.fakeListening = false;
    this.fakeCallbacks?.onState?.('idle');
  }

  async cancel(): Promise<void> {
    this.cancelRequests += 1;
    this.fakeListening = false;
    this.fakeCallbacks?.onState?.('idle');
  }

  emitText(text: string, isFinal: boolean): void {
    this.fakeCallbacks?.onText(text, isFinal);
  }

  emitError(message: string): void {
    this.fakeCallbacks?.onError?.(message);
  }
}

class FakeRemoteFileDownloadScheduler implements RemoteFileDownloadScheduler {
  callbacks: Map<number, () => void> = new Map<number, () => void>();
  delays: Map<number, number> = new Map<number, number>();
  clearedIds: number[] = [];
  nextId: number = 1;

  setTimeout(callback: () => void, delayMs: number): number {
    const timerId = this.nextId;
    this.nextId += 1;
    this.callbacks.set(timerId, callback);
    this.delays.set(timerId, delayMs);
    return timerId;
  }

  clearTimeout(timerId: number): void {
    this.clearedIds.push(timerId);
    this.callbacks.delete(timerId);
    this.delays.delete(timerId);
  }

  tick(timerId: number): void {
    const callback = this.callbacks.get(timerId);
    if (callback) {
      callback();
      this.callbacks.delete(timerId);
      this.delays.delete(timerId);
    }
  }
}

class FakeRemoteFileDownloadClient implements RemoteFileDownloadClient {
  infoRequests: string[] = [];
  readRequests: string[] = [];
  shouldFailRead: boolean = false;

  async getFileInfo(path: string, sessionId?: string): Promise<FileInfo> {
    this.infoRequests.push(`${path}|${sessionId || ''}`);
    return {
      name: 'README.md',
      size: 4096,
      mimeType: 'text/plain'
    };
  }

  async readFile(
    path: string,
    sessionId?: string,
    onProgress?: (downloaded: number, total: number) => void
  ): Promise<ReadFileResult> {
    this.readRequests.push(`${path}|${sessionId || ''}`);
    if (onProgress) {
      onProgress(2048, 4096);
    }
    if (this.shouldFailRead) {
      throw new Error('Expected download failure.');
    }
    return {
      name: 'README.md',
      contentBase64: 'cmVhZG1l',
      mimeType: 'text/plain',
      size: 4096
    };
  }
}

class RemoteDownloadStatusRecord {
  downloadingFilePath: string = '';
  downloadedFilePath: string = '';
  fileDownloadStatus: string = '';
}

class FakeRemoteToolActionClient implements RemoteToolActionClient {
  actions: string[] = [];
  shouldFail: boolean = false;

  async confirmTool(toolId: string, updatedInput?: Object): Promise<void> {
    this.actions.push(`confirm:${toolId}:${JSON.stringify(updatedInput || {})}`);
    this.failIfNeeded();
  }

  async rejectTool(toolId: string, reason: string): Promise<void> {
    this.actions.push(`reject:${toolId}:${reason}`);
    this.failIfNeeded();
  }

  async cancelTool(toolId: string, reason: string): Promise<void> {
    this.actions.push(`cancel:${toolId}:${reason}`);
    this.failIfNeeded();
  }

  async answerQuestion(toolId: string, answers: RemoteQuestionAnswerPayload): Promise<void> {
    this.actions.push(`answer:${toolId}:${JSON.stringify(answers)}`);
    this.failIfNeeded();
  }

  private failIfNeeded(): void {
    if (this.shouldFail) {
      throw new Error('Expected tool action failure.');
    }
  }
}

class FakeRemoteModelClient implements RemoteModelClient {
  catalog: RemoteModelCatalog = remoteModelCatalog();
  catalogRequests: string[] = [];
  modelSelections: string[] = [];
  selectedModelId: string = 'model-b';
  shouldFailCatalog: boolean = false;
  shouldFailSelect: boolean = false;

  async getModelCatalog(sessionId?: string): Promise<RemoteModelCatalog> {
    this.catalogRequests.push(sessionId || '');
    if (this.shouldFailCatalog) {
      throw new Error('Expected catalog failure.');
    }
    return this.catalog;
  }

  async setSessionModel(sessionId: string, modelId: string): Promise<string> {
    this.modelSelections.push(`${sessionId}:${modelId}`);
    if (this.shouldFailSelect) {
      throw new Error('Expected model selection failure.');
    }
    return this.selectedModelId;
  }
}

class FakeRemoteModelPreferenceStore implements RemoteModelPreferenceStore {
  savedModelIds: string[] = [];

  async saveLastModelId(modelId: string): Promise<void> {
    this.savedModelIds.push(modelId);
  }
}

class RemoteModelProjectionRecord {
  version: number = 0;
  selectedModelId: string = '';
  sessionModelId: string = '';
  modelCount: number = 0;
}

class FakeRemoteSessionClient implements RemoteSessionClient {
  listResults: SessionListResult[] = [];
  listRequests: string[] = [];
  createRequests: string[] = [];
  deleteRequests: string[] = [];
  createdSession: SessionSummary = {
    sessionId: 'created-session',
    title: 'Created Session',
    workspacePath: '/workspace',
    agentType: 'code'
  };
  shouldFailList: boolean = false;
  shouldFailCreate: boolean = false;
  shouldFailDelete: boolean = false;

  async listSessions(limit: number, offset: number, query: string, agentType: string): Promise<SessionListResult> {
    this.listRequests.push(`${limit}:${offset}:${query}:${agentType}`);
    if (this.shouldFailList) {
      throw new Error('Expected session list failure.');
    }
    const result = this.listResults.shift();
    if (!result) {
      throw new Error('Missing fake session list result.');
    }
    return result;
  }

  async createSession(options: CreateSessionOptions): Promise<SessionSummary> {
    this.createRequests.push(`${options.agentType}:${options.title}:${options.instruction}`);
    if (this.shouldFailCreate) {
      throw new Error('Expected session create failure.');
    }
    return this.createdSession;
  }

  async deleteSession(sessionId: string): Promise<void> {
    this.deleteRequests.push(sessionId);
    if (this.shouldFailDelete) {
      throw new Error('Expected session delete failure.');
    }
  }
}

class RemoteSessionProjectionRecord {
  sessions: RemoteSession[] = [];
  hasMore: boolean = false;
}

class FakeRemoteChatCommandClient implements RemoteChatCommandClient {
  messageResults: SessionMessagesResult[] = [];
  messageRequests: string[] = [];
  sendRequests: string[] = [];
  cancelRequests: string[] = [];
  renameRequests: string[] = [];
  turnId: string = 'turn-1';
  shouldFailMessages: boolean = false;
  shouldFailSend: boolean = false;
  shouldFailCancel: boolean = false;
  shouldFailRename: boolean = false;

  async getSessionMessages(sessionId: string): Promise<SessionMessagesResult> {
    this.messageRequests.push(sessionId);
    if (this.shouldFailMessages) {
      throw new Error('Expected message load failure.');
    }
    const result = this.messageResults.shift();
    if (!result) {
      throw new Error('Missing fake session messages result.');
    }
    return result;
  }

  async sendMessage(sessionId: string, text: string, agentType?: string): Promise<string> {
    this.sendRequests.push(`text:${sessionId}:${text}:${agentType || ''}`);
    if (this.shouldFailSend) {
      throw new Error('Expected send failure.');
    }
    return this.turnId;
  }

  async sendMessageWithImages(
    sessionId: string,
    text: string,
    agentType: string,
    imageContexts: RemoteImageContext[]
  ): Promise<string> {
    this.sendRequests.push(`images:${sessionId}:${text}:${agentType}:${imageContexts.length}`);
    if (this.shouldFailSend) {
      throw new Error('Expected send failure.');
    }
    return this.turnId;
  }

  async cancelTask(sessionId: string, turnId?: string): Promise<void> {
    this.cancelRequests.push(`${sessionId}:${turnId || ''}`);
    if (this.shouldFailCancel) {
      throw new Error('Expected cancel failure.');
    }
  }

  async renameSession(sessionId: string, title: string): Promise<void> {
    this.renameRequests.push(`${sessionId}:${title}`);
    if (this.shouldFailRename) {
      throw new Error('Expected rename failure.');
    }
  }
}

class RemoteChatMessagesProjectionRecord {
  messages: ChatMessage[] = [];
  hasMoreMessages: boolean = true;
  pollVersion: number = -1;
  knownMessageCount: number = -1;
}

class FakeGeneralChatCommandClient implements GeneralChatCommandClient {
  currentSessions: RemoteSession[] = [remoteSession('chat-1', 'Chat 1')];
  messagesBySession: Map<string, ChatMessage[]> = new Map<string, ChatMessage[]>();
  initRequests: number = 0;
  createRequests: string[] = [];
  deleteRequests: string[] = [];
  archiveRequests: string[] = [];
  renameRequests: string[] = [];
  removeFailedRequests: string[] = [];
  messageRequests: string[] = [];
  sendRequests: string[] = [];
  cancelCount: number = 0;
  recordedAssistantMessages: string[] = [];
  shouldFail: boolean = false;
  shouldFailSend: boolean = false;
  shouldFailRecordAssistant: boolean = false;

  async init(_context: Context): Promise<void> {
    this.initRequests += 1;
    this.failIfNeeded();
  }

  sessions(): RemoteSession[] {
    return this.currentSessions.slice();
  }

  async createSession(firstMessage: string): Promise<RemoteSession> {
    this.createRequests.push(firstMessage);
    this.failIfNeeded();
    const session = remoteSession('chat-created', firstMessage);
    session.agentType = 'chat';
    this.currentSessions = [session].concat(this.currentSessions);
    return session;
  }

  async messages(sessionId: string): Promise<ChatMessage[]> {
    this.messageRequests.push(sessionId);
    this.failIfNeeded();
    return (this.messagesBySession.get(sessionId) || []).slice();
  }

  async sendMessage(
    sessionId: string,
    text: string,
    images: ImageAttachment[],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult> {
    this.sendRequests.push(`${sessionId}:${text}:${images.length}`);
    if (this.shouldFailSend) {
      throw new Error('Expected general chat send failure.');
    }
    const userMessage = chatMessage('sent-user', 'user', text, 'completed');
    const assistantMessage = chatMessage('sent-assistant', 'assistant', `Echo: ${text}`, 'completed');
    if (observer) {
      await observer.onUserMessage(userMessage);
      observer.onDelta('Echo: ');
      observer.onDelta(text);
    }
    return {
      userMessage,
      assistantMessage,
      streamChunks: ['Echo: ', text],
      tools: []
    };
  }

  cancelCurrentRequest(): void {
    this.cancelCount += 1;
  }

  async recordAssistantMessage(sessionId: string, message: ChatMessage): Promise<void> {
    if (this.shouldFailRecordAssistant) {
      throw new Error('Expected general chat record failure.');
    }
    this.recordedAssistantMessages.push(`${sessionId}:${message.id}`);
  }

  async renameSession(sessionId: string, title: string): Promise<void> {
    this.renameRequests.push(`${sessionId}:${title}`);
    this.failIfNeeded();
    this.currentSessions = this.currentSessions.map((session: RemoteSession) => {
      if (session.id !== sessionId) {
        return session;
      }
      return {
        id: session.id,
        title,
        agentType: session.agentType,
        status: session.status,
        updatedAt: session.updatedAt,
        createdAt: session.createdAt,
        messageCount: session.messageCount,
        workspacePath: session.workspacePath,
        workspaceName: session.workspaceName
      };
    });
  }

  async archiveSession(sessionId: string, archived: boolean): Promise<void> {
    this.archiveRequests.push(`${sessionId}:${archived ? '1' : '0'}`);
    this.failIfNeeded();
  }

  async deleteSession(sessionId: string): Promise<void> {
    this.deleteRequests.push(sessionId);
    this.failIfNeeded();
    this.currentSessions = this.currentSessions.filter((session: RemoteSession) => session.id !== sessionId);
  }

  async removeFailedUserMessage(sessionId: string, text: string): Promise<void> {
    this.removeFailedRequests.push(`${sessionId}:${text}`);
    this.failIfNeeded();
  }

  private failIfNeeded(): void {
    if (this.shouldFail) {
      throw new Error('Expected general chat command failure.');
    }
  }
}

class FakeGeneralChatConfigStore extends GeneralChatConfigStore {
  initRequests: number = 0;
  snapshotResult: GeneralChatConfigSnapshot = {
    apiUrl: 'https://api.example.com',
    modelName: 'model-a',
    hasApiKey: true
  };
  shouldFailInit: boolean = false;

  async init(_context: Context): Promise<GeneralChatConfigSnapshot> {
    this.initRequests += 1;
    if (this.shouldFailInit) {
      throw new Error('Expected config init failure.');
    }
    return this.snapshotResult;
  }
}

export default function localUnitTest() {
  describe('RemoteDescriptorParser', () => {
    it('parses hash route URLs', 0, () => {
      const descriptor = RemoteDescriptorParser.parse('https://relay.example.com/r/mobile#/pair?room=room-a&pk=public-key');
      expect(descriptor.roomId).assertEqual('room-a');
      expect(descriptor.publicKey).assertEqual('public-key');
      expect(descriptor.relayUrl).assertEqual('https://relay.example.com');
    });

    it('normalizes relay websocket URLs', 0, () => {
      const descriptor = RemoteDescriptorParser.parse('https://app.example.com/#/pair?room=room-b&pk=key-b&relay=wss%3A%2F%2Frelay.example.com%2Fws');
      expect(descriptor.relayUrl).assertEqual('https://relay.example.com');
    });

    it('parses account pairing metadata', 0, () => {
      const descriptor = RemoteDescriptorParser.parse('https://relay.example.com/#/pair?room=room-account&pk=key-account&auth=account&user=alice');
      expect(descriptor.accountAuth).assertTrue();
      expect(descriptor.accountUsername).assertEqual('alice');
    });

    it('parses raw query strings', 0, () => {
      const descriptor = RemoteDescriptorParser.parse('room=room-c&pk=key%2Bc%3D&relay=http%3A%2F%2F127.0.0.1%3A30333');
      expect(descriptor.roomId).assertEqual('room-c');
      expect(descriptor.publicKey).assertEqual('key+c=');
      expect(descriptor.relayUrl).assertEqual('http://127.0.0.1:30333');
    });

    it('rejects URLs missing room', 0, () => {
      let didThrow = false;
      try {
        RemoteDescriptorParser.parse('https://relay.example.com/#/pair?pk=key-only');
      } catch (_err) {
        didThrow = true;
      }
      expect(didThrow).assertEqual(true);
    });

    it('rejects URLs missing pk', 0, () => {
      let didThrow = false;
      try {
        RemoteDescriptorParser.parse('https://relay.example.com/#/pair?room=room-only');
      } catch (_err) {
        didThrow = true;
      }
      expect(didThrow).assertEqual(true);
    });
  });

  describe('RemotePairingPolicy', () => {
    it('prefills account username from QR only over the default install id', 0, () => {
      const policy = new RemotePairingPolicy();
      const projection = policy.projection('https://relay.example.com/#/pair?room=room-a&pk=key-a&auth=account&user=alice');

      expect(projection.requiresAccountAuth).assertTrue();
      expect(projection.accountUsername).assertEqual('alice');
      expect(policy.userIdAfterProjection('harmony-device', 'harmony-device', projection)).assertEqual('alice');
      expect(policy.userIdAfterProjection('manual-user', 'harmony-device', projection)).assertEqual('manual-user');
    });

    it('blocks passwordless account reconnects and builds encrypted pairing identity', 0, () => {
      const policy = new RemotePairingPolicy();
      const descriptor = RemoteDescriptorParser.parse('https://relay.example.com/#/pair?room=room-a&pk=key-a&auth=account&user=alice');

      expect(policy.shouldAutoReconnect({
        autoReconnectAttempted: false,
        remoteUrl: 'https://relay.example.com/#/pair?room=room-a&pk=key-a&auth=account&user=alice',
        userId: 'alice',
        requiresAccountAuth: true
      })).assertFalse();

      const identity = policy.identityForConnect(descriptor, '', 'harmony-device', 'secret-password', false);
      expect(identity.userId).assertEqual('alice');
      expect(identity.password).assertEqual('secret-password');
    });

    it('keeps ordinary pairing passwordless and auto reconnectable', 0, () => {
      const policy = new RemotePairingPolicy();
      const descriptor = RemoteDescriptorParser.parse('https://relay.example.com/#/pair?room=room-a&pk=key-a');
      const identity = policy.identityForConnect(descriptor, '', 'harmony-device', '', true);

      expect(identity.userId).assertEqual('harmony-device');
      expect(identity.password || '').assertEqual('');
      expect(policy.shouldAutoReconnect({
        autoReconnectAttempted: false,
        remoteUrl: 'https://relay.example.com/#/pair?room=room-a&pk=key-a',
        userId: 'harmony-device',
        requiresAccountAuth: false
      })).assertTrue();
    });
  });

  describe('X25519', () => {
    const alicePrivate = '77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a';
    const alicePublic = '8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a';
    const bobPrivate = '5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb';
    const bobPublic = 'de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f';
    const sharedSecret = '4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742';
    const mobileWebPrivate = '000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f';
    const mobileWebPublic = '8f40c5adb68f25624ae5b214ea767a6ec94d829d3d7b5e1ad1ba6f3e2138285f';
    const desktopPrivate = 'f0e1d2c3b4a5968778695a4b3c2d1e0f00112233445566778899aabbccddeeff';
    const desktopPublic = 'ed99ba8df1a6a8c6396beafc3a42b8212fde389a7663217e170e82b68377e858';
    const mobileWebSharedSecret = '251b4e394babbef8dc30269e5a065946c08d89d6df010376549baa469c552f4b';

    it('matches RFC 7748 public key vectors', 0, () => {
      expect(bytesToHex(X25519.scalarMultBase(hexToBytes(alicePrivate)))).assertEqual(alicePublic);
      expect(bytesToHex(X25519.scalarMultBase(hexToBytes(bobPrivate)))).assertEqual(bobPublic);
    });

    it('matches RFC 7748 shared secret vector from both peers', 0, () => {
      expect(bytesToHex(X25519.scalarMult(hexToBytes(alicePrivate), hexToBytes(bobPublic)))).assertEqual(sharedSecret);
      expect(bytesToHex(X25519.scalarMult(hexToBytes(bobPrivate), hexToBytes(alicePublic)))).assertEqual(sharedSecret);
    });

    it('matches the mobile-web noble-curves shared secret shape', 0, () => {
      expect(bytesToHex(X25519.scalarMultBase(hexToBytes(mobileWebPrivate)))).assertEqual(mobileWebPublic);
      expect(bytesToHex(X25519.scalarMultBase(hexToBytes(desktopPrivate)))).assertEqual(desktopPublic);
      expect(bytesToHex(X25519.scalarMult(hexToBytes(mobileWebPrivate), hexToBytes(desktopPublic))))
        .assertEqual(mobileWebSharedSecret);
      expect(bytesToHex(X25519.scalarMult(hexToBytes(desktopPrivate), hexToBytes(mobileWebPublic))))
        .assertEqual(mobileWebSharedSecret);
    });
  });

  describe('RemoteCrypto', () => {
    it('encrypts and decrypts JSON between derived peers', 0, async () => {
      const peers = connectedPeers();
      const alice = peers[0];
      const bob = peers[1];
      const command: CryptoCommandFixture = {
        cmd: 'send_message',
        session_id: 'session-1',
        content: 'hello from mobile',
        agent_type: 'code',
        image_contexts: [{
          id: 'image-1',
          data_url: 'data:image/png;base64,aGVsbG8=',
          mime_type: 'image/png'
        }]
      };

      const encrypted = await alice.encryptJson(command);
      const decrypted = await bob.decryptJson<CryptoCommandFixture>(encrypted);

      expect(decrypted.cmd).assertEqual(command.cmd);
      expect(decrypted.session_id).assertEqual(command.session_id);
      expect(decrypted.content).assertEqual(command.content);
      expect(decrypted.agent_type).assertEqual(command.agent_type);
      expect(decrypted.image_contexts.length).assertEqual(1);
      expect(decrypted.image_contexts[0].id).assertEqual('image-1');
      expect(decrypted.image_contexts[0].mime_type).assertEqual('image/png');
    });

    it('produces relay-compatible encrypted payload shape', 0, async () => {
      const peers = connectedPeers();
      const command: CryptoSimpleCommandFixture = {
        cmd: 'get_workspace_info'
      };
      const encrypted = await peers[0].encryptJson(command);
      const keys = Object.keys(encrypted).sort().join(',');
      const nonce = Encoding.base64ToBytes(encrypted.nonce);
      const encryptedData = Encoding.base64ToBytes(encrypted.encrypted_data);

      expect(keys).assertEqual('encrypted_data,nonce');
      expect(nonce.length).assertEqual(12);
      expect(encryptedData.length).assertLarger(16);
      expect(encrypted.encrypted_data.indexOf('get_workspace_info')).assertEqual(-1);
      const decrypted = await peers[1].decryptJson<CryptoCommandFixture>(encrypted);
      expect(decrypted.cmd).assertEqual('get_workspace_info');
    });

    it('rejects use before shared-key derivation', 0, async () => {
      let didThrow = false;
      try {
        const command: CryptoSimpleCommandFixture = {
          cmd: 'ping'
        };
        const privateKey = hexToBytes('77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a');
        await new RemoteCrypto({
          cipher: new FakeRemoteCryptoCipher(),
          keyPair: {
            privateKey,
            publicKey: X25519.scalarMultBase(privateKey)
          },
          randomBytes: fixedRandomBytes
        }).encryptJson(command);
      } catch (_err) {
        didThrow = true;
      }
      expect(didThrow).assertEqual(true);
    });
  });

  describe('TimeFormat', () => {
    const now = Date.parse('2026-06-10T12:00:00.000Z');

    it('formats recent timestamps as just now', 0, () => {
      expect(TimeFormat.relative('2026-06-10T11:59:45.000Z', now)).assertEqual('刚刚');
    });

    it('formats minute and hour relative timestamps', 0, () => {
      expect(TimeFormat.relative('2026-06-10T11:42:00.000Z', now)).assertEqual('18 分钟前');
      expect(TimeFormat.relative('2026-06-10T09:00:00.000Z', now)).assertEqual('3 小时前');
    });

    it('formats older timestamps as days or dates', 0, () => {
      expect(TimeFormat.relative('2026-06-08T12:00:00.000Z', now)).assertEqual('2 天前');
      expect(TimeFormat.relative('2026-05-30T12:00:00.000Z', now)).assertEqual('2026-05-30');
    });

    it('does not show invalid timestamps as just now', 0, () => {
      expect(TimeFormat.relative('', now)).assertEqual('时间未知');
      expect(TimeFormat.relative('not-a-date', now)).assertEqual('时间未知');
    });

    it('formats numeric timestamps and future timestamps safely', 0, () => {
      expect(TimeFormat.relative('1781091720', now)).assertEqual('18 分钟前');
      expect(TimeFormat.timestampMs('1781091720000')).assertEqual(now - 18 * 60 * 1000);
      expect(TimeFormat.timestampMs('1781091720000000')).assertEqual(now - 18 * 60 * 1000);
      expect(TimeFormat.timestampMs('1781091720000000000')).assertEqual(now - 18 * 60 * 1000);
      expect(TimeFormat.relative('2026-06-12T12:00:00.000Z', now)).assertEqual('2026-06-12');
    });
  });

  describe('GeneralChatRepository', () => {
    it('persists a complete ordinary chat and restores it without Remote state', 0, async () => {
      const localStore = new FakeGeneralChatLocalStore();
      const repository = new GeneralChatRepository(new MockGeneralChatAdapter(), localStore);
      const session = await repository.createSession('帮我整理一份发布计划');
      const result = await repository.sendMessage(session.id, '先给我三个步骤');
      await repository.recordAssistantMessage(session.id, result.assistantMessage);

      expect(repository.listSessions().length).assertEqual(1);
      expect(repository.listSessions()[0].agentType).assertEqual('chat');
      expect(repository.listSessions()[0].messageCount).assertEqual(2);

      const restored = new GeneralChatRepository(new MockGeneralChatAdapter(), localStore);
      await restored.restoreFromLocal();
      const restoredMessages = await restored.messages(session.id);
      expect(restored.listSessions().length).assertEqual(1);
      expect(restoredMessages.length).assertEqual(2);
      expect(restoredMessages[0].role).assertEqual('user');
      expect(restoredMessages[1].role).assertEqual('assistant');
    });

    it('renames ordinary chat sessions through the local repository', 0, async () => {
      const localStore = new FakeGeneralChatLocalStore();
      const repository = new GeneralChatRepository(new MockGeneralChatAdapter(), localStore);
      const session = await repository.createSession('原始标题');
      await repository.renameSession(session.id, '新的标题');

      expect(repository.listSessions()[0].title).assertEqual('新的标题');
      expect(localStore.sessions[0].title).assertEqual('新的标题');
    });

    it('archives ordinary chat sessions and restores the archived state', 0, async () => {
      const localStore = new FakeGeneralChatLocalStore();
      const repository = new GeneralChatRepository(new MockGeneralChatAdapter(), localStore);
      const session = await repository.createSession('待归档会话');
      await repository.archiveSession(session.id, true);

      const restored = new GeneralChatRepository(new MockGeneralChatAdapter(), localStore);
      await restored.restoreFromLocal();
      expect(restored.listSessions()[0].status).assertEqual('archived');

      await restored.archiveSession(session.id, false);
      expect(restored.listSessions()[0].status).assertEqual('ready');
      expect(localStore.sessions[0].status).assertEqual('ready');
    });

    it('persists drafts independently and removes session-owned data on delete', 0, async () => {
      const localStore = new FakeGeneralChatLocalStore();
      const repository = new GeneralChatRepository(new MockGeneralChatAdapter(), localStore);
      const session = await repository.createSession('待删除会话');
      await repository.saveDraft('new-chat', '首页草稿');
      await repository.saveDraft(session.id, '会话草稿');
      await repository.sendMessage(session.id, '一条消息');

      expect(await repository.draft('new-chat')).assertEqual('首页草稿');
      expect(await repository.draft(session.id)).assertEqual('会话草稿');
      await repository.deleteSession(session.id);

      expect(repository.listSessions().length).assertEqual(0);
      expect((await repository.messages(session.id)).length).assertEqual(0);
      expect(await repository.draft(session.id)).assertEqual('');
      expect(await repository.draft('new-chat')).assertEqual('首页草稿');
    });

    it('persists a failed user message when the provider rejects before streaming', 0, async () => {
      const localStore = new FakeGeneralChatLocalStore();
      const repository = new GeneralChatRepository(new FailingGeneralChatAdapter(), localStore);
      const session = await repository.createSession('失败状态');
      let failed = false;
      try {
        await repository.sendMessage(session.id, '请重试我');
      } catch (_err) {
        failed = true;
      }

      const messages = await repository.messages(session.id);
      expect(failed).assertEqual(true);
      expect(messages.length).assertEqual(1);
      expect(messages[0].status).assertEqual('failed');
    });

    it('keeps the user message sent when a stream fails after receiving content', 0, async () => {
      const localStore = new FakeGeneralChatLocalStore();
      const repository = new GeneralChatRepository(new PartialFailingGeneralChatAdapter(), localStore);
      const session = await repository.createSession('流中断状态');
      try {
        await repository.sendMessage(session.id, '保留这条消息');
      } catch (_err) {
      }

      const messages = await repository.messages(session.id);
      expect(messages.length).assertEqual(1);
      expect(messages[0].status).assertEqual('sent');
    });
  });

  describe('GeneralChatExportFormatter', () => {
    it('exports visible conversation content as Markdown without hidden error detail', 0, () => {
      const session: RemoteSession = {
        id: 'chat-export',
        title: '发布计划',
        agentType: 'chat',
        status: 'ready',
        updatedAt: '2026-07-20T10:00:00.000Z',
        createdAt: '2026-07-20T09:00:00.000Z',
        messageCount: 2
      };
      const failedAssistant = chatMessage('assistant-1', 'assistant', '先检查发布清单。', 'failed');
      failedAssistant.detail = 'secret retry context';
      const markdown = GeneralChatExportFormatter.markdown(session, [
        chatMessage('user-1', 'user', '帮我整理发布计划'),
        failedAssistant
      ]);

      expect(markdown).assertEqual('# 发布计划\n\n## 用户\n\n帮我整理发布计划\n\n## BitFun\n\n先检查发布清单。\n');
      expect(markdown.indexOf('secret retry context')).assertEqual(-1);
    });
  });

  describe('GeneralChatStreamCoordinator', () => {
    it('batches deltas into one stable active message and flushes before completion', 0, () => {
      const coordinator = new GeneralChatStreamCoordinator();
      const first = coordinator.begin('chat-1', 'turn-1');
      const flushed: ChatMessage[] = [];
      coordinator.append('chat-1', '第一段\n\n\n', () => true, (message: ChatMessage) => {
        flushed.push(message);
      });
      coordinator.append('chat-1', '第二段', () => true, (message: ChatMessage) => {
        flushed.push(message);
      });
      const terminal = coordinator.flush('chat-1', true);

      expect(first.id).assertEqual('active-turn-1');
      expect(terminal?.id || '').assertEqual(first.id);
      expect(terminal?.text || '').assertEqual('第一段\n\n第二段');
      expect(coordinator.text()).assertEqual('第一段\n\n第二段');
      coordinator.reset();
    });

    it('rejects late deltas after reset or from another session', 0, () => {
      const coordinator = new GeneralChatStreamCoordinator();
      coordinator.begin('chat-1', 'turn-1');
      coordinator.reset();

      expect(coordinator.append('chat-1', 'late', () => true, (_message: ChatMessage) => {}))
        .assertEqual(false);
      expect(coordinator.append('chat-2', 'wrong', () => true, (_message: ChatMessage) => {}))
        .assertEqual(false);
      expect(coordinator.hasActiveStream()).assertEqual(false);
    });
  });

  describe('GeneralChatStreamLifecycleController', () => {
    it('owns stream request lifecycle state around coordinator updates', 0, () => {
      const controller = new GeneralChatStreamLifecycleController();
      const active = controller.begin('chat-1');
      const flushed: ChatMessage[] = [];

      controller.setRequestInFlight(true);
      expect(controller.isRequestInFlight()).assertEqual(true);
      expect(active.id.indexOf('active-')).assertEqual(0);
      expect(controller.append('chat-1', 'hello', () => true, (message: ChatMessage) => {
        flushed.push(message);
      })).assertEqual(true);
      const terminal = controller.flush('chat-1', true);
      expect(terminal?.text || '').assertEqual('hello');
      expect(controller.text()).assertEqual('hello');

      controller.markRequestCancelled();
      expect(controller.takeRequestCancelled()).assertEqual(true);
      expect(controller.takeRequestCancelled()).assertEqual(false);
      controller.setPendingFinalMessage(chatMessage('assistant-final', 'assistant', 'stopped', 'cancelled'));
      expect(controller.takePendingFinalMessage()?.id || '').assertEqual('assistant-final');
      expect(controller.takePendingFinalMessage() === undefined).assertTrue();

      controller.setRequestInFlight(false);
      controller.reset();
      expect(controller.isRequestInFlight()).assertEqual(false);
      expect(controller.currentSessionId()).assertEqual('');
      expect(controller.hasActiveStream()).assertEqual(false);
    });
  });

  describe('GeneralChatServiceStatus', () => {
    it('derives readiness only from complete model configuration', 0, () => {
      expect(GeneralChatServiceStatus.fromConfiguration('https://api.example.com', 'model', true))
        .assertEqual(GeneralChatServiceState.Ready);
      expect(GeneralChatServiceStatus.fromConfiguration('', 'model', true))
        .assertEqual(GeneralChatServiceState.Unconfigured);
    });

    it('classifies authentication, network and provider failures independently from Remote state', 0, () => {
      expect(GeneralChatServiceStatus.fromError(RemoteI18n.t('generalChat.authenticationFailed')))
        .assertEqual(GeneralChatServiceState.AuthExpired);
      expect(GeneralChatServiceStatus.fromError('Network connection timeout.'))
        .assertEqual(GeneralChatServiceState.Offline);
      expect(GeneralChatServiceStatus.fromError(RemoteI18n.t('generalChat.serviceUnavailable')))
        .assertEqual(GeneralChatServiceState.Failed);
    });
  });

  describe('ChatComposerPolicy', () => {
    it('keeps ordinary chat send enabled across every Remote connection state', 0, () => {
      const states = ['idle', 'pairing', 'connected', 'reconnecting', 'failed', 'disconnected'];
      states.forEach((state: string) => {
        expect(ChatComposerPolicy.canSend('普通问题', 0, false, false, state)).assertEqual(true);
      });
    });

    it('still gates Remote chat when the desktop connection is unavailable', 0, () => {
      expect(ChatComposerPolicy.canSend('运行任务', 0, false, true, 'failed')).assertEqual(false);
      expect(ChatComposerPolicy.canSend('运行任务', 0, false, true, 'connected')).assertEqual(true);
      expect(ChatComposerPolicy.canSend('运行任务', 0, false, true, 'reconnecting')).assertEqual(true);
    });
  });

  describe('GeneralChatApiClient', () => {
    it('builds authenticated session and turn requests', 0, async () => {
      const transport = new FakeGeneralChatHttpTransport();
      transport.responses = [{
        statusCode: 200,
        body: '{"session":{"id":"chat-1","title":"计划","status":"ready","created_at":"now","updated_at":"now","message_count":0}}'
      }, {
        statusCode: 202,
        body: '{"turn_id":"turn-1","accepted_at":"now","version":1}'
      }];
      const client = new GeneralChatApiClient(
        'https://chat.example.com/',
        new FakeGeneralChatTokenProvider(),
        transport
      );

      const session = await client.createSession('  计划  ');
      const receipt = await client.sendTurn(session.session.id, '  生成步骤  ', '  chat-model  ');

      expect(session.session.id).assertEqual('chat-1');
      expect(receipt.turn_id).assertEqual('turn-1');
      expect(transport.requests[0].url).assertEqual('https://chat.example.com/api/mobile/v1/chat/sessions');
      expect(transport.requests[0].authorization).assertEqual('Bearer test-token');
      expect(transport.requests[0].body).assertEqual('{"title":"计划"}');
      expect(transport.requests[1].method).assertEqual(GeneralChatHttpMethod.Post);
      expect(transport.requests[1].body).assertEqual('{"content":"生成步骤","model":"chat-model"}');
    });

    it('maps authentication failures without exposing response bodies', 0, async () => {
      const transport = new FakeGeneralChatHttpTransport();
      transport.responses = [{ statusCode: 401, body: '{"token":"must-not-leak"}' }];
      const client = new GeneralChatApiClient(
        'https://chat.example.com',
        new FakeGeneralChatTokenProvider(),
        transport
      );
      let message = '';
      try {
        await client.capabilities();
      } catch (err) {
        message = err instanceof Error ? err.message : `${err}`;
      }

      expect(message).assertEqual('General chat authentication expired.');
      expect(message.indexOf('must-not-leak')).assertEqual(-1);
    });
  });

  describe('GeneralChatConfigValidator', () => {
    it('accepts an existing key when URL and model are valid', 0, () => {
      const error = GeneralChatConfigValidator.validate({
        apiUrl: 'https://chat.example.com/',
        apiKey: '',
        modelName: 'chat-model',
        clearApiKey: false
      }, true);
      expect(error).assertEqual('');
    });

    it('rejects unsafe URLs and incomplete new configurations', 0, () => {
      const urlError = GeneralChatConfigValidator.validate({
        apiUrl: 'file:///tmp/chat',
        apiKey: 'secret',
        modelName: 'chat-model',
        clearApiKey: false
      }, false);
      const keyError = GeneralChatConfigValidator.validate({
        apiUrl: 'https://chat.example.com',
        apiKey: '',
        modelName: 'chat-model',
        clearApiKey: false
      }, false);
      expect(urlError).assertEqual('API URL 必须以 http:// 或 https:// 开头。');
      expect(keyError).assertEqual('请输入 API Key。');
    });
  });

  describe('ModelProviderGeneralChatAdapter', () => {
    it('infers provider protocol and request URL without a protocol setting', 0, () => {
      const openBitFunProtocol = ModelProviderGeneralChatAdapter.resolveProtocol('https://api.openbitfun.com');
      const customProtocol = ModelProviderGeneralChatAdapter.resolveProtocol('https://llm.example.com/v1');

      expect(openBitFunProtocol).assertEqual(ModelProviderProtocol.Anthropic);
      expect(customProtocol).assertEqual(ModelProviderProtocol.OpenAi);
      expect(ModelProviderGeneralChatAdapter.resolveRequestUrl(
        'https://api.openbitfun.com/',
        openBitFunProtocol
      )).assertEqual('https://api.openbitfun.com/v1/messages');
      expect(ModelProviderGeneralChatAdapter.resolveRequestUrl(
        'https://llm.example.com/v1',
        customProtocol
      )).assertEqual('https://llm.example.com/v1/chat/completions');
    });

    it('extracts text from Anthropic and OpenAI compatible responses', 0, () => {
      const anthropicText = ModelProviderGeneralChatAdapter.responseText(
        '{"content":[{"type":"text","text":"你好"}]}',
        ModelProviderProtocol.Anthropic
      );
      const openAiText = ModelProviderGeneralChatAdapter.responseText(
        '{"choices":[{"message":{"content":"Hello"}}]}',
        ModelProviderProtocol.OpenAi
      );

      expect(anthropicText).assertEqual('你好');
      expect(openAiText).assertEqual('Hello');
    });

    it('places the BitFun system prompt in both request formats', 0, () => {
      const messages: ModelProviderMessage[] = [{ role: 'user', content: 'BitFun 是什么？' }];
      const anthropicBody = ModelProviderGeneralChatAdapter.buildRequestBody(
        'model-a',
        messages,
        ModelProviderProtocol.Anthropic
      );
      const openAiBody = ModelProviderGeneralChatAdapter.buildRequestBody(
        'model-b',
        messages,
        ModelProviderProtocol.OpenAi
      );

      expect(anthropicBody.indexOf('local AI workbench')).assertLarger(-1);
      expect(anthropicBody.indexOf('"system"')).assertLarger(-1);
      expect(openAiBody.indexOf('"role":"system"')).assertLarger(-1);
      expect(openAiBody.indexOf('blockchain')).assertLarger(-1);
    });
  });

  describe('ModelProviderSseParser', () => {
    it('parses Anthropic deltas split across network chunks', 0, () => {
      const parser = new ModelProviderSseParser(ModelProviderProtocol.Anthropic);
      const first = parser.feed(
        'event: message_start\ndata: {"type":"message_start","message":{"id":"msg-1"}}\n\n' +
        'event: content_block_delta\ndata: {"type":"content_block_delta","delta":{"type":"text_'
      );
      const second = parser.feed('delta","text":"你好"}}\n\n');
      const third = parser.feed(
        'event: content_block_delta\ndata: {"type":"content_block_delta","delta":{"type":"text_delta","text":"！"}}\n\n'
      );

      expect(first.length).assertEqual(0);
      expect(second.join('')).assertEqual('你好');
      expect(third.join('')).assertEqual('！');
      expect(parser.responseId()).assertEqual('msg-1');
    });

    it('parses OpenAI CRLF events and ignores the done marker', 0, () => {
      const parser = new ModelProviderSseParser(ModelProviderProtocol.OpenAi);
      const deltas = parser.feed(
        'data: {"id":"chat-1","choices":[{"delta":{"content":"Hello"}}]}\r\n\r\n' +
        'data: {"id":"chat-1","choices":[{"delta":{"content":" world"}}]}\r\n\r\n' +
        'data: [DONE]\r\n\r\n'
      );

      expect(deltas.join('')).assertEqual('Hello world');
      expect(parser.responseId()).assertEqual('chat-1');
      expect(parser.finish().length).assertEqual(0);
    });
  });

  describe('GeneralChatEventMapper', () => {
    it('projects ordered stream deltas and ignores duplicate versions', 0, () => {
      let projection = GeneralChatEventMapper.emptyProjection('chat-1', 'turn-1');
      projection = GeneralChatEventMapper.apply(projection, {
        event_id: 'event-1',
        version: 1,
        session_id: 'chat-1',
        turn_id: 'turn-1',
        type: 'message.delta',
        delta: '你好'
      });
      const duplicate = GeneralChatEventMapper.apply(projection, {
        event_id: 'event-1',
        version: 1,
        session_id: 'chat-1',
        turn_id: 'turn-1',
        type: 'message.delta',
        delta: '不应重复'
      });
      projection = GeneralChatEventMapper.apply(duplicate, {
        event_id: 'event-2',
        version: 2,
        session_id: 'chat-1',
        turn_id: 'turn-1',
        type: 'turn.completed'
      });

      expect(projection.text).assertEqual('你好');
      expect(projection.status).assertEqual('completed');
      expect(projection.version).assertEqual(2);
    });

    it('rejects stream version gaps so callers can reload a snapshot', 0, () => {
      const projection = GeneralChatEventMapper.emptyProjection('chat-1', 'turn-1', 3);
      let didThrow = false;
      try {
        GeneralChatEventMapper.apply(projection, {
          event_id: 'event-5',
          version: 5,
          session_id: 'chat-1',
          turn_id: 'turn-1',
          type: 'message.delta',
          delta: '缺少版本 4'
        });
      } catch (_err) {
        didThrow = true;
      }
      expect(didThrow).assertEqual(true);
    });

    it('preserves text and tool event order while updating a stable tool item', 0, () => {
      let projection = GeneralChatEventMapper.emptyProjection('chat-1', 'turn-1');
      projection = GeneralChatEventMapper.apply(projection, {
        event_id: 'text-1', version: 1, session_id: 'chat-1', turn_id: 'turn-1',
        type: 'message.delta', delta: 'before'
      });
      projection = GeneralChatEventMapper.apply(projection, {
        event_id: 'tool-event-1', version: 2, session_id: 'chat-1', turn_id: 'turn-1',
        type: 'tool.started', tool: { id: 'tool-1', name: 'WebSearch', status: 'running' }
      });
      projection = GeneralChatEventMapper.apply(projection, {
        event_id: 'tool-event-2', version: 3, session_id: 'chat-1', turn_id: 'turn-1',
        type: 'tool.completed', tool: { id: 'tool-1', name: 'WebSearch', status: 'completed' }
      });
      projection = GeneralChatEventMapper.apply(projection, {
        event_id: 'text-2', version: 4, session_id: 'chat-1', turn_id: 'turn-1',
        type: 'message.delta', delta: 'after'
      });

      expect(projection.items.length).assertEqual(3);
      expect(projection.items[0].content).assertEqual('before');
      expect(projection.items[1].tool?.id || '').assertEqual('tool-1');
      expect(projection.items[1].tool?.status || '').assertEqual('completed');
      expect(projection.items[2].content).assertEqual('after');
      expect(projection.tools.length).assertEqual(1);
    });

    it('projects permission and capability states and ignores late terminal events', 0, () => {
      let projection = GeneralChatEventMapper.emptyProjection('chat-1', 'turn-1');
      projection = GeneralChatEventMapper.apply(projection, {
        event_id: 'permission-1', version: 1, session_id: 'chat-1', turn_id: 'turn-1',
        type: 'permission.required',
        tool: { id: 'tool-1', name: 'Write', status: 'waiting' }
      });
      expect(projection.status).assertEqual('pending_confirmation');
      expect(projection.tools[0].status || '').assertEqual('pending_confirmation');

      projection = GeneralChatEventMapper.apply(projection, {
        event_id: 'capability-1', version: 2, session_id: 'chat-1', turn_id: 'turn-1',
        type: 'capability.required', capability: 'remote_workspace'
      });
      expect(projection.status).assertEqual('capability_required');
      expect(projection.capabilityRequired).assertEqual('remote_workspace');

      projection = GeneralChatEventMapper.apply(projection, {
        event_id: 'cancel-1', version: 3, session_id: 'chat-1', turn_id: 'turn-1',
        type: 'turn.cancelled'
      });
      const terminal = GeneralChatEventMapper.apply(projection, {
        event_id: 'late-1', version: 4, session_id: 'chat-1', turn_id: 'turn-1',
        type: 'message.delta', delta: 'late'
      });
      expect(terminal.status).assertEqual('cancelled');
      expect(terminal.version).assertEqual(3);
      expect(terminal.text).assertEqual('');
    });
  });

  describe('RemoteCommandFactory', () => {
    it('builds workspace and assistant commands', 0, () => {
      expectCommandJson(RemoteCommandFactory.getWorkspaceInfo(), '{"cmd":"get_workspace_info"}');
      expectCommandJson(RemoteCommandFactory.listRecentWorkspaces(), '{"cmd":"list_recent_workspaces"}');
      expectCommandJson(RemoteCommandFactory.setWorkspace('/repo'), '{"cmd":"set_workspace","path":"/repo"}');
      expectCommandJson(RemoteCommandFactory.listAssistants(), '{"cmd":"list_assistants"}');
      expectCommandJson(RemoteCommandFactory.setAssistant('/assistant'), '{"cmd":"set_assistant","path":"/assistant"}');
    });

    it('builds session list and creation commands without unsupported fields', 0, () => {
      const listCommand = RemoteCommandFactory.listSessions('/repo', 8, 16, '  auth  ');
      expectCommandJson(listCommand, '{"cmd":"list_sessions","workspace_path":"/repo","limit":8,"offset":16,"query":"auth"}');
      expect(JSON.stringify(listCommand).indexOf('agent_type')).assertEqual(-1);

      const createCommand = RemoteCommandFactory.createSession({
        agentType: 'cowork',
        title: '',
        instruction: 'hello'
      }, '/repo');
      expectCommandJson(createCommand, '{"cmd":"create_session","agent_type":"cowork","session_name":"Remote Cowork Session","workspace_path":"/repo"}');
      expect(JSON.stringify(createCommand).indexOf('run_mode')).assertEqual(-1);
    });

    it('builds message and polling commands', 0, () => {
      expectCommandJson(RemoteCommandFactory.sendMessage('s1', 'hello', 'code'), '{"cmd":"send_message","session_id":"s1","content":"hello","agent_type":"code"}');
      expectCommandJson(RemoteCommandFactory.sendMessage('s1', 'look', 'code', [{
        id: 'img1',
        data_url: 'data:image/png;base64,abc',
        mime_type: 'image/png'
      }]), '{"cmd":"send_message","session_id":"s1","content":"look","agent_type":"code","image_contexts":[{"id":"img1","data_url":"data:image/png;base64,abc","mime_type":"image/png"}]}');
      expectCommandJson(RemoteCommandFactory.getSessionMessages('s1'), '{"cmd":"get_session_messages","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.pollSession('s1', 7, 3, 2), '{"cmd":"poll_session","session_id":"s1","since_version":7,"known_msg_count":3,"known_model_catalog_version":2}');
    });

    it('builds model and session lifecycle commands', 0, () => {
      expectCommandJson(RemoteCommandFactory.getModelCatalog('s1'), '{"cmd":"get_model_catalog","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.setSessionModel('s1', 'model-a'), '{"cmd":"set_session_model","session_id":"s1","model_id":"model-a"}');
      expectCommandJson(RemoteCommandFactory.cancelTask('s1'), '{"cmd":"cancel_task","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.cancelTask('s1', 'turn-9'), '{"cmd":"cancel_task","session_id":"s1","turn_id":"turn-9"}');
      expectCommandJson(RemoteCommandFactory.renameSession('s1', 'Next'), '{"cmd":"update_session_title","session_id":"s1","title":"Next"}');
      expectCommandJson(RemoteCommandFactory.deleteSession('s1'), '{"cmd":"delete_session","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.ping(), '{"cmd":"ping"}');
    });

    it('builds tool and file commands', 0, () => {
      const answers: RemoteQuestionAnswerPayload = { answer: 'yes', '0': 'yes' };

      expectCommandJson(RemoteCommandFactory.confirmTool('tool-1'), '{"cmd":"confirm_tool","tool_id":"tool-1"}');
      expectCommandJson(RemoteCommandFactory.rejectTool('tool-1', 'no'), '{"cmd":"reject_tool","tool_id":"tool-1","reason":"no"}');
      expectCommandJson(RemoteCommandFactory.cancelTool('tool-1', 'stop'), '{"cmd":"cancel_tool","tool_id":"tool-1","reason":"stop"}');
      expectCommandJson(RemoteCommandFactory.answerQuestion('tool-2', answers), '{"cmd":"answer_question","tool_id":"tool-2","answers":{"0":"yes","answer":"yes"}}');
      expectCommandJson(RemoteCommandFactory.getFileInfo('/tmp/a.txt', 's1'), '{"cmd":"get_file_info","path":"/tmp/a.txt","session_id":"s1"}');
      expectCommandJson(RemoteCommandFactory.readFileChunk('/tmp/a.txt', 12, 64, 's1'), '{"cmd":"read_file_chunk","path":"/tmp/a.txt","offset":12,"limit":64,"session_id":"s1"}');
    });
  });

  describe('RemoteResponseMapper', () => {
    it('maps session response items', 0, () => {
      const sessions = RemoteResponseMapper.sessions([{
        session_id: 'session-abcdef',
        name: 'Fallback title',
        agent_type: 'agentic',
        status: 'running',
        updated_at: '2026-06-10T12:00:00.000Z',
        created_at: '2026-06-09T12:00:00.000Z',
        message_count: 4,
        workspace_path: '/repo',
        workspace_name: 'Repo'
      }]);
      expect(sessions.length).assertEqual(1);
      expect(sessions[0].id).assertEqual('session-abcdef');
      expect(sessions[0].title).assertEqual('Fallback title');
      expect(sessions[0].agentType).assertEqual('agentic');
      expect(sessions[0].messageCount).assertEqual(4);
      expect(sessions[0].workspaceName).assertEqual('Repo');
    });

    it('maps alternate session time fields', 0, () => {
      const sessions = RemoteResponseMapper.sessions([{
        session_id: 'session-time',
        title: 'Alternate time fields',
        last_message_at: 1781092800000,
        createdAt: '2026-06-09 12:00:00',
        message_count: 1
      }]);
      expect(sessions.length).assertEqual(1);
      expect(sessions[0].updatedAt).assertEqual('1781092800000');
      expect(sessions[0].createdAt).assertEqual('2026-06-09 12:00:00');
    });

    it('maps assistant messages and nested tools', 0, () => {
      const message = RemoteResponseMapper.chatMessage({
        id: 'm1',
        role: 'assistant',
        content: '',
        thinking: 'Thinking out loud',
        timestamp: '2026-06-10T12:00:00.000Z',
        items: [{
          type: 'tool',
          tool: {
            id: 'tool-1',
            name: 'shell',
            status: 'pending'
          },
          subItems: [{
            type: 'tool',
            tool: {
              id: 'tool-2',
              name: 'edit',
              status: 'done'
            }
          }]
        }]
      });
      expect(message.id).assertEqual('m1');
      expect(message.text).assertEqual('');
      expect(message.thinking || '').assertEqual('Thinking out loud');
      expect(message.status).assertEqual('done');
      expect(message.detail).assertEqual('shell · pending\nedit · done');
      expect(message.tools ? message.tools.length : 0).assertEqual(2);
    });

    it('uses fallback ids for message rendering keys', 0, () => {
      const fromMessageId = RemoteResponseMapper.chatMessage({
        message_id: 'desktop-message-1',
        role: 'assistant',
        content: 'Mapped from alternate id',
        timestamp: '2026-06-10T12:00:00.000Z'
      });
      const generated = RemoteResponseMapper.chatMessage({
        role: 'user',
        content: 'No id in response',
        timestamp: '2026-06-10T12:01:00.000Z'
      });

      expect(fromMessageId.id).assertEqual('desktop-message-1');
      expect(generated.id.length > 0).assertTrue();
      expect(generated.id.indexOf('user-2026-06-10T12:01:00.000Z-')).assertEqual(0);
    });

    it('maps active turns with item fallback text and tool detail', 0, () => {
      const message = RemoteResponseMapper.activeTurnToMessage({
        turn_id: 'turn-1',
        status: 'active',
        items: [{
          type: 'text',
          content: 'Working on it'
        }],
        tools: [{
          id: 'tool-3',
          name: 'read_file',
          status: 'running'
        }]
      });
      expect(message.id).assertEqual('active-turn-1');
      expect(message.turnId || '').assertEqual('turn-1');
      expect(message.role).assertEqual('assistant');
      expect(message.text).assertEqual('Working on it');
      expect(message.status).assertEqual('active');
      expect(message.detail).assertEqual('read_file · running');
    });

    it('does not promote subagent progress to active turn text', 0, () => {
      const message = RemoteResponseMapper.activeTurnToMessage({
        turn_id: 'turn-structured',
        status: 'active',
        items: [{
          type: 'subagent',
          is_subagent: true,
          content: 'Subagent check',
          subItems: [{
            type: 'text',
            content: 'Nested progress'
          }, {
            type: 'tool',
            tool: {
              id: 'tool-nested-1',
              name: 'inspect',
              status: 'running'
            }
          }]
        }]
      });

      expect(message.text).assertEqual('');
      expect(message.tools ? message.tools.length : 0).assertEqual(1);
      expect(message.detail).assertEqual('inspect · running');
    });

    it('does not use thinking as active turn final text', 0, () => {
      const message = RemoteResponseMapper.activeTurnToMessage({
        turn_id: 'turn-thinking-only',
        status: 'completed',
        thinking: 'Still reasoning',
        items: [{
          type: 'thinking',
          content: 'Still reasoning'
        }]
      });

      expect(message.text).assertEqual('');
      expect(message.thinking || '').assertEqual('Still reasoning');
      expect(message.status).assertEqual('completed');
    });
  });

  describe('ChatTimelineProjector', () => {
    it('filters seed messages and returns empty state for empty real history', 0, () => {
      const items = ChatTimelineProjector.project([
        chatMessage('system-seed', 'assistant', 'Welcome')
      ], [], chatMessage('', 'assistant', ''), false);

      expect(items.length).assertEqual(1);
      expect(items[0].type).assertEqual('empty_state');
    });

    it('deduplicates optimistic user messages replaced by persisted messages', 0, () => {
      const items = ChatTimelineProjector.project([
        chatMessage('remote-user-1', 'user', 'Explain this file')
      ], [
        chatMessage('msg-local-1', 'user', ' Explain this file ')
      ], chatMessage('', 'assistant', ''), false);

      expect(items.length).assertEqual(1);
      expect(items[0].id).assertEqual('message-remote-user-1');
      expect(items[0].type).assertEqual('user_message');
    });

    it('keeps optimistic image messages when persisted image payload differs', 0, () => {
      const items = ChatTimelineProjector.project([
        chatMessageWithImage('remote-user-1', 'user', 'Analyze image', 'remote.png', 'data:image/png;base64,remote')
      ], [
        chatMessageWithImage('msg-local-1', 'user', 'Analyze image', 'local.png', 'data:image/png;base64,local')
      ], chatMessage('', 'assistant', ''), false);

      expect(items.length).assertEqual(2);
      expect(items[0].id).assertEqual('message-remote-user-1');
      expect(items[1].id).assertEqual('pending-msg-local-1');
      expect(items[1].type).assertEqual('optimistic_user_message');
    });

    it('keeps active turn until matching assistant message is persisted', 0, () => {
      const activeTurn = chatMessage('active-turn-1', 'assistant', 'Done with the edit', 'completed');
      const beforeFinal = ChatTimelineProjector.project([
        chatMessage('remote-user-1', 'user', 'Please edit')
      ], [], activeTurn, false);
      const afterFinal = ChatTimelineProjector.project([
        chatMessage('remote-user-1', 'user', 'Please edit'),
        chatMessage('assistant-final-1', 'assistant', 'Done with the edit')
      ], [], activeTurn, false);

      expect(beforeFinal.length).assertEqual(2);
      expect(beforeFinal[1].type).assertEqual('assistant_live_turn');
      expect(beforeFinal[1].isFinalizing).assertEqual(true);
      expect(afterFinal.length).assertEqual(2);
      expect(afterFinal[1].id).assertEqual('message-assistant-final-1');
      expect(afterFinal[1].type).assertEqual('assistant_message');
    });

    it('updates active turn item id when visible content changes', 0, () => {
      const first = activeChatMessage('turn-stable-1', 'abc', 'active', 1);
      const second = activeChatMessage('turn-stable-1', 'abcdef', 'active', 2);
      const firstItems = ChatTimelineProjector.project([], [], first, false);
      const secondItems = ChatTimelineProjector.project([], [], second, false);

      expect(firstItems.length).assertEqual(1);
      expect(secondItems.length).assertEqual(1);
      expect(firstItems[0].id.indexOf('active-turn-stable-1-')).assertEqual(0);
      expect(secondItems[0].id.indexOf('active-turn-stable-1-')).assertEqual(0);
      expect(secondItems[0].id === firstItems[0].id).assertFalse();
    });

    it('hides active turn when final assistant id matches turn id', 0, () => {
      const activeTurn = activeChatMessage('turn-final-1', 'partial active text', 'completed');
      const items = ChatTimelineProjector.project([
        chatMessage('turn-final-1_assistant', 'assistant', 'rewritten final text')
      ], [], activeTurn, false);

      expect(items.length).assertEqual(1);
      expect(items[0].id).assertEqual('message-turn-final-1_assistant');
      expect(items[0].type).assertEqual('assistant_message');
    });

    it('keeps active turn when matching persisted assistant has no final text yet', 0, () => {
      const activeTurn = activeChatMessage('turn-final-wait-1', 'final answer', 'completed');
      const pendingFinal = chatMessage('turn-final-wait-1_assistant', 'assistant', '');
      pendingFinal.text = 'Still reasoning';
      pendingFinal.thinking = 'Still reasoning';
      const items = ChatTimelineProjector.project([pendingFinal], [], activeTurn, false);

      expect(items.length).assertEqual(2);
      expect(items[1].type).assertEqual('assistant_live_turn');
      expect(items[1].isFinalizing).assertEqual(true);
    });
  });

  describe('ChatTimelineStore', () => {
    it('updates same-id persisted messages when final content arrives later', 0, () => {
      const pendingFinal = chatMessage('assistant-final-same-id', 'assistant', '');
      pendingFinal.text = 'Still reasoning';
      pendingFinal.thinking = 'Still reasoning';
      const merged = RemoteUiState.mergeMessages([pendingFinal], [
        chatMessage('assistant-final-same-id', 'assistant', 'Final answer')
      ]);

      expect(merged.length).assertEqual(1);
      expect(merged[0].text).assertEqual('Final answer');
      expect(merged[0].thinking || '').assertEqual('');
    });

    it('owns optimistic merge and active turn cleanup rules', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.appendOptimisticMessage(chatMessage('msg-local-1', 'user', 'Please edit'));
      store.setActiveTurn(chatMessage('active-turn-1', 'assistant', 'Final text', 'completed'));
      store.mergePersistedMessages([
        chatMessage('remote-user-1', 'user', 'Please edit')
      ]);

      let state = store.snapshot();
      expect(state.persistedMessages.length).assertEqual(1);
      expect(state.optimisticMessages.length).assertEqual(0);
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-1');
      expect(state.syncPhase).assertEqual('finalizing');

      store.mergePersistedMessages([
        chatMessage('assistant-final-1', 'assistant', 'Final text')
      ]);
      state = store.snapshot();
      expect(state.persistedMessages.length).assertEqual(2);
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('');
    });

    it('projects empty state through the store', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-empty');
      const items = store.project(false);

      expect(items.length).assertEqual(1);
      expect(items[0].type).assertEqual('empty_state');
    });

    it('creates a local active turn from send response turn ids', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.setLocalActiveTurn(' turn-preview-1 ');

      let state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-preview-1');
      expect(state.activeTurn ? (state.activeTurn.turnId || '') : '').assertEqual('turn-preview-1');
      expect(state.activeTurn ? state.activeTurn.status : '').assertEqual('active');
      expect(state.syncPhase).assertEqual('streaming');

      store.setLocalActiveTurn('turn-preview-2');
      state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-preview-1');
    });

    it('replaces pending active turns with remote turn ids', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      const pendingId = store.setPendingActiveTurn('msg-local-1');

      let state = store.snapshot();
      expect(pendingId).assertEqual('active-pending-msg-local-1');
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-pending-msg-local-1');
      expect(state.syncPhase).assertEqual('streaming');

      store.setLocalActiveTurn('turn-remote-1');
      state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-remote-1');
      expect(state.activeTurn ? (state.activeTurn.turnId || '') : '').assertEqual('turn-remote-1');
    });

    it('keeps streamed active turn text when send response arrives later', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      const active = chatMessage('active-turn-remote-1', 'assistant', 'partial text', 'active');
      active.turnId = 'turn-remote-1';
      store.setActiveTurn(active);

      store.setLocalActiveTurn('turn-remote-1');

      const state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-remote-1');
      expect(state.activeTurn ? state.activeTurn.text : '').assertEqual('partial text');
      expect(state.activeTurn ? (state.activeTurn.turnId || '') : '').assertEqual('turn-remote-1');
    });

    it('keeps active turn text from going backwards for the same turn id', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.setActiveTurn(activeChatMessage('turn-stream-1', 'abcdef'));
      store.setActiveTurn(activeChatMessage('turn-stream-1', 'abc', 'active', 2));

      const state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.text : '').assertEqual('abcdef');
      expect(state.activeTurn ? (state.activeTurn.turnId || '') : '').assertEqual('turn-stream-1');
      expect(state.activeTurn ? (state.activeTurn.renderVersion || 0) : 0).assertEqual(2);
    });

    it('monotonically merges structured text items for active turns', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      const first = activeChatMessage('turn-items-1', '');
      first.items = [{
        type: 'text',
        content: 'abcdef'
      }];
      const second = activeChatMessage('turn-items-1', '', 'active', 2);
      second.items = [{
        type: 'text',
        content: 'abc'
      }];

      store.setActiveTurn(first);
      store.setActiveTurn(second);

      const state = store.snapshot();
      const items = state.activeTurn && state.activeTurn.items ? state.activeTurn.items : [];
      expect(items.length).assertEqual(1);
      expect(items[0].content || '').assertEqual('abcdef');
    });

    it('updates active tool status from latest structured item snapshot', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      const first = activeChatMessage('turn-tool-1', '');
      first.items = [{
        type: 'tool',
        tool: {
          id: 'tool-1',
          name: 'read_file',
          status: 'running'
        }
      }];
      const second = activeChatMessage('turn-tool-1', '', 'active', 2);
      second.items = [{
        type: 'tool',
        tool: {
          id: 'tool-1',
          name: 'read_file',
          status: 'completed',
          duration_ms: 50
        }
      }];

      store.setActiveTurn(first);
      store.setActiveTurn(second);

      const state = store.snapshot();
      const items = state.activeTurn && state.activeTurn.items ? state.activeTurn.items : [];
      expect(items.length).assertEqual(1);
      expect(items[0].tool ? (items[0].tool.status || '') : '').assertEqual('completed');
      expect(items[0].tool ? (items[0].tool.duration_ms || 0) : 0).assertEqual(50);
    });

    it('clears active turn when persisted assistant id matches turn id', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.setActiveTurn(activeChatMessage('turn-final-store-1', 'partial active text', 'completed'));
      store.mergePersistedMessages([
        chatMessage('turn-final-store-1_assistant', 'assistant', 'rewritten final text')
      ]);

      const state = store.snapshot();
      expect(state.persistedMessages.length).assertEqual(1);
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('');
    });

    it('holds completed active turn until persisted assistant has final content', 0, () => {
      const store = new ChatTimelineStore();
      store.reset('session-1');
      store.setActiveTurn(activeChatMessage('turn-final-store-wait-1', 'final answer', 'completed'));

      const pendingFinal = chatMessage('turn-final-store-wait-1_assistant', 'assistant', '');
      pendingFinal.text = 'Still reasoning';
      pendingFinal.thinking = 'Still reasoning';
      store.mergePersistedMessages([pendingFinal]);
      store.setActiveTurn(undefined);

      let state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('active-turn-final-store-wait-1');
      expect(state.syncPhase).assertEqual('finalizing');

      store.mergePersistedMessages([
        chatMessage('turn-final-store-wait-1_assistant', 'assistant', 'final answer')
      ]);
      state = store.snapshot();
      expect(state.activeTurn ? state.activeTurn.id : '').assertEqual('');
    });
  });

  describe('MarkdownParser', () => {
    it('parses common markdown blocks into stable block types', 0, () => {
      const blocks = MarkdownParser.parse('# Title\n\n- first\n- **second**\n\n> quote\n\n```ts\nconst value = 1;\n```\n\n| A | B |\n| --- | --- |\n| 1 | 2 |');

      expect(blocks.length).assertEqual(5);
      expect(blocks[0].type).assertEqual('heading');
      expect(blocks[0].level).assertEqual(1);
      expect(blocks[1].type).assertEqual('list');
      expect(blocks[1].items.length).assertEqual(2);
      expect(blocks[1].items[1].text).assertEqual('second');
      expect(blocks[2].type).assertEqual('quote');
      expect(blocks[3].type).assertEqual('code');
      expect(blocks[3].language).assertEqual('ts');
      expect(blocks[4].type).assertEqual('table');
    });

    it('formats inline code, emphasis, and links without markdown control text', 0, () => {
      const blocks = MarkdownParser.parse('Use `pnpm test`, **bold text**, and [docs](https://example.test).');

      expect(blocks.length).assertEqual(1);
      expect(blocks[0].type).assertEqual('paragraph');
      expect(blocks[0].text).assertEqual('Use pnpm test, bold text, and docs (https://example.test).');
      expect(blocks[0].inlines.length).assertEqual(7);
      expect(blocks[0].inlines[1].type).assertEqual('code');
      expect(blocks[0].inlines[3].type).assertEqual('strong');
      expect(blocks[0].inlines[5].type).assertEqual('link');
      expect(blocks[0].inlines[5].url).assertEqual('https://example.test');
    });
  });

  describe('GeneralChatPageState', () => {
    it('projects configuration, busy state, and status text', 0, () => {
      const state = new GeneralChatPageState();
      state.setConfiguration(
        'https://api.openbitfun.com',
        'deepseek-v4-flash',
        true,
        GeneralChatServiceState.Ready
      );
      state.setBusy(true);
      state.setStatus('Generating');

      expect(state.apiUrl).assertEqual('https://api.openbitfun.com');
      expect(state.modelName).assertEqual('deepseek-v4-flash');
      expect(state.hasApiKey).assertTrue();
      expect(state.serviceState).assertEqual(GeneralChatServiceState.Ready);
      expect(state.isBusy).assertTrue();
      expect(state.statusText).assertEqual('Generating');
    });

    it('owns a copied session list and returns it in recent order', 0, () => {
      const state = new GeneralChatPageState();
      const older = remoteSession('chat-older', 'Older chat');
      const newer = remoteSession('chat-newer', 'Newer chat');
      older.updatedAt = '2026-07-20T00:00:00.000Z';
      newer.updatedAt = '2026-07-21T00:00:00.000Z';
      const sessions = [older, newer];

      state.setSessions(sessions);
      sessions.length = 0;

      const recent = state.recentSessions();
      expect(state.sessions.length).assertEqual(2);
      expect(recent[0].id).assertEqual('chat-newer');
      expect(recent[1].id).assertEqual('chat-older');
    });

    it('owns active session and timeline projection copies', 0, () => {
      const state = new GeneralChatPageState();
      const userMessage = chatMessage('user-1', 'user', 'Plan the refactor');
      const pendingMessage = chatMessage('pending-1', 'user', 'Retry this');
      const activeTurn = activeChatMessage('turn-1', 'Working');
      const timelineItems: ChatTimelineItem[] = [{
        id: 'message-user-1',
        type: 'user_message',
        message: userMessage,
        isStreaming: false,
        isFinalizing: false
      }];

      state.setActiveSession({
        sessionId: 'chat-1',
        title: 'Architecture',
        workspacePath: '',
        agentType: 'chat'
      });
      state.setTimelineProjection([userMessage], [pendingMessage], activeTurn, true, timelineItems);
      timelineItems.length = 0;

      expect(state.activeSession.sessionId).assertEqual('chat-1');
      expect(state.activeSession.agentType).assertEqual('chat');
      expect(state.persistedMessages.length).assertEqual(1);
      expect(state.optimisticMessages.length).assertEqual(1);
      expect(state.timelineItems.length).assertEqual(1);
      expect(state.hasMoreMessages).assertTrue();
      expect(state.hasRunningActiveTurn()).assertTrue();
      expect(state.latestUserMessageText()).assertEqual('Retry this');
    });

    it('copies nested session projection and replaces streaming turn snapshots', 0, () => {
      const state = new GeneralChatPageState();
      const session: SessionSummary = {
        sessionId: 'chat-nested-1',
        title: 'Original title',
        workspacePath: '/must-not-leak',
        agentType: 'code',
        initialTurnId: 'turn-initial'
      };
      const firstTurn = activeChatMessage('turn-stream-1', 'Hel', 'active', 1);
      const secondTurn = activeChatMessage('turn-stream-1', 'Hello', 'active', 2);

      state.setActiveSession(session);
      session.sessionId = 'mutated-session';
      session.title = 'Mutated title';

      state.setTimelineProjection([], [], firstTurn, false, []);
      firstTurn.text = 'mutated first turn';
      state.setTimelineProjection([], [], secondTurn, false, []);

      expect(state.activeSession.sessionId).assertEqual('chat-nested-1');
      expect(state.activeSession.title).assertEqual('Original title');
      expect(state.activeSession.agentType).assertEqual('chat');
      expect(state.activeSession.workspacePath).assertEqual('/must-not-leak');
      expect(state.activeTurnMessage.text).assertEqual('Hello');
      expect(state.activeTurnMessage.renderVersion || 0).assertEqual(2);
      expect(state.hasRunningActiveTurn()).assertTrue();
    });

    it('owns ordinary chat composer projection', 0, () => {
      const state = new GeneralChatPageState();
      const images = [selectedImage('general-image-1')];

      state.setChatInput('Draft text');
      state.setSelectedImages(images);
      state.setVoiceListening(true);
      images.length = 0;

      expect(state.chatInput).assertEqual('Draft text');
      expect(state.selectedImages.length).assertEqual(1);
      expect(state.isVoiceListening).assertTrue();

      state.removeSelectedImage('general-image-1');
      expect(state.selectedImages.length).assertEqual(0);

      state.clearComposer();
      state.setVoiceListening(false);
      expect(state.chatInput).assertEqual('');
      expect(state.selectedImages.length).assertEqual(0);
      expect(state.isVoiceListening).assertFalse();
    });
  });

  describe('RemotePageState', () => {
    it('projects connection summary and filters visible sessions', 0, () => {
      const state = new RemotePageState();
      state.setDesktopIdentity('Developer Mac', 'desktop-1');
      state.setPairingInput('https://relay.example.com/#/pair?room=abc', 'harmony-device', true);
      state.setAuthenticatedUserId('desktop-user');
      state.setStatusText('Connected');
      state.setConnectionState('connected');
      state.setConnectionFailureKind('');
      state.setBusy(true);
      state.setWorkspace('BitFun', '/workspace/BitFun', 'default', 'main', 'normal');
      state.setSessions([
        remoteSession('session-1', '/canvas draw architecture'),
        remoteSession('session-2', 'Write release notes'),
        remoteSession('session-3', '/canvas archived', 'archived'),
        remoteSession('', '/canvas invalid')
      ], true);
      state.setQuery('CANVAS');

      const visible = state.visibleSessions();
      expect(state.desktopName).assertEqual('Developer Mac');
      expect(state.desktopId).assertEqual('desktop-1');
      expect(state.remoteUrl).assertEqual('https://relay.example.com/#/pair?room=abc');
      expect(state.userId).assertEqual('harmony-device');
      expect(state.authenticatedUserId).assertEqual('desktop-user');
      expect(state.statusText).assertEqual('Connected');
      expect(state.connectionState).assertEqual('connected');
      expect(state.connectionFailureKind).assertEqual('');
      expect(state.isBusy).assertTrue();
      expect(state.showRemoteUrlInput).assertTrue();
      expect(state.workspacePath).assertEqual('/workspace/BitFun');
      expect(state.workspaceBranch).assertEqual('main');
      expect(state.workspaceKind).assertEqual('normal');
      expect(state.assistantId).assertEqual('default');
      expect(state.sessions.length).assertEqual(4);
      expect(state.hasMoreSessions).assertTrue();
      expect(visible.length).assertEqual(1);
      expect(visible[0].id).assertEqual('session-1');
    });

    it('clears loading errors on success and clears only list state on reset', 0, () => {
      const state = new RemotePageState();
      state.setDesktopName('Developer Mac');
      state.setWorkspace('BitFun', '/workspace/BitFun', 'default');
      state.setQuery('canvas');
      state.setLoading(true);
      state.setError('network failed');

      expect(state.isLoadingSessions).assertFalse();
      expect(state.sessionErrorText).assertEqual('network failed');

      state.setSessions([remoteSession('session-1', '/canvas')], false);
      expect(state.sessionErrorText).assertEqual('');
      state.clear();

      expect(state.sessions.length).assertEqual(0);
      expect(state.hasMoreSessions).assertFalse();
      expect(state.isLoadingSessions).assertFalse();
      expect(state.sessionQuery).assertEqual('canvas');
      expect(state.desktopName).assertEqual('Developer Mac');
      expect(state.workspaceName).assertEqual('BitFun');
    });

    it('owns active remote session, timeline projection, and model catalog', 0, () => {
      const state = new RemotePageState();
      const userMessage = chatMessage('remote-user-1', 'user', 'Run tests');
      const activeTurn = activeChatMessage('remote-turn-1', 'Running cargo test');
      const timelineItems: ChatTimelineItem[] = [{
        id: 'active-remote-turn-1-0',
        type: 'assistant_live_turn',
        message: activeTurn,
        isStreaming: true,
        isFinalizing: false
      }];
      const modelCatalog: RemoteModelCatalog = {
        version: 2,
        models: [{
          id: 'model-a',
          name: 'Model A',
          provider: 'local',
          base_url: 'https://model.example.com',
          model_name: 'model-a',
          enabled: true,
          capabilities: ['code']
        }],
        default_models: { primary: 'model-a' },
        session_model_id: 'model-a'
      };

      state.setActiveSession({
        sessionId: 'remote-1',
        title: 'Remote task',
        workspacePath: '/workspace/BitFun',
        agentType: 'code'
      });
      state.setTimelineProjection([userMessage], [], activeTurn, false, timelineItems);
      state.setModelCatalog(modelCatalog, 'model-a');
      timelineItems.length = 0;

      expect(state.activeSession.sessionId).assertEqual('remote-1');
      expect(state.activeSession.workspacePath).assertEqual('/workspace/BitFun');
      expect(state.timelineItems.length).assertEqual(1);
      expect(state.hasRunningActiveTurn()).assertTrue();
      expect(state.modelCatalog.version).assertEqual(2);
      expect(state.selectedModelId).assertEqual('model-a');
    });

    it('copies nested remote session projection and replaces streaming turn snapshots', 0, () => {
      const state = new RemotePageState();
      const session: SessionSummary = {
        sessionId: 'remote-nested-1',
        title: 'Remote original',
        workspacePath: '/workspace/BitFun',
        agentType: 'code',
        initialTurnId: 'remote-turn-initial'
      };
      const firstTurn = activeChatMessage('remote-stream-1', 'Run', 'active', 1);
      const secondTurn = activeChatMessage('remote-stream-1', 'Run tests', 'active', 2);
      const catalog = remoteModelCatalog('model-a');

      state.setActiveSession(session);
      session.sessionId = 'mutated-remote';
      session.workspacePath = '/mutated';

      state.setTimelineProjection([], [], firstTurn, false, []);
      firstTurn.text = 'mutated remote turn';
      state.setTimelineProjection([], [], secondTurn, false, []);
      state.setModelCatalog(catalog, 'model-a');
      catalog.models.length = 0;
      catalog.session_model_id = 'model-b';

      expect(state.activeSession.sessionId).assertEqual('remote-nested-1');
      expect(state.activeSession.workspacePath).assertEqual('/workspace/BitFun');
      expect(state.activeSession.agentType).assertEqual('code');
      expect(state.activeTurnMessage.text).assertEqual('Run tests');
      expect(state.activeTurnMessage.renderVersion || 0).assertEqual(2);
      expect(state.modelCatalog.models.length).assertEqual(2);
      expect(state.modelCatalog.session_model_id || '').assertEqual('model-a');
      expect(state.selectedModelId).assertEqual('model-a');
      expect(state.hasRunningActiveTurn()).assertTrue();
    });

    it('owns remote download status projection', 0, () => {
      const state = new RemotePageState();

      state.setDownloadStatus('/workspace/BitFun/README.md', '', '10 B / 20 B');
      expect(state.downloadingFilePath).assertEqual('/workspace/BitFun/README.md');
      expect(state.downloadedFilePath).assertEqual('');
      expect(state.fileDownloadStatus).assertEqual('10 B / 20 B');

      state.setDownloadStatus('/workspace/BitFun/README.md', '/workspace/BitFun/README.md', '下载完成 README.md · 20 B');
      state.clearDownloadingFilePath();
      expect(state.downloadingFilePath).assertEqual('');
      expect(state.downloadedFilePath).assertEqual('/workspace/BitFun/README.md');
      expect(state.fileDownloadStatus).assertEqual('下载完成 README.md · 20 B');

      state.clearDownloadStatus();
      expect(state.downloadingFilePath).assertEqual('');
      expect(state.downloadedFilePath).assertEqual('');
      expect(state.fileDownloadStatus).assertEqual('');
    });

    it('owns remote composer attachments and voice projection', 0, () => {
      const state = new RemotePageState();
      const first = selectedImage('remote-image-1');
      const second = selectedImage('remote-image-2');
      const images = [first];

      state.setChatInput('/canvas draw');
      state.setSelectedImages(images);
      state.addSelectedImages([second]);
      state.setVoiceListening(true);
      images.length = 0;

      expect(state.chatInput).assertEqual('/canvas draw');
      expect(state.selectedImages.length).assertEqual(2);
      expect(state.selectedImages[1].id).assertEqual('remote-image-2');
      expect(state.isVoiceListening).assertTrue();

      state.removeSelectedImage('remote-image-1');
      expect(state.selectedImages.length).assertEqual(1);
      expect(state.selectedImages[0].id).assertEqual('remote-image-2');

      state.clearComposer();
      state.setVoiceListening(false);
      expect(state.chatInput).assertEqual('');
      expect(state.selectedImages.length).assertEqual(0);
      expect(state.isVoiceListening).assertFalse();
    });

    it('owns copied workspace and assistant picker projections', 0, () => {
      const state = new RemotePageState();
      const workspaces: RecentWorkspaceEntry[] = [{
        path: '/workspace/BitFun',
        name: 'BitFun',
        lastOpened: '2026-07-21T00:00:00.000Z',
        workspaceKind: 'normal'
      }];
      const assistants: AssistantEntry[] = [{
        path: '/workspace/BitFun/.bitfun/assistants/default.md',
        name: 'Default',
        assistant_id: 'default'
      }];

      state.setRecentWorkspaces(workspaces);
      state.setAssistants(assistants);
      state.setWorkspacePickerVisible(true);
      state.setAssistantPickerVisible(true);
      workspaces.length = 0;
      assistants.length = 0;

      expect(state.recentWorkspaces.length).assertEqual(1);
      expect(state.assistants.length).assertEqual(1);
      expect(state.showWorkspacePicker).assertTrue();
      expect(state.showAssistantPicker).assertTrue();

      state.clearWorkspaceActions();
      expect(state.recentWorkspaces.length).assertEqual(0);
      expect(state.assistants.length).assertEqual(0);
      expect(state.showWorkspacePicker).assertFalse();
      expect(state.showAssistantPicker).assertFalse();
    });
  });

  describe('ProductSurfaceIsolation', () => {
    it('keeps ordinary chat and remote chat page state isolated', 0, () => {
      const general = new GeneralChatPageState();
      const remote = new RemotePageState();
      const generalMessage = chatMessage('general-user-1', 'user', 'Local chat only');
      const remoteMessage = chatMessage('remote-user-1', 'user', 'Remote task only');

      general.setActiveSession({
        sessionId: 'general-1',
        title: 'General chat',
        workspacePath: '/should-not-leak',
        agentType: 'code'
      });
      remote.setActiveSession({
        sessionId: 'remote-1',
        title: 'Remote chat',
        workspacePath: '/workspace/BitFun',
        agentType: 'code'
      });
      general.setTimelineProjection([generalMessage], [], RemoteUiState.emptyActiveTurn(), false, [{
        id: 'general-item-1',
        type: 'user_message',
        message: generalMessage,
        isStreaming: false,
        isFinalizing: false
      }]);
      remote.setTimelineProjection([remoteMessage], [], RemoteUiState.emptyActiveTurn(), false, [{
        id: 'remote-item-1',
        type: 'user_message',
        message: remoteMessage,
        isStreaming: false,
        isFinalizing: false
      }]);

      expect(general.activeSession.agentType).assertEqual('chat');
      expect(remote.activeSession.agentType).assertEqual('code');
      expect(general.persistedMessages[0].id).assertEqual('general-user-1');
      expect(remote.persistedMessages[0].id).assertEqual('remote-user-1');

      general.clearActiveSession();

      expect(general.activeSession.sessionId).assertEqual('');
      expect(general.persistedMessages.length).assertEqual(0);
      expect(remote.activeSession.sessionId).assertEqual('remote-1');
      expect(remote.activeSession.workspacePath).assertEqual('/workspace/BitFun');
      expect(remote.persistedMessages.length).assertEqual(1);
    });

    it('keeps streaming turn projections isolated across product surfaces', 0, () => {
      const general = new GeneralChatPageState();
      const remote = new RemotePageState();
      const generalTurn = activeChatMessage('general-live-1', 'General partial', 'active', 1);
      const remoteTurn = activeChatMessage('remote-live-1', 'Remote partial', 'active', 1);

      general.setTimelineProjection([], [], generalTurn, false, [{
        id: 'general-live-item',
        type: 'assistant_live_turn',
        message: generalTurn,
        isStreaming: true,
        isFinalizing: false
      }]);
      remote.setTimelineProjection([], [], remoteTurn, false, [{
        id: 'remote-live-item',
        type: 'assistant_live_turn',
        message: remoteTurn,
        isStreaming: true,
        isFinalizing: false
      }]);

      expect(general.activeTurnMessage.id).assertEqual('active-general-live-1');
      expect(remote.activeTurnMessage.id).assertEqual('active-remote-live-1');
      expect(general.timelineItems[0].id).assertEqual('general-live-item');
      expect(remote.timelineItems[0].id).assertEqual('remote-live-item');

      general.clearTimeline();

      expect(general.hasRunningActiveTurn()).assertFalse();
      expect(general.timelineItems.length).assertEqual(0);
      expect(remote.hasRunningActiveTurn()).assertTrue();
      expect(remote.activeTurnMessage.text).assertEqual('Remote partial');
      expect(remote.timelineItems.length).assertEqual(1);
    });
  });

  describe('AppShellState', () => {
    it('owns global sidebar and sheet visibility', 0, () => {
      const state = new AppShellState();

      state.setSidebarVisible(true);
      state.setSettingsVisible(true);
      state.setConnectSheetVisible(true);

      expect(state.showSidebar).assertTrue();
      expect(state.showSettings).assertTrue();
      expect(state.showConnectSheet).assertTrue();

      state.closeGlobalSurfaces();
      expect(state.showSidebar).assertFalse();
      expect(state.showSettings).assertFalse();
      expect(state.showConnectSheet).assertFalse();
    });
  });

  describe('RemoteHeartbeatController', () => {
    it('owns heartbeat timer restart, stop, and tick dispatch', 0, () => {
      const scheduler = new FakeRemoteHeartbeatScheduler();
      let tickCount = 0;
      const controller = new RemoteHeartbeatController(() => {
        tickCount += 1;
      }, 15000, scheduler);

      controller.start();
      expect(controller.isRunning()).assertTrue();
      expect(scheduler.intervals.get(1)).assertEqual(15000);
      scheduler.tick(1);
      expect(tickCount).assertEqual(1);

      controller.start();
      expect(scheduler.clearedIds.length).assertEqual(1);
      expect(scheduler.clearedIds[0]).assertEqual(1);
      expect(scheduler.callbacks.has(1)).assertFalse();
      expect(scheduler.callbacks.has(2)).assertTrue();

      controller.stop();
      controller.stop();
      expect(controller.isRunning()).assertFalse();
      expect(scheduler.clearedIds.length).assertEqual(2);
      expect(scheduler.clearedIds[1]).assertEqual(2);
    });
  });

  describe('RemoteActivityLifecycleController', () => {
    it('owns remote heartbeat lifecycle while preserving tick dispatch', 0, () => {
      const scheduler = new FakeRemoteHeartbeatScheduler();
      let tickCount = 0;
      const controller = new RemoteActivityLifecycleController(() => {
        tickCount += 1;
      }, 15000, scheduler);

      controller.startHeartbeat();
      expect(controller.isHeartbeatRunning()).assertTrue();
      expect(scheduler.intervals.get(1)).assertEqual(15000);
      scheduler.tick(1);
      expect(tickCount).assertEqual(1);

      controller.startHeartbeat();
      expect(scheduler.clearedIds[0]).assertEqual(1);
      expect(scheduler.callbacks.has(1)).assertFalse();
      expect(scheduler.callbacks.has(2)).assertTrue();

      controller.stopHeartbeat();
      controller.stopHeartbeat();
      expect(controller.isHeartbeatRunning()).assertFalse();
      expect(scheduler.clearedIds.length).assertEqual(2);
      expect(scheduler.clearedIds[1]).assertEqual(2);
    });
  });

  describe('RemoteChatPollingLifecycleController', () => {
    it('starts the active remote chat poller and forwards snapshots', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      manager.results = [pollResult({
        version: 5,
        changed: true,
        sessionState: 'idle',
        title: 'Updated Session',
        newMessages: [chatMessage('message-5', 'assistant', 'Done')],
        totalMessageCount: 5
      })];
      const controller = new RemoteChatPollingLifecycleController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          snapshots.push(snapshot);
        },
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      controller.startActiveSession({
        sessionId: 'session-1',
        cursor: {
          pollVersion: 4,
          knownMessageCount: 4,
          knownModelCatalogVersion: 0
        }
      });
      await delay(20);
      controller.stop();

      expect(snapshots.length).assertEqual(1);
      expect(snapshots[0].sessionId).assertEqual('session-1');
      expect(snapshots[0].cursor.pollVersion).assertEqual(5);
      expect(snapshots[0].cursor.knownMessageCount).assertEqual(5);
      expect(snapshots[0].newMessages[0].id).assertEqual('message-5');
    });

    it('ignores empty active session starts', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      manager.results = [pollResult({
        version: 1,
        changed: false,
        sessionState: 'idle',
        title: 'Ignored',
        newMessages: [],
        totalMessageCount: 0
      })];
      const controller = new RemoteChatPollingLifecycleController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          snapshots.push(snapshot);
        },
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      controller.startActiveSession({
        sessionId: '',
        cursor: {
          pollVersion: 0,
          knownMessageCount: 0,
          knownModelCatalogVersion: 0
        }
      });
      await delay(20);

      expect(snapshots.length).assertEqual(0);
      expect(manager.results.length).assertEqual(1);
    });
  });

  describe('VoiceInputLifecycleController', () => {
    it('owns start stop guard and callback routing for visible voice input', 0, async () => {
      const service = new FakeVoiceInputLifecycleService();
      let inputText = '';
      let statusText = '';
      let clearAllCount = 0;
      const inputEvents: string[] = [];
      const listeningEvents: string[] = [];
      const statuses: string[] = [];
      const errors: string[] = [];
      const callbacks: VoiceInputLifecycleCallbacks = {
        currentInputText: (): string => inputText,
        currentStatusText: (): string => statusText,
        onInputText: (routeId: string, text: string): void => {
          inputText = text;
          inputEvents.push(`${routeId}:${text}`);
        },
        onListening: (routeId: string, isListening: boolean): void => {
          listeningEvents.push(`${routeId}:${isListening ? '1' : '0'}`);
        },
        onStatusText: (nextStatusText: string): void => {
          statusText = nextStatusText;
          statuses.push(nextStatusText);
        },
        onError: (message: string): void => {
          statusText = message;
          errors.push(message);
        }
      };
      const controller = new VoiceInputLifecycleController(service, callbacks);
      const snapshot: VoiceInputRouteSnapshot = {
        routeId: 'GeneralChat',
        isListening: false,
        isBusy: false,
        inputText: '',
        selectedImageCount: 0
      };

      expect(await controller.toggle({} as Context, snapshot)).assertTrue();
      service.emitText('recognized text', true);
      expect(service.startRequests).assertEqual(1);
      expect(inputEvents[0]).assertEqual('GeneralChat:recognized text');
      expect(listeningEvents[0]).assertEqual('GeneralChat:1');
      expect(statuses.indexOf(RemoteI18n.t('status.voiceStarting')) >= 0).assertTrue();
      expect(statuses.indexOf(RemoteI18n.t('status.voiceListening')) >= 0).assertTrue();
      expect(statuses.indexOf(RemoteI18n.t('status.voiceRecognized')) >= 0).assertTrue();

      expect(await controller.stop({
        routeId: 'GeneralChat',
        isListening: true,
        isBusy: false,
        inputText,
        selectedImageCount: 0
      }, true)).assertTrue();
      expect(service.stopRequests).assertEqual(1);
      expect(listeningEvents[listeningEvents.length - 1]).assertEqual('GeneralChat:0');

      expect(await controller.toggle({} as Context, {
        routeId: 'GeneralChat',
        isListening: false,
        isBusy: true,
        inputText: '',
        selectedImageCount: 0
      })).assertFalse();
      expect(service.startRequests).assertEqual(1);

      service.shouldFailStart = true;
      inputText = '';
      expect(await controller.toggle({} as Context, snapshot)).assertFalse();
      expect(errors[0]).assertEqual('Expected voice start failure.');

      await controller.cancel('GeneralChat', () => {
        clearAllCount += 1;
      });
      expect(service.cancelRequests).assertEqual(1);
      expect(clearAllCount).assertEqual(1);
    });
  });

  describe('GeneralChatDraftController', () => {
    it('owns debounced draft save timer and immediate persistence paths', 0, async () => {
      const store = new FakeGeneralChatLocalStore();
      const scheduler = new FakeGeneralChatDraftScheduler();
      const controller = new GeneralChatDraftController(store, 250, (_err: Error) => {}, scheduler);

      controller.scheduleSave('draft-1', 'first');
      expect(controller.isPending()).assertTrue();
      expect(scheduler.delays.get(1)).assertEqual(250);
      scheduler.tick(1);
      await Promise.resolve();
      expect(controller.isPending()).assertFalse();
      expect(await store.loadDraft('draft-1')).assertEqual('first');

      controller.scheduleSave('draft-1', 'old');
      controller.scheduleSave('draft-1', 'new');
      expect(scheduler.clearedIds[0]).assertEqual(2);
      scheduler.tick(2);
      await Promise.resolve();
      expect(await store.loadDraft('draft-1')).assertEqual('first');
      scheduler.tick(3);
      await Promise.resolve();
      expect(await store.loadDraft('draft-1')).assertEqual('new');

      controller.scheduleSave('draft-1', 'pending');
      controller.persistNow('draft-1', 'now');
      await Promise.resolve();
      expect(scheduler.clearedIds[1]).assertEqual(4);
      expect(await store.loadDraft('draft-1')).assertEqual('now');

      await controller.clearNow('draft-1');
      expect(await store.loadDraft('draft-1')).assertEqual('');
      await store.saveDraft('draft-1', 'restored');
      expect(await controller.restore('draft-1')).assertEqual('restored');
    });
  });

  describe('GeneralChatDraftLifecycleController', () => {
    it('routes home and visible draft lifecycle operations through the draft controller', 0, async () => {
      const store = new FakeGeneralChatLocalStore();
      const scheduler = new FakeGeneralChatDraftScheduler();
      const draftController = new GeneralChatDraftController(store, 250, (_err: Error) => {}, scheduler);
      let visibleDraftId = '';
      const lifecycle = new GeneralChatDraftLifecycleController(
        draftController,
        'home-draft',
        (): string => visibleDraftId
      );

      lifecycle.scheduleVisible('ignored');
      expect(draftController.isPending()).assertFalse();

      visibleDraftId = 'chat-1';
      lifecycle.scheduleVisible('draft text');
      expect(draftController.isPending()).assertTrue();
      scheduler.tick(1);
      await Promise.resolve();
      expect(await lifecycle.restore('chat-1')).assertEqual('draft text');

      lifecycle.persistVisible('persisted text');
      await Promise.resolve();
      expect(await lifecycle.restore('chat-1')).assertEqual('persisted text');

      await store.saveDraft('home-draft', 'home text');
      expect(await lifecycle.restoreHome()).assertEqual('home text');
      await lifecycle.clearHomeNow();
      expect(await lifecycle.restoreHome()).assertEqual('');

      lifecycle.scheduleVisible('pending');
      lifecycle.cancel();
      expect(draftController.isPending()).assertFalse();
    });
  });

  describe('GeneralChatBootstrapController', () => {
    it('restores configuration sessions and home draft through lifecycle collaborators', 0, async () => {
      const configStore = new FakeGeneralChatConfigStore();
      const client = new FakeGeneralChatCommandClient();
      const localStore = new FakeGeneralChatLocalStore();
      const draftController = new GeneralChatDraftController(localStore);
      const draftLifecycle = new GeneralChatDraftLifecycleController(
        draftController,
        'home-draft',
        (): string => 'home-draft'
      );
      const restoredConfigs: GeneralChatConfigSnapshot[] = [];
      const restoredDrafts: string[] = [];
      const statuses: string[] = [];
      await localStore.saveDraft('home-draft', 'saved home draft');
      const commandController = new GeneralChatCommandController(client, {
        onSessions: (_sessions: RemoteSession[]) => {},
        onSessionPrepared: (_sessionId: string) => {},
        onActiveSession: (_session: SessionSummary) => {},
        onMessagesLoaded: (_messages: ChatMessage[]) => {},
        onClearComposer: () => {},
        onChatInput: (_text: string) => {},
        onStatusText: (_statusText: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onToast: (_statusText: string) => {}
      });
      const bootstrap = new GeneralChatBootstrapController(
        configStore,
        commandController,
        draftLifecycle,
        {
          onConfigRestored: (snapshot: GeneralChatConfigSnapshot) => {
            restoredConfigs.push(snapshot);
          },
          onHomeDraftRestored: (text: string) => {
            restoredDrafts.push(text);
          },
          onStatusText: (statusText: string) => {
            statuses.push(statusText);
          }
        }
      );

      await bootstrap.restore({} as Context);

      expect(configStore.initRequests).assertEqual(1);
      expect(client.initRequests).assertEqual(1);
      expect(restoredConfigs[0].modelName).assertEqual('model-a');
      expect(restoredDrafts[0]).assertEqual('saved home draft');
      expect(statuses.length).assertEqual(0);

      client.shouldFail = true;
      await bootstrap.restore({} as Context);
      expect(statuses[0]).assertEqual(RemoteI18n.t('generalChat.localRestoreFailed'));
    });
  });

  describe('GeneralChatCommandController', () => {
    it('opens and creates general chat sessions through page callbacks', 0, async () => {
      const client = new FakeGeneralChatCommandClient();
      const activeSessions: SessionSummary[] = [];
      const preparedSessions: string[] = [];
      const openedRoutes: string[] = [];
      const loadedMessages: ChatMessage[][] = [];
      const chatInputs: string[] = [];
      let clearComposerCount = 0;
      const session = remoteSession('chat-1', 'Chat 1');
      session.agentType = 'chat';
      client.currentSessions = [session];
      client.messagesBySession.set('chat-1', [chatMessage('message-1', 'assistant', 'Welcome')]);
      const controller = new GeneralChatCommandController(client, {
        onSessions: (_items: RemoteSession[]) => {},
        onSessionPrepared: (sessionId: string) => {
          preparedSessions.push(sessionId);
        },
        onActiveSession: (activeSession: SessionSummary) => {
          activeSessions.push(activeSession);
        },
        onMessagesLoaded: (messages: ChatMessage[]) => {
          loadedMessages.push(messages);
        },
        onClearComposer: () => {
          clearComposerCount += 1;
        },
        onChatInput: (text: string) => {
          chatInputs.push(text);
        },
        onStatusText: (_statusText: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onToast: (_statusText: string) => {}
      });

      const opened = await controller.openSession(session, false, async (_sessionId: string): Promise<string> => {
        return 'draft text';
      }, (sessionId: string): void => {
        openedRoutes.push(`open:${sessionId}`);
      });
      const created = await controller.createSession(' new prompt ', false, async (): Promise<void> => {
        openedRoutes.push('draft-cleared');
      }, (sessionId: string): void => {
        openedRoutes.push(`create:${sessionId}`);
      });

      expect(opened).assertTrue();
      expect(created).assertTrue();
      expect(preparedSessions[0]).assertEqual('chat-1');
      expect(preparedSessions[1]).assertEqual('chat-created');
      expect(activeSessions[0].sessionId).assertEqual('chat-1');
      expect(activeSessions[1].sessionId).assertEqual('chat-created');
      expect(loadedMessages[0][0].id).assertEqual('message-1');
      expect(clearComposerCount).assertEqual(1);
      expect(chatInputs[0]).assertEqual('draft text');
      expect(client.createRequests[0]).assertEqual('new prompt');
      expect(openedRoutes[0]).assertEqual('open:chat-1');
      expect(openedRoutes[1]).assertEqual('create:chat-created');
      expect(openedRoutes[2]).assertEqual('draft-cleared');
    });

    it('handles delete archive and export session commands', 0, async () => {
      const client = new FakeGeneralChatCommandClient();
      const sessions: RemoteSession[][] = [];
      const statuses: string[] = [];
      const busyEvents: boolean[] = [];
      const toasts: string[] = [];
      const exportedText: string[] = [];
      const session = remoteSession('chat-1', 'Chat 1');
      session.agentType = 'chat';
      client.currentSessions = [session];
      client.messagesBySession.set('chat-1', [chatMessage('message-1', 'user', 'Hello')]);
      const controller = new GeneralChatCommandController(client, {
        onSessions: (items: RemoteSession[]) => {
          sessions.push(items);
        },
        onSessionPrepared: (_sessionId: string) => {},
        onActiveSession: (_activeSession: SessionSummary) => {},
        onMessagesLoaded: (_messages: ChatMessage[]) => {},
        onClearComposer: () => {},
        onChatInput: (_text: string) => {},
        onStatusText: (statusText: string) => {
          statuses.push(statusText);
        },
        onBusy: (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        onToast: (statusText: string) => {
          toasts.push(statusText);
        }
      });

      await controller.archiveSession(session, true, false);
      await controller.exportSession(session, false, async (text: string): Promise<void> => {
        exportedText.push(text);
      });
      await controller.deleteSession(session, false);

      expect(client.archiveRequests[0]).assertEqual('chat-1:1');
      expect(client.messageRequests[0]).assertEqual('chat-1');
      expect(exportedText[0].indexOf('Hello') >= 0).assertTrue();
      expect(client.deleteRequests[0]).assertEqual('chat-1');
      expect(sessions.length).assertEqual(2);
      expect(sessions[sessions.length - 1].length).assertEqual(0);
      expect(statuses.indexOf(RemoteI18n.t('status.sessionArchived')) >= 0).assertTrue();
      expect(statuses.indexOf(RemoteI18n.t('status.sessionExported')) >= 0).assertTrue();
      expect(statuses[statuses.length - 1]).assertEqual(RemoteI18n.t('status.sessionDeleted'));
      expect(toasts.length).assertEqual(2);
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[busyEvents.length - 1]).assertFalse();
    });

    it('proxies send cancel and assistant persistence through the command boundary', 0, async () => {
      const client = new FakeGeneralChatCommandClient();
      const sessions: RemoteSession[][] = [];
      const userMessages: ChatMessage[] = [];
      const deltas: string[] = [];
      const controller = new GeneralChatCommandController(client, {
        onSessions: (items: RemoteSession[]) => {
          sessions.push(items);
        },
        onSessionPrepared: (_sessionId: string) => {},
        onActiveSession: (_activeSession: SessionSummary) => {},
        onMessagesLoaded: (_messages: ChatMessage[]) => {},
        onClearComposer: () => {},
        onChatInput: (_text: string) => {},
        onStatusText: (_statusText: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onToast: (_statusText: string) => {}
      });

      const result = await controller.sendMessage('chat-1', 'Hello', [selectedImage('image-1')],
        new GeneralChatStreamCallbacks(
          async (message: ChatMessage): Promise<void> => {
            userMessages.push(message);
          },
          (delta: string): void => {
            deltas.push(delta);
          }
        ));
      const recorded = await controller.recordAssistantMessage('chat-1', result.assistantMessage);
      controller.cancelCurrentRequest();

      expect(client.sendRequests[0]).assertEqual('chat-1:Hello:1');
      expect(userMessages[0].id).assertEqual('sent-user');
      expect(deltas.join('')).assertEqual('Echo: Hello');
      expect(recorded).assertTrue();
      expect(client.recordedAssistantMessages[0]).assertEqual('chat-1:sent-assistant');
      expect(sessions.length).assertEqual(1);
      expect(client.cancelCount).assertEqual(1);
    });

    it('renames active sessions and prepares retry message state', 0, async () => {
      const client = new FakeGeneralChatCommandClient();
      const activeSessions: SessionSummary[] = [];
      const loadedMessages: ChatMessage[][] = [];
      const chatInputs: string[] = [];
      const statuses: string[] = [];
      const session = remoteSession('chat-1', 'Old title');
      session.agentType = 'chat';
      client.currentSessions = [session];
      client.messagesBySession.set('chat-1', [chatMessage('assistant-1', 'assistant', 'Recovered')]);
      const controller = new GeneralChatCommandController(client, {
        onSessions: (_items: RemoteSession[]) => {},
        onSessionPrepared: (_sessionId: string) => {},
        onActiveSession: (activeSession: SessionSummary) => {
          activeSessions.push(activeSession);
        },
        onMessagesLoaded: (messages: ChatMessage[]) => {
          loadedMessages.push(messages);
        },
        onClearComposer: () => {},
        onChatInput: (text: string) => {
          chatInputs.push(text);
        },
        onStatusText: (statusText: string) => {
          statuses.push(statusText);
        },
        onBusy: (_isBusy: boolean) => {},
        onToast: (_statusText: string) => {}
      });

      const renamed = await controller.renameActiveSession({
        sessionId: 'chat-1',
        title: 'Old title',
        workspacePath: '',
        agentType: 'chat',
        initialTurnId: 'turn-0'
      }, ' New title ');
      const prepared = await controller.retryMessage('chat-1', 'Retry text', false);

      expect(renamed).assertTrue();
      expect(client.renameRequests[0]).assertEqual('chat-1:New title');
      expect(activeSessions[0].title).assertEqual('New title');
      expect(activeSessions[0].initialTurnId || '').assertEqual('turn-0');
      expect(statuses[0]).assertEqual(RemoteI18n.t('status.titleUpdated'));
      expect(prepared).assertTrue();
      expect(client.removeFailedRequests[0]).assertEqual('chat-1:Retry text');
      expect(loadedMessages[0][0].id).assertEqual('assistant-1');
      expect(chatInputs[0]).assertEqual('Retry text');
    });
  });

  describe('RemoteFileDownloadController', () => {
    it('owns remote file download progress and delayed downloading marker cleanup', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      const scheduler = new FakeRemoteFileDownloadScheduler();
      const status = new RemoteDownloadStatusRecord();
      const busyEvents: boolean[] = [];
      let statusText = '';
      const controller = new RemoteFileDownloadController(
        client,
        (downloadingFilePath: string, downloadedFilePath: string, fileDownloadStatus: string) => {
          status.downloadingFilePath = downloadingFilePath;
          status.downloadedFilePath = downloadedFilePath;
          status.fileDownloadStatus = fileDownloadStatus;
        },
        () => {
          status.downloadingFilePath = '';
        },
        (nextStatusText: string) => {
          statusText = nextStatusText;
        },
        (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        100,
        scheduler
      );

      await controller.download('/workspace/BitFun/README.md', 'session-1', false, true);

      expect(client.infoRequests[0]).assertEqual('/workspace/BitFun/README.md|session-1');
      expect(client.readRequests[0]).assertEqual('/workspace/BitFun/README.md|session-1');
      expect(status.downloadedFilePath).assertEqual('/workspace/BitFun/README.md');
      expect(status.fileDownloadStatus.indexOf('README.md') >= 0).assertTrue();
      expect(statusText).assertEqual(status.fileDownloadStatus);
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
      expect(controller.isClearPending()).assertTrue();
      expect(scheduler.delays.get(1)).assertEqual(100);

      scheduler.tick(1);
      expect(status.downloadingFilePath).assertEqual('');
      expect(status.downloadedFilePath).assertEqual('/workspace/BitFun/README.md');
    });

    it('skips unavailable downloads before touching the client', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      const scheduler = new FakeRemoteFileDownloadScheduler();
      const status = new RemoteDownloadStatusRecord();
      const controller = new RemoteFileDownloadController(
        client,
        (downloadingFilePath: string, downloadedFilePath: string, fileDownloadStatus: string) => {
          status.downloadingFilePath = downloadingFilePath;
          status.downloadedFilePath = downloadedFilePath;
          status.fileDownloadStatus = fileDownloadStatus;
        },
        () => {
          status.downloadingFilePath = '';
        },
        (_statusText: string) => {},
        (_isBusy: boolean) => {},
        100,
        scheduler
      );

      await controller.download('', 'session-1', false, true);
      await controller.download('/workspace/BitFun/README.md', '', false, true);
      await controller.download('/workspace/BitFun/README.md', 'session-1', true, true);
      await controller.download('/workspace/BitFun/README.md', 'session-1', false, false);

      expect(client.infoRequests.length).assertEqual(0);
      expect(client.readRequests.length).assertEqual(0);
      expect(status.fileDownloadStatus).assertEqual('');
    });
  });

  describe('RemoteToolActionController', () => {
    it('owns remote tool action status, busy, and poll callbacks', 0, async () => {
      const client = new FakeRemoteToolActionClient();
      const busyEvents: boolean[] = [];
      const statuses: string[] = [];
      let pollCount = 0;
      const controller = new RemoteToolActionController(
        client,
        (statusText: string) => {
          statuses.push(statusText);
        },
        (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        () => {
          pollCount += 1;
        }
      );

      const updatedInput: ToolInputFixture = { safe: true };
      await controller.approve('tool-1', 'session-1', false, true, updatedInput);
      await controller.reject('tool-2', 'session-1', false, true);
      await controller.cancel('tool-3', 'session-1', false, true);
      await controller.answer('tool-4', 'session-1', false, true, { answer: 'yes', '0': 'yes' });

      expect(client.actions[0]).assertEqual('confirm:tool-1:{"safe":true}');
      expect(client.actions[1]).assertEqual('reject:tool-2:Rejected from HarmonyOS remote client');
      expect(client.actions[2]).assertEqual('cancel:tool-3:Cancelled from HarmonyOS remote client');
      expect(client.actions[3].indexOf('answer:tool-4:') === 0).assertTrue();
      expect(client.actions[3].indexOf('"answer":"yes"') >= 0).assertTrue();
      expect(client.actions[3].indexOf('"0":"yes"') >= 0).assertTrue();
      expect(pollCount).assertEqual(4);
      expect(busyEvents.length).assertEqual(8);
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
      expect(statuses.indexOf(RemoteI18n.t('status.approvingTool')) >= 0).assertTrue();
      expect(statuses.indexOf(RemoteI18n.t('status.toolApproved')) >= 0).assertTrue();
    });

    it('skips tool actions when route state cannot run them', 0, async () => {
      const client = new FakeRemoteToolActionClient();
      const busyEvents: boolean[] = [];
      let pollCount = 0;
      const controller = new RemoteToolActionController(
        client,
        (_statusText: string) => {},
        (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        () => {
          pollCount += 1;
        }
      );

      await controller.approve('', 'session-1', false, true);
      await controller.approve('tool-1', '', false, true);
      await controller.approve('tool-1', 'session-1', true, true);
      await controller.approve('tool-1', 'session-1', false, false);

      expect(client.actions.length).assertEqual(0);
      expect(busyEvents.length).assertEqual(0);
      expect(pollCount).assertEqual(0);
    });

    it('reports failed tool actions and clears busy without polling', 0, async () => {
      const client = new FakeRemoteToolActionClient();
      client.shouldFail = true;
      const busyEvents: boolean[] = [];
      const statuses: string[] = [];
      let pollCount = 0;
      const controller = new RemoteToolActionController(
        client,
        (statusText: string) => {
          statuses.push(statusText);
        },
        (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        () => {
          pollCount += 1;
        }
      );

      await controller.reject('tool-1', 'session-1', false, true);

      expect(client.actions.length).assertEqual(1);
      expect(pollCount).assertEqual(0);
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
      expect(statuses[statuses.length - 1].indexOf('Expected tool action failure') >= 0).assertTrue();
    });
  });

  describe('RemoteModelController', () => {
    it('loads model catalog and applies the saved preferred model when still current', 0, async () => {
      const client = new FakeRemoteModelClient();
      const preferences = new FakeRemoteModelPreferenceStore();
      const resets: RemoteModelProjectionRecord[] = [];
      const applied: RemoteModelProjectionRecord[] = [];
      const controller = new RemoteModelController(
        client,
        preferences,
        (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
          const record = new RemoteModelProjectionRecord();
          record.version = knownModelCatalogVersion;
          record.selectedModelId = selectedModelId;
          record.sessionModelId = modelCatalog.session_model_id || '';
          record.modelCount = modelCatalog.models.length;
          resets.push(record);
        },
        (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
          const record = new RemoteModelProjectionRecord();
          record.version = knownModelCatalogVersion;
          record.selectedModelId = selectedModelId;
          record.sessionModelId = modelCatalog.session_model_id || '';
          record.modelCount = modelCatalog.models.length;
          applied.push(record);
        },
        (_statusText: string) => {},
        (_isBusy: boolean) => {}
      );

      controller.setPreferredModelId('model-b');
      await controller.loadCatalog('session-1', true, (sessionId: string) => sessionId === 'session-1');

      expect(client.catalogRequests[0]).assertEqual('session-1');
      expect(resets.length).assertEqual(1);
      expect(resets[0].modelCount).assertEqual(0);
      expect(applied.length).assertEqual(1);
      expect(applied[0].version).assertEqual(2);
      expect(applied[0].selectedModelId).assertEqual('model-b');
      expect(applied[0].modelCount).assertEqual(2);
    });

    it('ignores stale catalog loads after clearing the visible projection', 0, async () => {
      const client = new FakeRemoteModelClient();
      const preferences = new FakeRemoteModelPreferenceStore();
      let resetCount = 0;
      let appliedCount = 0;
      const controller = new RemoteModelController(
        client,
        preferences,
        (_modelCatalog: RemoteModelCatalog, _selectedModelId: string, _knownModelCatalogVersion: number) => {
          resetCount += 1;
        },
        (_modelCatalog: RemoteModelCatalog, _selectedModelId: string, _knownModelCatalogVersion: number) => {
          appliedCount += 1;
        },
        (_statusText: string) => {},
        (_isBusy: boolean) => {}
      );

      await controller.loadCatalog('session-1', true, (_sessionId: string) => false);

      expect(client.catalogRequests.length).assertEqual(1);
      expect(resetCount).assertEqual(1);
      expect(appliedCount).assertEqual(0);
    });

    it('selects and persists a session model through the model client', 0, async () => {
      const client = new FakeRemoteModelClient();
      const preferences = new FakeRemoteModelPreferenceStore();
      const statuses: string[] = [];
      const busyEvents: boolean[] = [];
      const applied: RemoteModelProjectionRecord[] = [];
      const controller = new RemoteModelController(
        client,
        preferences,
        (_modelCatalog: RemoteModelCatalog, _selectedModelId: string, _knownModelCatalogVersion: number) => {},
        (modelCatalog: RemoteModelCatalog, selectedModelId: string, knownModelCatalogVersion: number) => {
          const record = new RemoteModelProjectionRecord();
          record.version = knownModelCatalogVersion;
          record.selectedModelId = selectedModelId;
          record.sessionModelId = modelCatalog.session_model_id || '';
          record.modelCount = modelCatalog.models.length;
          applied.push(record);
        },
        (statusText: string) => {
          statuses.push(statusText);
        },
        (isBusy: boolean) => {
          busyEvents.push(isBusy);
        }
      );

      controller.applyCatalog(remoteModelCatalog());
      await controller.selectModel('model-b', 'session-1', false, true);

      expect(client.modelSelections[0]).assertEqual('session-1:model-b');
      expect(preferences.savedModelIds[0]).assertEqual('model-b');
      expect(applied[applied.length - 1].selectedModelId).assertEqual('model-b');
      expect(applied[applied.length - 1].sessionModelId).assertEqual('model-b');
      expect(statuses[0]).assertEqual(RemoteI18n.t('status.switchingModel'));
      expect(statuses[1]).assertEqual(RemoteI18n.t('status.modelSwitched'));
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
    });

    it('skips model selection guards and reports selection failures', 0, async () => {
      const client = new FakeRemoteModelClient();
      const preferences = new FakeRemoteModelPreferenceStore();
      const statuses: string[] = [];
      const busyEvents: boolean[] = [];
      const controller = new RemoteModelController(
        client,
        preferences,
        (_modelCatalog: RemoteModelCatalog, _selectedModelId: string, _knownModelCatalogVersion: number) => {},
        (_modelCatalog: RemoteModelCatalog, _selectedModelId: string, _knownModelCatalogVersion: number) => {},
        (statusText: string) => {
          statuses.push(statusText);
        },
        (isBusy: boolean) => {
          busyEvents.push(isBusy);
        }
      );

      await controller.selectModel('', 'session-1', false, true);
      await controller.selectModel('model-a', '', false, true);
      await controller.selectModel('model-a', 'session-1', true, true);
      await controller.selectModel('model-a', 'session-1', false, false);
      expect(client.modelSelections.length).assertEqual(0);
      expect(busyEvents.length).assertEqual(0);

      client.shouldFailSelect = true;
      await controller.selectModel('model-a', 'session-1', false, true);
      expect(client.modelSelections.length).assertEqual(1);
      expect(preferences.savedModelIds.length).assertEqual(0);
      expect(statuses[statuses.length - 1].indexOf('Expected model selection failure') >= 0).assertTrue();
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
    });
  });

  describe('RemoteSessionController', () => {
    it('refreshes session list and restores connected projection', 0, async () => {
      const client = new FakeRemoteSessionClient();
      const projection = new RemoteSessionProjectionRecord();
      const statuses: string[] = [];
      const loadingEvents: boolean[] = [];
      let reconnectCount = 0;
      let connectedCount = 0;
      let heartbeatCount = 0;
      client.listResults = [{
        sessions: [remoteSession('session-1', 'Session 1'), remoteSession('session-2', 'Session 2')],
        hasMore: true
      }];
      const controller = new RemoteSessionController(client, 8, {
        onSessions: (sessions: RemoteSession[], hasMore: boolean) => {
          projection.sessions = sessions;
          projection.hasMore = hasMore;
        },
        onActiveSession: (_session: SessionSummary) => {},
        onStatusText: (statusText: string) => {
          statuses.push(statusText);
        },
        onBusy: (_isBusy: boolean) => {},
        onLoading: (isLoading: boolean) => {
          loadingEvents.push(isLoading);
        },
        onSessionError: (_errorText: string) => {},
        onReconnecting: () => {
          reconnectCount += 1;
        },
        onConnected: () => {
          connectedCount += 1;
        },
        onConnectionFailed: (_err: Object) => {},
        onStartHeartbeat: () => {
          heartbeatCount += 1;
        }
      });

      await controller.refresh('repo', 'code', true, false);

      expect(client.listRequests[0]).assertEqual('8:0:repo:code');
      expect(projection.sessions.length).assertEqual(2);
      expect(projection.hasMore).assertTrue();
      expect(loadingEvents[0]).assertTrue();
      expect(loadingEvents[1]).assertFalse();
      expect(reconnectCount).assertEqual(1);
      expect(connectedCount).assertEqual(1);
      expect(heartbeatCount).assertEqual(1);
      expect(statuses[0]).assertEqual(RemoteI18n.t('status.refreshSessions'));
      expect(statuses[statuses.length - 1]).assertEqual(RemoteI18n.t('connection.connected'));
    });

    it('loads more sessions from current offset and keeps existing duplicates stable', 0, async () => {
      const client = new FakeRemoteSessionClient();
      const projection = new RemoteSessionProjectionRecord();
      const busyEvents: boolean[] = [];
      client.listResults = [{
        sessions: [remoteSession('session-1', 'Updated 1'), remoteSession('session-2', 'Session 2')],
        hasMore: false
      }];
      const controller = new RemoteSessionController(client, 2, {
        onSessions: (sessions: RemoteSession[], hasMore: boolean) => {
          projection.sessions = sessions;
          projection.hasMore = hasMore;
        },
        onActiveSession: (_session: SessionSummary) => {},
        onStatusText: (_statusText: string) => {},
        onBusy: (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        onLoading: (_isLoading: boolean) => {},
        onSessionError: (_errorText: string) => {},
        onReconnecting: () => {},
        onConnected: () => {},
        onConnectionFailed: (_err: Object) => {},
        onStartHeartbeat: () => {}
      });
      controller.setSessions([remoteSession('session-1', 'Session 1')], true);

      await controller.loadMore('', 'all', false, true);

      expect(client.listRequests[0]).assertEqual('2:1::all');
      expect(projection.sessions.length).assertEqual(2);
      expect(projection.sessions[0].title).assertEqual('Session 1');
      expect(projection.sessions[1].id).assertEqual('session-2');
      expect(projection.hasMore).assertFalse();
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
    });

    it('creates and opens sessions through active-session callbacks', 0, async () => {
      const client = new FakeRemoteSessionClient();
      const activeSessions: SessionSummary[] = [];
      const openedSessions: SessionSummary[] = [];
      const controller = new RemoteSessionController(client, 8, {
        onSessions: (_sessions: RemoteSession[], _hasMore: boolean) => {},
        onActiveSession: (session: SessionSummary) => {
          activeSessions.push(session);
        },
        onStatusText: (_statusText: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onLoading: (_isLoading: boolean) => {},
        onSessionError: (_errorText: string) => {},
        onReconnecting: () => {},
        onConnected: () => {},
        onConnectionFailed: (_err: Object) => {},
        onStartHeartbeat: () => {}
      });

      await controller.create('code', false, true, async (session: SessionSummary): Promise<void> => {
        openedSessions.push(session);
      });
      await controller.open(remoteSession('opened-session', 'Opened Session'), '/workspace', false, true,
        async (session: SessionSummary): Promise<void> => {
          openedSessions.push(session);
        });

      expect(client.createRequests[0]).assertEqual('code::');
      expect(activeSessions.length).assertEqual(2);
      expect(activeSessions[0].sessionId).assertEqual('created-session');
      expect(activeSessions[1].sessionId).assertEqual('opened-session');
      expect(activeSessions[1].workspacePath).assertEqual('/workspace');
      expect(openedSessions.length).assertEqual(2);
    });

    it('deletes active sessions and backfills list when more pages exist', 0, async () => {
      const client = new FakeRemoteSessionClient();
      const projection = new RemoteSessionProjectionRecord();
      let deletedActiveCount = 0;
      client.listResults = [{
        sessions: [remoteSession('session-3', 'Session 3')],
        hasMore: false
      }];
      const controller = new RemoteSessionController(client, 2, {
        onSessions: (sessions: RemoteSession[], hasMore: boolean) => {
          projection.sessions = sessions;
          projection.hasMore = hasMore;
        },
        onActiveSession: (_session: SessionSummary) => {},
        onStatusText: (_statusText: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onLoading: (_isLoading: boolean) => {},
        onSessionError: (_errorText: string) => {},
        onReconnecting: () => {},
        onConnected: () => {},
        onConnectionFailed: (_err: Object) => {},
        onStartHeartbeat: () => {}
      });
      controller.setSessions([
        remoteSession('session-1', 'Session 1'),
        remoteSession('session-2', 'Session 2')
      ], true);

      await controller.delete(
        remoteSession('session-1', 'Session 1'),
        'session-1',
        false,
        true,
        '',
        'all',
        async (): Promise<void> => {
          deletedActiveCount += 1;
        }
      );

      expect(client.deleteRequests[0]).assertEqual('session-1');
      expect(client.listRequests[0]).assertEqual('2:1::all');
      expect(deletedActiveCount).assertEqual(1);
      expect(projection.sessions.length).assertEqual(2);
      expect(projection.sessions[0].id).assertEqual('session-2');
      expect(projection.sessions[1].id).assertEqual('session-3');
      expect(projection.hasMore).assertFalse();
    });
  });

  describe('RemoteChatCommandController', () => {
    it('loads active messages and ignores stale route projections', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const projection = new RemoteChatMessagesProjectionRecord();
      const statuses: string[] = [];
      client.messageResults = [{
        messages: [chatMessage('message-1', 'assistant', 'Hello')],
        hasMore: false
      }, {
        messages: [chatMessage('message-2', 'assistant', 'Stale')],
        hasMore: false
      }];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => {
          projection.messages = messages;
          projection.hasMoreMessages = hasMoreMessages;
        },
        onMessageCountKnown: (pollVersion: number, knownMessageCount: number) => {
          projection.pollVersion = pollVersion;
          projection.knownMessageCount = knownMessageCount;
        },
        onSendSucceeded: (_turnId: string, _pendingActiveId: string) => {},
        onSendFailed: (
          _rawText: string,
          _images: SelectedImageAttachment[],
          _localMessageId: string,
          _pendingActiveId: string
        ) => {},
        onActiveSession: (_session: SessionSummary) => {},
        onSessionTitleChanged: (_sessionId: string, _title: string) => {},
        onStatusText: (statusText: string) => {
          statuses.push(statusText);
        },
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      });

      await controller.loadMessages('session-1', (_sessionId: string) => true);
      await controller.loadMessages('session-2', (_sessionId: string) => false);

      expect(client.messageRequests[0]).assertEqual('session-1');
      expect(client.messageRequests[1]).assertEqual('session-2');
      expect(projection.messages.length).assertEqual(1);
      expect(projection.messages[0].id).assertEqual('message-1');
      expect(projection.hasMoreMessages).assertFalse();
      expect(projection.pollVersion).assertEqual(0);
      expect(projection.knownMessageCount).assertEqual(1);
      expect(statuses[0]).assertEqual(RemoteI18n.t('status.loadMessages'));
      expect(statuses[1]).assertEqual(RemoteI18n.t('status.messagesSynced'));
    });

    it('loads older messages with busy and cursor callbacks', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const projection = new RemoteChatMessagesProjectionRecord();
      const busyEvents: boolean[] = [];
      client.messageResults = [{
        messages: [
          chatMessage('message-1', 'user', 'Older'),
          chatMessage('message-2', 'assistant', 'Reply')
        ],
        hasMore: false
      }];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => {
          projection.messages = messages;
          projection.hasMoreMessages = hasMoreMessages;
        },
        onMessageCountKnown: (pollVersion: number, knownMessageCount: number) => {
          projection.pollVersion = pollVersion;
          projection.knownMessageCount = knownMessageCount;
        },
        onSendSucceeded: (_turnId: string, _pendingActiveId: string) => {},
        onSendFailed: (
          _rawText: string,
          _images: SelectedImageAttachment[],
          _localMessageId: string,
          _pendingActiveId: string
        ) => {},
        onActiveSession: (_session: SessionSummary) => {},
        onSessionTitleChanged: (_sessionId: string, _title: string) => {},
        onStatusText: (_statusText: string) => {},
        onBusy: (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        onPollRequested: () => {}
      });

      await controller.loadOlderMessages('session-1', 7, true, false);

      expect(client.messageRequests[0]).assertEqual('session-1');
      expect(projection.messages.length).assertEqual(2);
      expect(projection.hasMoreMessages).assertFalse();
      expect(projection.pollVersion).assertEqual(7);
      expect(projection.knownMessageCount).assertEqual(2);
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
    });

    it('sends prepared remote messages and reports failed payloads', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const succeeded: string[] = [];
      const failed: string[] = [];
      const busyEvents: boolean[] = [];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onSendSucceeded: (turnId: string, pendingActiveId: string) => {
          succeeded.push(`${turnId}:${pendingActiveId}`);
        },
        onSendFailed: (
          rawText: string,
          images: SelectedImageAttachment[],
          localMessageId: string,
          pendingActiveId: string
        ) => {
          failed.push(`${rawText}:${images.length}:${localMessageId}:${pendingActiveId}`);
        },
        onActiveSession: (_session: SessionSummary) => {},
        onSessionTitleChanged: (_sessionId: string, _title: string) => {},
        onStatusText: (_statusText: string) => {},
        onBusy: (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        onPollRequested: () => {}
      });

      await controller.sendPreparedMessage(
        'session-1',
        'Describe image',
        'code',
        'Describe image',
        [selectedImage('image-1')],
        [{
          id: 'image-1',
          data_url: 'data:image/png;base64,AA==',
          mime_type: 'image/png'
        }],
        'local-1',
        'pending-1',
        false,
        true
      );
      client.shouldFailSend = true;
      await controller.sendPreparedMessage(
        'session-1',
        'Retry text',
        'code',
        'Retry text',
        [],
        [],
        'local-2',
        'pending-2',
        false,
        true
      );

      expect(client.sendRequests[0]).assertEqual('images:session-1:Describe image:code:1');
      expect(client.sendRequests[1]).assertEqual('text:session-1:Retry text:code');
      expect(succeeded[0]).assertEqual('turn-1:pending-1');
      expect(failed[0]).assertEqual('Retry text:0:local-2:pending-2');
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
      expect(busyEvents[2]).assertTrue();
      expect(busyEvents[3]).assertFalse();
    });

    it('stops active tasks and reports guard states', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const statuses: string[] = [];
      let pollCount = 0;
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onSendSucceeded: (_turnId: string, _pendingActiveId: string) => {},
        onSendFailed: (
          _rawText: string,
          _images: SelectedImageAttachment[],
          _localMessageId: string,
          _pendingActiveId: string
        ) => {},
        onActiveSession: (_session: SessionSummary) => {},
        onSessionTitleChanged: (_sessionId: string, _title: string) => {},
        onStatusText: (statusText: string) => {
          statuses.push(statusText);
        },
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {
          pollCount += 1;
        }
      });

      await controller.stopTask('session-1', '', '', true);
      await controller.stopTask('session-1', 'active-turn', 'turn-1', true);

      expect(client.cancelRequests[0]).assertEqual('session-1:turn-1');
      expect(statuses[0]).assertEqual(RemoteI18n.t('status.noRunningTask'));
      expect(statuses[1]).assertEqual(RemoteI18n.t('status.stoppingTask'));
      expect(statuses[2]).assertEqual(RemoteI18n.t('status.stopRequested'));
      expect(pollCount).assertEqual(1);
    });

    it('renames active sessions and updates active/list projections', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const activeSessions: SessionSummary[] = [];
      const titleUpdates: string[] = [];
      const busyEvents: boolean[] = [];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onSendSucceeded: (_turnId: string, _pendingActiveId: string) => {},
        onSendFailed: (
          _rawText: string,
          _images: SelectedImageAttachment[],
          _localMessageId: string,
          _pendingActiveId: string
        ) => {},
        onActiveSession: (session: SessionSummary) => {
          activeSessions.push(session);
        },
        onSessionTitleChanged: (sessionId: string, title: string) => {
          titleUpdates.push(`${sessionId}:${title}`);
        },
        onStatusText: (_statusText: string) => {},
        onBusy: (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        onPollRequested: () => {}
      });

      await controller.renameActiveSession({
        sessionId: 'session-1',
        title: 'Old title',
        workspacePath: '/workspace',
        agentType: 'code',
        initialTurnId: 'turn-0'
      }, '  New title  ', false, true);

      expect(client.renameRequests[0]).assertEqual('session-1:New title');
      expect(activeSessions[0].title).assertEqual('New title');
      expect(activeSessions[0].initialTurnId || '').assertEqual('turn-0');
      expect(titleUpdates[0]).assertEqual('session-1:New title');
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
    });
  });

  describe('ChatComposerCapabilities', () => {
    it('keeps route-to-surface composer ownership explicit', 0, () => {
      expect(AppRouteContract.isGeneralComposerRoute(AppRoute.ChatHome)).assertTrue();
      expect(AppRouteContract.isGeneralComposerRoute(AppRoute.GeneralChat)).assertTrue();
      expect(AppRouteContract.isGeneralComposerRoute(AppRoute.RemoteHome)).assertFalse();
      expect(AppRouteContract.isGeneralComposerRoute(AppRoute.RemoteChat)).assertFalse();
    });

    it('keeps general chat independent from remote-only composer requirements', 0, () => {
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.surface).assertEqual(ChatSurface.General);
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.supportsAttachments).assertFalse();
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.requiresRemoteConnection).assertFalse();
    });

    it('keeps remote composer attachments behind the remote connection', 0, () => {
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.surface).assertEqual(ChatSurface.Remote);
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.supportsAttachments).assertTrue();
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.requiresRemoteConnection).assertTrue();
    });
  });

  describe('ConversationViewContract', () => {
    it('keeps empty ordinary chat on the unified conversation surface with suggestions', 0, () => {
      const emptyItems = ChatTimelineProjector.project([], [], RemoteUiState.emptyActiveTurn(), false);

      expect(ConversationViewContract.surfaceForRoute(false)).assertEqual(ChatSurface.General);
      expect(ConversationViewContract.shouldShowSuggestions(true, false, emptyItems)).assertTrue();
      expect(ConversationViewContract.visibleTimelineItems(emptyItems).length).assertEqual(0);
      expect(ConversationViewContract.hasRealTimelineItem(emptyItems)).assertFalse();
    });

    it('removes suggestions on the same page once ordinary chat has real content or is sending', 0, () => {
      const emptyItems = ChatTimelineProjector.project([], [], RemoteUiState.emptyActiveTurn(), false);
      const messageItems = ChatTimelineProjector.project([
        chatMessage('general-user-1', 'user', 'Write a short intro')
      ], [], RemoteUiState.emptyActiveTurn(), false);

      expect(ConversationViewContract.shouldShowSuggestions(true, true, emptyItems)).assertFalse();
      expect(ConversationViewContract.shouldShowSuggestions(true, false, messageItems)).assertFalse();
      expect(ConversationViewContract.visibleTimelineItems(messageItems).length).assertEqual(1);
      expect(ConversationViewContract.visibleTimelineItems(messageItems)[0].type).assertEqual('user_message');
      expect(ConversationViewContract.hasRealTimelineItem(messageItems)).assertTrue();
    });

    it('keeps remote empty conversations blank instead of rendering the old empty card', 0, () => {
      const emptyItems = ChatTimelineProjector.project([], [], RemoteUiState.emptyActiveTurn(), false);

      expect(ConversationViewContract.surfaceForRoute(true)).assertEqual(ChatSurface.Remote);
      expect(ConversationViewContract.shouldShowSuggestions(false, false, emptyItems)).assertFalse();
      expect(ConversationViewContract.visibleTimelineItems(emptyItems).length).assertEqual(0);
    });
  });

  describe('AppRouteContract', () => {
    it('defines stable route ids and session-carrying route params', 0, () => {
      const generalParam = AppRouteContract.routeParam('general-session-1');
      const remoteParam = AppRouteContract.routeParam('remote-session-1');

      expect(`${AppRoute.ChatHome}`).assertEqual('ChatHome');
      expect(`${AppRoute.GeneralChat}`).assertEqual('GeneralChat');
      expect(`${AppRoute.RemoteHome}`).assertEqual('RemoteHome');
      expect(`${AppRoute.RemoteChat}`).assertEqual('RemoteChat');
      expect(AppRouteContract.isSessionRoute(AppRoute.ChatHome)).assertFalse();
      expect(AppRouteContract.isSessionRoute(AppRoute.RemoteHome)).assertFalse();
      expect(AppRouteContract.isSessionRoute(AppRoute.GeneralChat)).assertTrue();
      expect(AppRouteContract.isSessionRoute(AppRoute.RemoteChat)).assertTrue();
      expect(generalParam.sessionId).assertEqual('general-session-1');
      expect(remoteParam.sessionId).assertEqual('remote-session-1');
    });

    it('derives current route from NavPathStack path names with ChatHome as root', 0, () => {
      expect(AppRouteContract.currentRoute([])).assertEqual(AppRoute.ChatHome);
      expect(AppRouteContract.currentRoute([AppRoute.RemoteHome])).assertEqual(AppRoute.RemoteHome);
      expect(AppRouteContract.currentRoute([
        AppRoute.RemoteHome,
        AppRoute.RemoteChat
      ])).assertEqual(AppRoute.RemoteChat);
      expect(AppRouteContract.currentRoute([
        AppRoute.GeneralChat
      ])).assertEqual(AppRoute.GeneralChat);
    });

    it('builds push path specs and suppresses duplicate current routes', 0, () => {
      const duplicate = AppRouteContract.pathSpec(AppRoute.RemoteHome, AppRoute.RemoteHome, '');
      const chatHomeDuplicate = AppRouteContract.pathSpec(AppRoute.ChatHome, AppRoute.ChatHome, 'general-session-1');
      const remoteChatSpec = AppRouteContract.pathSpec(
        AppRoute.RemoteHome,
        AppRoute.RemoteChat,
        'remote-session-1'
      ) as AppNavigationPathSpec;
      const generalChatSpec = AppRouteContract.pathSpec(
        AppRoute.ChatHome,
        AppRoute.GeneralChat,
        'general-session-1'
      ) as AppNavigationPathSpec;

      expect(duplicate === undefined).assertTrue();
      expect(chatHomeDuplicate === undefined).assertTrue();
      expect(remoteChatSpec.name).assertEqual(AppRoute.RemoteChat);
      expect(remoteChatSpec.hasSessionParam()).assertTrue();
      expect(remoteChatSpec.routeParam().sessionId).assertEqual('remote-session-1');
      expect(generalChatSpec.name).assertEqual(AppRoute.GeneralChat);
      expect(generalChatSpec.routeParam().sessionId).assertEqual('general-session-1');
    });

    it('classifies back actions without depending on ArkUI NavPathStack', 0, () => {
      expect(AppRouteContract.backAction(AppRoute.ChatHome, true))
        .assertEqual(AppNavigationBackAction.CloseSidebar);
      expect(AppRouteContract.backAction(AppRoute.GeneralChat, false))
        .assertEqual(AppNavigationBackAction.CloseActiveChat);
      expect(AppRouteContract.backAction(AppRoute.RemoteChat, false))
        .assertEqual(AppNavigationBackAction.CloseActiveChat);
      expect(AppRouteContract.backAction(AppRoute.RemoteHome, false))
        .assertEqual(AppNavigationBackAction.PopRemoteHome);
      expect(AppRouteContract.backAction(AppRoute.ChatHome, false))
        .assertEqual(AppNavigationBackAction.AllowSystem);
    });
  });

  describe('ChatSessionController', () => {
    it('preserves active turn when unchanged poll omits active turn', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      manager.results = [pollResult({
        version: 1,
        changed: false,
        sessionState: 'running',
        title: 'Session',
        newMessages: [],
        totalMessageCount: 1
      })];
      const controller = new ChatSessionController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          snapshots.push(snapshot);
        },
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      controller.start('session-1', {
        pollVersion: 1,
        knownMessageCount: 1,
        knownModelCatalogVersion: 0
      }, chatMessage('active-turn-1', 'assistant', 'Working', 'active'));
      await delay(20);
      controller.stop(false);

      expect(snapshots.length).assertEqual(1);
      expect(snapshots[0].changed).assertEqual(false);
      expect(snapshots[0].activeTurn ? snapshots[0].activeTurn.id : '').assertEqual('active-turn-1');
    });

    it('keeps completed active turn until final assistant message arrives', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      const controller = new ChatSessionController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          snapshots.push(snapshot);
        },
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      manager.results = [pollResult({
        version: 2,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [],
        totalMessageCount: 1,
        activeTurn: chatMessage('active-turn-2', 'assistant', 'Final text', 'completed')
      })];
      controller.start('session-1', {
        pollVersion: 1,
        knownMessageCount: 1,
        knownModelCatalogVersion: 0
      }, chatMessage('active-turn-2', 'assistant', 'Final text', 'active'));
      await delay(20);

      manager.results = [pollResult({
        version: 3,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [],
        totalMessageCount: 1
      })];
      await controller.pollNow();

      manager.results = [pollResult({
        version: 4,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [
          chatMessage('assistant-final-1', 'assistant', 'Final text')
        ],
        totalMessageCount: 2
      })];
      await controller.pollNow();
      controller.stop(false);

      expect(snapshots.length).assertEqual(3);
      expect(snapshots[0].activeTurn ? snapshots[0].activeTurn.status : '').assertEqual('completed');
      expect(snapshots[1].activeTurn ? snapshots[1].activeTurn.id : '').assertEqual('active-turn-2');
      expect(snapshots[2].activeTurn ? snapshots[2].activeTurn.id : '').assertEqual('');
    });

    it('resyncs when a completed assistant message is updated during the settle window', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      const controller = new ChatSessionController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          snapshots.push(snapshot);
        },
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      manager.results = [pollResult({
        version: 2,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [
          chatMessage('turn-1-assistant', 'assistant', '')
        ],
        totalMessageCount: 2
      })];
      controller.start('session-1', {
        pollVersion: 1,
        knownMessageCount: 1,
        knownModelCatalogVersion: 0
      }, chatMessage('active-turn-1', 'assistant', '', 'active'));
      await delay(20);

      manager.results = [pollResult({
        version: 3,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [],
        totalMessageCount: 2
      })];
      await controller.pollNow();
      controller.stop(false);

      expect(snapshots.length).assertEqual(2);
      expect(snapshots[0].shouldSyncAfterTurnEnded).assertEqual(true);
      expect(snapshots[1].newMessages.length).assertEqual(0);
      expect(snapshots[1].shouldSyncAfterTurnEnded).assertEqual(true);
    });
  });
}
