import { describe, it, expect } from '@ohos/hypium';
import { RemoteI18n } from '../main/ets/i18n/RemoteI18n';
import { ChatSessionController, ChatSessionSnapshot } from '../main/ets/services/ChatSessionController';
import { ChatComposerPolicy } from '../main/ets/services/ChatComposerPolicy';
import { ChatTimelineItem, ChatTimelineProjector } from '../main/ets/services/ChatTimelineProjector';
import { ChatTimelineStore } from '../main/ets/services/ChatTimelineStore';
import { ConversationEvent } from '../main/ets/services/ConversationEvent';
import { AsyncLifecycleGate } from '../main/ets/services/AsyncLifecycleGate';
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
import { MarkdownParseCache, MarkdownParser } from '../main/ets/services/MarkdownParser';
import { RemoteSessionListCache } from '../main/ets/services/RemoteSessionListCache';
import {
  ToolFileInputPayload,
  ToolFileReferenceResolver
} from '../main/ets/pages/policy/ToolFileReferenceResolver';
import {
  CodeSyntaxHighlightCache,
  CodeSyntaxHighlighter,
  CodeSyntaxToken,
  CodeSyntaxTokenKind
} from '../main/ets/services/CodeSyntaxHighlighter';
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
import { FileReferenceKind, FileTargetResolver } from '../main/ets/pages/policy/FileTargetResolver';
import { FilePreviewPolicy } from '../main/ets/services/FilePreviewPolicy';
import { FilePreviewErrorPolicy } from '../main/ets/services/FilePreviewErrorPolicy';
import {
  MessageFileReferenceProjectionCache,
  MessageFileReferenceProjector
} from '../main/ets/pages/policy/MessageFileReferenceProjector';
import { RemoteFilePreviewController } from '../main/ets/pages/viewmodel/RemoteFilePreviewController';
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
import { RemoteWorkspaceCoordinator } from '../main/ets/services/RemoteWorkspaceCoordinator';
import { RemoteWorkspaceDataSource } from '../main/ets/services/RemoteWorkspaceRepository';
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
import { RemoteSessionViewModel } from '../main/ets/pages/viewmodel/RemoteSessionViewModel';
import { ConversationViewState } from '../main/ets/pages/state/ConversationViewState';
import {
  FilePreviewPhase,
  FilePreviewRendererKind,
  FilePreviewState
} from '../main/ets/pages/state/FilePreviewState';
import { FilePreviewTarget, FilePreviewTargetContext } from '../main/ets/model/FilePreviewTarget';
import {
  FilePreviewPlacement,
  FilePreviewPlacementPolicy
} from '../main/ets/pages/policy/FilePreviewPlacementPolicy';
import {
  ConversationLayoutCrease,
  ConversationLayoutHorizontalCrease,
  ConversationLayoutPolicy,
  ConversationLayoutRect
} from '../main/ets/pages/policy/ConversationLayoutPolicy';
import { SessionActionPolicy, SessionActionScope } from '../main/ets/pages/policy/SessionActionPolicy';
import { ConversationSessionFilterPolicy } from '../main/ets/pages/policy/ConversationSessionFilterPolicy';
import {
  SessionListInputs,
  SessionListProjectionCache,
  SessionListProjector
} from '../main/ets/pages/policy/SessionListProjection';
import { ConversationModelPresentationPolicy } from '../main/ets/pages/policy/ConversationModelPresentationPolicy';
import {
  GENERAL_CHAT_COMPOSER_CAPABILITIES,
  REMOTE_CHAT_COMPOSER_CAPABILITIES
} from '../main/ets/pages/state/ChatComposerCapabilities';
import { ChatSurface } from '../main/ets/pages/state/ChatSurface';
import { ConversationViewContract } from '../main/ets/pages/components/ConversationViewContract';
import {
  ConversationUiModel,
  ConversationUiModelCatalog
} from '../main/ets/pages/state/ConversationUiModels';
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
  SessionSummary,
  WorkspaceInfo
} from '../main/ets/model/RemoteModels';

import {
  ToolInputFixture,
  CryptoImageFixture,
  CryptoCommandFixture,
  CryptoSimpleCommandFixture,
  GeneralChatRecordedRequest,
  FakeRemoteWorkspaceDataSource,
  FakeRemoteCryptoCipher,
  hexToBytes,
  bytesToHex,
  fixedRandomBytes,
  expectCommandJson,
  connectedPeers,
  chatMessage,
  remoteSession,
  selectedImage,
  remoteModelCatalog,
  activeChatMessage,
  chatMessageWithImage,
  imageAttachment,
  pollResult,
  delay,
  FakePollSessionManager,
  BlockingPollSessionManager,
  FakeGeneralChatLocalStore,
  FailingGeneralChatAdapter,
  PartialFailingGeneralChatAdapter,
  FakeGeneralChatTokenProvider,
  FakeGeneralChatHttpTransport,
  FakeRemoteHeartbeatScheduler,
  FakeGeneralChatDraftScheduler,
  FakeVoiceInputLifecycleService,
  FakeRemoteFileDownloadScheduler,
  FakeRemoteFileDownloadClient,
  FakeRemoteFileSaver,
  RemoteDownloadStatusRecord,
  FakeRemoteToolActionClient,
  FakeRemoteModelClient,
  FakeRemoteModelPreferenceStore,
  RemoteModelProjectionRecord,
  FakeRemoteSessionClient,
  RemoteSessionProjectionRecord,
  FakeRemoteChatCommandClient,
  FakeRemoteChatCacheStore,
  FakeRemoteSessionListStore,
  inertRemoteChatCache,
  readyRemoteChatCache,
  readyRemoteSessionListCache,
  RemoteChatMessagesProjectionRecord,
  FakeGeneralChatCommandClient,
  FakeGeneralChatConfigStore
} from './LocalTestFixtures';

class BlockingFileInfoClient extends FakeRemoteFileDownloadClient {
  private infoResolver?: (info: FileInfo) => void;

  async getFileInfo(path: string, sessionId?: string): Promise<FileInfo> {
    this.infoRequests.push(`${path}|${sessionId || ''}`);
    return new Promise<FileInfo>((resolve: (info: FileInfo) => void) => {
      this.infoResolver = resolve;
    });
  }

  resolveFileInfo(): void {
    if (this.infoResolver) {
      const resolve = this.infoResolver;
      this.infoResolver = undefined;
      resolve(this.fileInfo);
    }
  }
}

class MultiBlockingFileInfoClient extends FakeRemoteFileDownloadClient {
  private infoResolvers: Map<string, (info: FileInfo) => void> = new Map<string, (info: FileInfo) => void>();

  async getFileInfo(path: string, sessionId?: string): Promise<FileInfo> {
    this.infoRequests.push(`${path}|${sessionId || ''}`);
    return new Promise<FileInfo>((resolve: (info: FileInfo) => void) => {
      this.infoResolvers.set(path, resolve);
    });
  }

  resolveFileInfo(path: string, info: FileInfo): void {
    const resolve = this.infoResolvers.get(path);
    if (resolve) {
      this.infoResolvers.delete(path);
      resolve(info);
    }
  }
}

export default function remoteControllersUnitTest() {
  describe('ToolFileReferenceResolver', () => {
    it('extracts structured single-file paths from supported tools', 0, () => {
      const readInput: ToolFileInputPayload = { file_path: 'src/main.rs' };
      const read = ToolFileReferenceResolver.resolve('Read', readInput);
      const edit = ToolFileReferenceResolver.resolve('edit_file', undefined,
        '{"path":"computer://src/app.ts#L12-L20"}');

      expect(read?.path || '').assertEqual('src/main.rs');
      expect(read?.label || '').assertEqual('main.rs');
      expect(edit?.path || '').assertEqual('computer://src/app.ts#L12-L20');
      expect(edit?.label || '').assertEqual('app.ts');
    });

    it('rejects commands, directory tools and unstructured previews', 0, () => {
      const fileInput: ToolFileInputPayload = { path: 'src/main.rs' };
      const directoryInput: ToolFileInputPayload = { path: 'src/' };
      expect(ToolFileReferenceResolver.resolve('Bash', fileInput) === undefined).assertTrue();
      expect(ToolFileReferenceResolver.resolve('LS', directoryInput) === undefined).assertTrue();
      expect(ToolFileReferenceResolver.resolve('Read', undefined, 'src/main.rs') === undefined).assertTrue();
    });
  });

  describe('CodeSyntaxHighlighter', () => {
    it('colors common code tokens while preserving numbered source text', 0, () => {
      const source = 'const answer = 42;\n// note\nfunction run() { return "ok"; }';
      const tokens = CodeSyntaxHighlighter.tokenize(source, 'server.js');
      const rendered = tokens.map((token: CodeSyntaxToken): string => token.text).join('');
      const kinds = tokens.map((token: CodeSyntaxToken): CodeSyntaxTokenKind => token.kind);

      expect(rendered).assertEqual('1  const answer = 42;\n2  // note\n3  function run() { return "ok"; }');
      expect(kinds.indexOf(CodeSyntaxTokenKind.Keyword) >= 0).assertTrue();
      expect(kinds.indexOf(CodeSyntaxTokenKind.Number) >= 0).assertTrue();
      expect(kinds.indexOf(CodeSyntaxTokenKind.Comment) >= 0).assertTrue();
      expect(kinds.indexOf(CodeSyntaxTokenKind.Function) >= 0).assertTrue();
      expect(kinds.indexOf(CodeSyntaxTokenKind.String) >= 0).assertTrue();
    });

    it('falls back to one plain span for bounded rendering of large files', 0, () => {
      const tokens = CodeSyntaxHighlighter.tokenize('x'.repeat(257 * 1024), 'large.rs');

      expect(tokens.length).assertEqual(1);
      expect(tokens[0].kind).assertEqual(CodeSyntaxTokenKind.Plain);
    });

    it('reuses token arrays until content or language identity changes', 0, () => {
      const cache = new CodeSyntaxHighlightCache();
      const first = cache.tokensFor('const answer = 42;', 'main.ts');
      const same = cache.tokensFor('const answer = 42;', 'main.ts');
      const renamed = cache.tokensFor('const answer = 42;', 'main.txt');
      const changed = cache.tokensFor('const answer = 43;', 'main.txt');

      expect(first === same).assertTrue();
      expect(same === renamed).assertFalse();
      expect(renamed === changed).assertFalse();
    });
  });

  describe('MarkdownParser', () => {
    it('caches parsed blocks until the Markdown source changes', 0, () => {
      const cache = new MarkdownParseCache();
      const first = cache.blocksFor('# Title\n\nBody');
      const same = cache.blocksFor('# Title\n\nBody');
      const changed = cache.blocksFor('# Title\n\nUpdated');

      expect(first === same).assertTrue();
      expect(same === changed).assertFalse();
      expect(MarkdownParser.formatInlineText('[file](README.md)')).assertEqual('file (README.md)');
    });
  });

  describe('MessageFileReferenceProjector', () => {
    it('deduplicates explicit desktop file cards by normalized target path', 0, () => {
      const references = MessageFileReferenceProjector.project(
        '[first](computer://src/main.rs#L4) and computer://src/main.rs:9, ' +
        '[second](computer://src/lib.rs). [relative](README.md) `computer://src/hidden.rs`'
      );

      expect(references.length).assertEqual(2);
      expect(references[0].remotePath).assertEqual('src/main.rs');
      expect(references[0].path).assertEqual('computer://src/main.rs#L4');
      expect(references[1].remotePath).assertEqual('src/lib.rs');
    });

    it('caps cards and reuses projections until message text changes', 0, () => {
      const source = 'computer://a.rs computer://b.rs computer://c.rs computer://d.rs computer://e.rs';
      const cache = new MessageFileReferenceProjectionCache();
      const first = cache.referencesFor(source);
      const same = cache.referencesFor(source);
      const changed = cache.referencesFor('computer://next.rs');

      expect(first.length).assertEqual(4);
      expect(first === same).assertTrue();
      expect(same === changed).assertFalse();
      expect(changed.length).assertEqual(1);
    });
  });

  describe('FileTargetResolver', () => {
    it('resolves remote file references and line ranges', 0, () => {
      const context = new FilePreviewTargetContext('session-1', '/workspace/BitFun', 4);
      const computer = FileTargetResolver.resolve('computer://src/main.rs#L42-L58', '', context);
      const relative = FileTargetResolver.resolve('README.md:12-18', 'Readme', context);
      const windows = FileTargetResolver.resolve('C:\\workspace\\main.cpp#L9', '', context);

      expect(computer.kind).assertEqual(FileReferenceKind.RemoteWorkspaceFile);
      expect(computer.target?.remotePath || '').assertEqual('src/main.rs');
      expect(computer.target?.lineStart || 0).assertEqual(42);
      expect(computer.target?.lineEnd || 0).assertEqual(58);
      expect(relative.target?.displayName || '').assertEqual('Readme');
      expect(relative.target?.lineStart || 0).assertEqual(12);
      expect(windows.target?.remotePath || '').assertEqual('C:\\workspace\\main.cpp');
      expect(windows.target?.lineStart || 0).assertEqual(9);
      expect(FileTargetResolver.matchesRemotePath('computer://src/main.rs#L42-L58', 'src/main.rs')).assertTrue();
      expect(FileTargetResolver.matchesRemotePath('src/main.rs:42', 'src/main.rs')).assertTrue();
      expect(FileTargetResolver.matchesRemotePath('src/other.rs', 'src/main.rs')).assertFalse();
    });

    it('normalizes schemes, encoded paths, extensionless files and trailing punctuation', 0, () => {
      const context = new FilePreviewTargetContext('session-1', '/workspace/BitFun', 4);
      const fileUrl = FileTargetResolver.resolve('file:///workspace/Makefile', '', context);
      const encoded = FileTargetResolver.resolve('computer://docs/My%20File.md),', '', context);
      const dockerfile = FileTargetResolver.resolve('/workspace/Dockerfile', '', context);
      const dotfile = FileTargetResolver.resolve('.env', '', context);
      const extensionless = FileTargetResolver.resolve('LICENSE', '', context);
      const repeated = FileTargetResolver.resolve('file:///workspace/Makefile', '', context);

      expect(fileUrl.target?.remotePath || '').assertEqual('/workspace/Makefile');
      expect(encoded.target?.remotePath || '').assertEqual('docs/My File.md');
      expect(dockerfile.kind).assertEqual(FileReferenceKind.RemoteWorkspaceFile);
      expect(dotfile.kind).assertEqual(FileReferenceKind.RemoteWorkspaceFile);
      expect(extensionless.kind).assertEqual(FileReferenceKind.RemoteWorkspaceFile);
      expect(repeated.target?.remotePath || '').assertEqual(fileUrl.target?.remotePath || '');
    });

    it('classifies web, anchor and unsupported references without file targets', 0, () => {
      const context = new FilePreviewTargetContext('session-1', '/workspace/BitFun', 1);
      expect(FileTargetResolver.resolve('https://example.com', '', context).kind)
        .assertEqual(FileReferenceKind.HttpUrl);
      expect(FileTargetResolver.resolve('http://example.com', '', context).kind)
        .assertEqual(FileReferenceKind.HttpUrl);
      expect(FileTargetResolver.resolve('#section', '', context).kind)
        .assertEqual(FileReferenceKind.Anchor);
      expect(FileTargetResolver.resolve('mailto:test@example.com', '', context).kind)
        .assertEqual(FileReferenceKind.UnsupportedScheme);
      expect(FileTargetResolver.resolve('', '', context).kind)
        .assertEqual(FileReferenceKind.Invalid);
    });
  });

  describe('FilePreviewState', () => {
    it('targets linked lines and restores the last user scroll offset', 0, () => {
      const state = new FilePreviewState();
      state.begin(new FilePreviewTarget(
        'src/main.rs#L42-L48', 'src/main.rs', 'main.rs', 'session-1', '/workspace', 1, 42, 48
      ));

      expect(state.initialScrollY()).assertEqual(741);
      state.recordScroll(15, 380);
      expect(state.initialScrollX()).assertEqual(15);
      expect(state.initialScrollY()).assertEqual(380);

      state.begin(new FilePreviewTarget(
        'src/lib.rs', 'src/lib.rs', 'lib.rs', 'session-1', '/workspace', 1
      ));
      expect(state.initialScrollY()).assertEqual(0);
    });
  });

  describe('RemoteFilePreviewController', () => {
    it('centralizes bounded text and image size decisions in FilePreviewPolicy', 0, () => {
      expect(FilePreviewPolicy.textReadLimit(128)).assertEqual(128);
      expect(FilePreviewPolicy.textReadLimit(0)).assertEqual(2 * 1024 * 1024);
      expect(FilePreviewPolicy.textReadLimit(3 * 1024 * 1024)).assertEqual(2 * 1024 * 1024);
      expect(FilePreviewPolicy.canPreviewImage(12 * 1024 * 1024)).assertTrue();
      expect(FilePreviewPolicy.canPreviewImage(12 * 1024 * 1024 + 1)).assertFalse();
    });

    it('maps file protocol failures to localized preview errors', 0, () => {
      const missing = FilePreviewErrorPolicy.resolve(new Error('File not found: /workspace/missing.md'));
      const unresolved = FilePreviewErrorPolicy.resolve(new Error('Remote file path could not be resolved: missing.md'));
      const denied = FilePreviewErrorPolicy.resolve(new Error('Path outside workspace is restricted'));
      const tooLarge = FilePreviewErrorPolicy.resolve(new Error('File too large (20 bytes, limit 10 bytes)'));
      const unknown = FilePreviewErrorPolicy.resolve(new Error('Unexpected backend detail'));

      expect(missing.text).assertEqual(RemoteI18n.t('filePreview.notFound'));
      expect(missing.retryable).assertTrue();
      expect(unresolved.text).assertEqual(RemoteI18n.t('filePreview.unavailable'));
      expect(unresolved.retryable).assertTrue();
      expect(denied.text).assertEqual(RemoteI18n.t('filePreview.accessDenied'));
      expect(denied.retryable).assertFalse();
      expect(tooLarge.text).assertEqual(RemoteI18n.t('filePreview.tooLarge'));
      expect(tooLarge.retryable).assertFalse();
      expect(unknown.text).assertEqual(RemoteI18n.t('filePreview.loadFailed'));
      expect(unknown.retryable).assertTrue();
    });

    it('loads a bounded text preview without global busy state', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      client.fileInfo = { name: 'main.rs', size: 16, mimeType: 'text/plain' };
      client.chunkResult = {
        name: 'main.rs',
        contentBase64: Encoding.bytesToBase64(Encoding.utf8ToBytes('fn main() {}')),
        offset: 0,
        chunkSize: 12,
        totalSize: 16,
        mimeType: 'text/plain'
      };
      const state = new FilePreviewState();
      const controller = new RemoteFilePreviewController(client, state, () => true, () => 3);
      const target = new FilePreviewTarget(
        'computer://src/main.rs', 'src/main.rs', 'main.rs', 'session-1', '/workspace/BitFun', 3
      );

      await controller.open(target);

      expect(state.phase).assertEqual(FilePreviewPhase.Ready);
      expect(state.rendererKind).assertEqual(FilePreviewRendererKind.Text);
      expect(state.textContent).assertEqual('fn main() {}');
      expect(state.truncated).assertTrue();
      expect(client.infoRequests[0]).assertEqual('src/main.rs|session-1');
      expect(client.readRequests[0].indexOf('src/main.rs|session-1|0|16') === 0).assertTrue();
    });

    it('does not transfer unsupported binary files', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      client.fileInfo = { name: 'archive.zip', size: 2048, mimeType: 'application/zip' };
      const state = new FilePreviewState();
      const controller = new RemoteFilePreviewController(client, state, () => true, () => 2);

      await controller.open(new FilePreviewTarget(
        'archive.zip', 'archive.zip', 'archive.zip', 'session-1', '/workspace', 2
      ));

      expect(state.phase).assertEqual(FilePreviewPhase.Unsupported);
      expect(state.rendererKind).assertEqual(FilePreviewRendererKind.Unsupported);
      expect(client.readRequests.length).assertEqual(0);
    });

    it('uses a bounded default range for unknown text sizes and rejects invalid UTF-8', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      client.fileInfo = { name: 'unknown.txt', size: 0, mimeType: 'text/plain' };
      client.chunkResult = {
        name: 'unknown.txt',
        contentBase64: Encoding.bytesToBase64(new Uint8Array([0xC3, 0x28])),
        offset: 0,
        chunkSize: 2,
        totalSize: 0,
        mimeType: 'text/plain'
      };
      const state = new FilePreviewState();
      const controller = new RemoteFilePreviewController(client, state, () => true, () => 2);

      await controller.open(new FilePreviewTarget(
        'unknown.txt', 'unknown.txt', 'unknown.txt', 'session-1', '/workspace', 2
      ));

      expect(state.phase).assertEqual(FilePreviewPhase.Unsupported);
      expect(client.readRequests[0].indexOf('|0|2097152') >= 0).assertTrue();
    });

    it('rejects an image when returned content exceeds the preview limit', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      client.fileInfo = { name: 'changed.png', size: 1024, mimeType: 'image/png' };
      client.fileReadResult = {
        name: 'changed.png',
        contentBase64: '',
        mimeType: 'image/png',
        size: 13 * 1024 * 1024
      };
      const state = new FilePreviewState();
      const controller = new RemoteFilePreviewController(client, state, () => true, () => 2);

      await controller.open(new FilePreviewTarget(
        'changed.png', 'changed.png', 'changed.png', 'session-1', '/workspace', 2
      ));

      expect(state.phase).assertEqual(FilePreviewPhase.Unsupported);
      expect(state.contentBase64).assertEqual('');
    });

    it('loads Markdown and image content into their dedicated renderer states', 0, async () => {
      const markdownClient = new FakeRemoteFileDownloadClient();
      markdownClient.fileInfo = { name: 'README.md', size: 7, mimeType: 'text/markdown' };
      markdownClient.chunkResult = {
        name: 'README.md',
        contentBase64: Encoding.bytesToBase64(Encoding.utf8ToBytes('# Title')),
        offset: 0,
        chunkSize: 7,
        totalSize: 7,
        mimeType: 'text/markdown'
      };
      const markdownState = new FilePreviewState();
      const markdownController = new RemoteFilePreviewController(markdownClient, markdownState, () => true, () => 2);
      await markdownController.open(new FilePreviewTarget(
        'README.md', 'README.md', 'README.md', 'session-1', '/workspace', 2
      ));

      expect(markdownState.phase).assertEqual(FilePreviewPhase.Ready);
      expect(markdownState.rendererKind).assertEqual(FilePreviewRendererKind.Markdown);
      expect(markdownState.textContent).assertEqual('# Title');

      const imageClient = new FakeRemoteFileDownloadClient();
      imageClient.fileInfo = { name: 'photo.png', size: 4, mimeType: 'image/png' };
      imageClient.fileReadResult = {
        name: 'photo.png',
        contentBase64: 'AAECAw==',
        mimeType: 'image/png',
        size: 4
      };
      const imageState = new FilePreviewState();
      const imageController = new RemoteFilePreviewController(imageClient, imageState, () => true, () => 2);
      await imageController.open(new FilePreviewTarget(
        'photo.png', 'photo.png', 'photo.png', 'session-1', '/workspace', 2
      ));

      expect(imageState.phase).assertEqual(FilePreviewPhase.Ready);
      expect(imageState.rendererKind).assertEqual(FilePreviewRendererKind.Image);
      expect(imageState.contentBase64).assertEqual('AAECAw==');
    });

    it('does not access the client for unavailable or invalid targets', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      let available = false;
      const state = new FilePreviewState();
      const controller = new RemoteFilePreviewController(client, state, () => available, () => 2);

      await controller.open(new FilePreviewTarget(
        'README.md', 'README.md', 'README.md', 'session-1', '/workspace', 2
      ));
      available = true;
      await controller.open(new FilePreviewTarget(
        'README.md', 'README.md', 'README.md', '', '/workspace', 2
      ));

      expect(client.infoRequests.length).assertEqual(0);
      expect(client.readRequests.length).assertEqual(0);
      expect(state.visible).assertFalse();
    });

    it('discards results after close or control-target changes', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      const state = new FilePreviewState();
      let epoch = 5;
      const controller = new RemoteFilePreviewController(client, state, () => true, () => epoch);
      const target = new FilePreviewTarget(
        'README.md', 'README.md', 'README.md', 'session-1', '/workspace', 5
      );

      const pending = controller.open(target);
      controller.close();
      epoch = 6;
      await pending;

      expect(state.visible).assertFalse();
      expect(state.phase).assertEqual(FilePreviewPhase.Idle);
      await controller.open(target);
      expect(state.visible).assertFalse();
    });

    it('turns an interrupted load into a retryable error after a transient disconnect', 0, async () => {
      const client = new BlockingFileInfoClient();
      client.fileInfo = { name: 'README.md', size: 6, mimeType: 'text/plain' };
      client.chunkResult = {
        name: 'README.md',
        contentBase64: Encoding.bytesToBase64(Encoding.utf8ToBytes('readme')),
        offset: 0,
        chunkSize: 6,
        totalSize: 6,
        mimeType: 'text/plain'
      };
      const state = new FilePreviewState();
      let available = true;
      const controller = new RemoteFilePreviewController(client, state, () => available, () => 4);
      const target = new FilePreviewTarget(
        'README.md', 'README.md', 'README.md', 'session-1', '/workspace', 4
      );

      const interrupted = controller.open(target);
      expect(state.phase).assertEqual(FilePreviewPhase.Loading);
      available = false;
      client.resolveFileInfo();
      await interrupted;

      expect(state.visible).assertTrue();
      expect(state.phase).assertEqual(FilePreviewPhase.Error);
      expect(state.errorText).assertEqual(RemoteI18n.t('filePreview.offline'));
      expect(state.target.remotePath).assertEqual('README.md');

      available = true;
      const retried = controller.refresh();
      client.resolveFileInfo();
      await retried;

      expect(state.phase).assertEqual(FilePreviewPhase.Ready);
      expect(state.textContent).assertEqual('readme');
    });

    it('keeps C when A and B metadata resolve after three rapid requests', 0, async () => {
      const client = new MultiBlockingFileInfoClient();
      client.chunkResult = {
        name: 'c.rs',
        contentBase64: Encoding.bytesToBase64(Encoding.utf8ToBytes('fn c() {}')),
        offset: 0,
        chunkSize: 9,
        totalSize: 9,
        mimeType: 'text/plain'
      };
      const state = new FilePreviewState();
      const controller = new RemoteFilePreviewController(client, state, () => true, () => 7);
      const targetA = new FilePreviewTarget('a.rs', 'a.rs', 'a.rs', 'session-1', '/workspace', 7);
      const targetB = new FilePreviewTarget('b.rs', 'b.rs', 'b.rs', 'session-1', '/workspace', 7);
      const targetC = new FilePreviewTarget('c.rs', 'c.rs', 'c.rs', 'session-1', '/workspace', 7);

      const pendingA = controller.open(targetA);
      const pendingB = controller.open(targetB);
      const pendingC = controller.open(targetC);
      client.resolveFileInfo('c.rs', { name: 'c.rs', size: 9, mimeType: 'text/plain' });
      await pendingC;
      client.resolveFileInfo('b.rs', { name: 'b.rs', size: 9, mimeType: 'text/plain' });
      await pendingB;
      client.resolveFileInfo('a.rs', { name: 'a.rs', size: 9, mimeType: 'text/plain' });
      await pendingA;

      expect(state.phase).assertEqual(FilePreviewPhase.Ready);
      expect(state.target.remotePath).assertEqual('c.rs');
      expect(state.textContent).assertEqual('fn c() {}');
      expect(client.readRequests.length).assertEqual(1);
      expect(client.readRequests[0].indexOf('c.rs|session-1') === 0).assertTrue();
    });

    it('classifies markdown, images and executable SVG conservatively', 0, () => {
      expect(RemoteFilePreviewController.rendererFor('README.md', 'text/markdown'))
        .assertEqual(FilePreviewRendererKind.Markdown);
      expect(RemoteFilePreviewController.rendererFor('Dockerfile', 'application/octet-stream'))
        .assertEqual(FilePreviewRendererKind.Text);
      expect(RemoteFilePreviewController.rendererFor('Makefile', 'application/octet-stream'))
        .assertEqual(FilePreviewRendererKind.Text);
      expect(RemoteFilePreviewController.rendererFor('LICENSE', 'application/octet-stream'))
        .assertEqual(FilePreviewRendererKind.Text);
      expect(RemoteFilePreviewController.rendererFor('photo.png', 'image/png'))
        .assertEqual(FilePreviewRendererKind.Image);
      expect(RemoteFilePreviewController.rendererFor('drawing.svg', 'image/svg+xml'))
        .assertEqual(FilePreviewRendererKind.Unsupported);
    });
  });

  describe('FilePreviewPlacementPolicy', () => {
    it('keeps compact devices full-page and regular tablets focused on chat plus preview', 0, () => {
      expect(FilePreviewPlacementPolicy.resolve(true, false, 1080, []))
        .assertEqual(FilePreviewPlacement.CompactFullPage);
      expect(FilePreviewPlacementPolicy.resolve(true, true, 900, []))
        .assertEqual(FilePreviewPlacement.WideFocusSplit);
      expect(FilePreviewPlacementPolicy.resolve(false, true, 1400, []))
        .assertEqual(FilePreviewPlacement.Hidden);
    });

    it('uses triple pane only when all flat or crease-aligned panes meet minimum widths', 0, () => {
      expect(FilePreviewPlacementPolicy.resolve(true, true, 1002, []))
        .assertEqual(FilePreviewPlacement.WideTriplePane);
      expect(FilePreviewPlacementPolicy.resolve(true, true, 1200, [
        new ConversationLayoutCrease(300, 8),
        new ConversationLayoutCrease(700, 8)
      ])).assertEqual(FilePreviewPlacement.WideTriplePane);
      expect(FilePreviewPlacementPolicy.resolve(true, true, 1050, [
        new ConversationLayoutCrease(280, 8),
        new ConversationLayoutCrease(620, 8)
      ])).assertEqual(FilePreviewPlacement.WideFocusSplit);
    });

    it('returns renderable pane geometry that matches flat and crease-aligned decisions', 0, () => {
      const flat = FilePreviewPlacementPolicy.resolveLayout(true, true, 1100, [], 344);
      expect(flat.placement).assertEqual(FilePreviewPlacement.WideTriplePane);
      expect(flat.masterPaneWidth).assertEqual(344);
      expect(flat.masterConversationGap).assertEqual(1);
      expect(flat.conversationPaneWidth).assertEqual(377);
      expect(flat.conversationPreviewGap).assertEqual(1);
      expect(flat.previewPaneWidth).assertEqual(377);

      const creased = FilePreviewPlacementPolicy.resolveLayout(true, true, 1260, [
        new ConversationLayoutCrease(320, 12),
        new ConversationLayoutCrease(760, 16)
      ], 344);
      expect(creased.placement).assertEqual(FilePreviewPlacement.WideTriplePane);
      expect(creased.masterPaneWidth).assertEqual(320);
      expect(creased.masterConversationGap).assertEqual(12);
      expect(creased.conversationPaneWidth).assertEqual(428);
      expect(creased.conversationPreviewGap).assertEqual(16);
      expect(creased.previewPaneWidth).assertEqual(484);
      expect(creased.masterConversationGapIsCrease).assertTrue();
      expect(creased.conversationPreviewGapIsCrease).assertTrue();
    });

    it('shrinks the flat master pane at the triple-pane threshold without narrowing content panes', 0, () => {
      const threshold = FilePreviewPlacementPolicy.resolveLayout(true, true, 1002, [], 344);
      expect(threshold.placement).assertEqual(FilePreviewPlacement.WideTriplePane);
      expect(threshold.masterPaneWidth).assertEqual(280);
      expect(threshold.conversationPaneWidth).assertEqual(360);
      expect(threshold.previewPaneWidth).assertEqual(360);
    });

    it('handles invalid, unsorted and rotation-sized geometry conservatively', 0, () => {
      expect(FilePreviewPlacementPolicy.resolve(true, true, 900, [
        new ConversationLayoutCrease(-20, 8),
        new ConversationLayoutCrease(880, 40)
      ])).assertEqual(FilePreviewPlacement.WideFocusSplit);
      expect(FilePreviewPlacementPolicy.resolve(true, true, 1200, [
        new ConversationLayoutCrease(700, 8),
        new ConversationLayoutCrease(300, 8)
      ])).assertEqual(FilePreviewPlacement.WideTriplePane);
      expect(FilePreviewPlacementPolicy.resolve(true, true, 760, []))
        .assertEqual(FilePreviewPlacement.WideFocusSplit);
      expect(FilePreviewPlacementPolicy.resolve(true, false, 760, []))
        .assertEqual(FilePreviewPlacement.CompactFullPage);
    });

    it('keeps the focus layout full-page until both panes meet their minimum widths', 0, () => {
      expect(FilePreviewPlacementPolicy.resolve(true, true, 720, []))
        .assertEqual(FilePreviewPlacement.CompactFullPage);
      const threshold = FilePreviewPlacementPolicy.resolveLayout(true, true, 721, []);
      expect(threshold.placement).assertEqual(FilePreviewPlacement.WideFocusSplit);
      expect(threshold.conversationPaneWidth).assertEqual(360);
      expect(threshold.previewPaneWidth).assertEqual(360);
    });

    it('aligns dual-fold preview focus to the single vertical crease', 0, () => {
      const layout = FilePreviewPlacementPolicy.resolveLayout(true, true, 2210, [
        new ConversationLayoutCrease(1100, 16)
      ]);
      expect(layout.placement).assertEqual(FilePreviewPlacement.WideFocusSplit);
      expect(layout.masterPaneWidth).assertEqual(0);
      expect(layout.conversationPaneWidth).assertEqual(1100);
      expect(layout.conversationPreviewGap).assertEqual(16);
      expect(layout.previewPaneWidth).assertEqual(1094);
    });

    it('keeps tri-fold preview on the right panel until the user expands it', 0, () => {
      const single = FilePreviewPlacementPolicy.resolveLayout(true, true, 1107, [
        new ConversationLayoutCrease(351, 44)
      ]);
      expect(single.placement).assertEqual(FilePreviewPlacement.WideFocusSplit);
      expect(single.conversationPaneWidth).assertEqual(351);
      expect(single.conversationPreviewGap).assertEqual(44);
      expect(single.previewPaneWidth).assertEqual(712);
      expect(single.conversationPreviewGapIsCrease).assertTrue();

      const dual = FilePreviewPlacementPolicy.resolveLayout(true, true, 1107, [
        new ConversationLayoutCrease(351, 44),
        new ConversationLayoutCrease(773, 44)
      ]);
      expect(dual.placement).assertEqual(FilePreviewPlacement.WideTriplePane);
      expect(dual.masterPaneWidth).assertEqual(351);
      expect(dual.masterConversationGap).assertEqual(44);
      expect(dual.conversationPaneWidth).assertEqual(378);
      expect(dual.conversationPreviewGap).assertEqual(44);
      expect(dual.previewPaneWidth).assertEqual(290);
      expect(dual.masterConversationGapIsCrease).assertTrue();
      expect(dual.conversationPreviewGapIsCrease).assertTrue();

      const expanded = FilePreviewPlacementPolicy.resolveLayout(true, true, 1107, [
        new ConversationLayoutCrease(351, 44),
        new ConversationLayoutCrease(773, 44)
      ], FilePreviewPlacementPolicy.MIN_MASTER_WIDTH, true);
      expect(expanded.placement).assertEqual(FilePreviewPlacement.WideMasterPreviewFocus);
      expect(expanded.masterPaneWidth).assertEqual(351);
      expect(expanded.masterConversationGap).assertEqual(44);
      expect(expanded.conversationPaneWidth).assertEqual(0);
      expect(expanded.previewPaneWidth).assertEqual(712);
    });

    it('falls back from crease-aligned focus when one side is too narrow', 0, () => {
      const layout = FilePreviewPlacementPolicy.resolveLayout(true, true, 900, [
        new ConversationLayoutCrease(200, 8)
      ]);
      expect(layout.placement).assertEqual(FilePreviewPlacement.WideFocusSplit);
      expect(layout.conversationPaneWidth).assertEqual(449);
      expect(layout.conversationPreviewGap).assertEqual(1);
      expect(layout.previewPaneWidth).assertEqual(450);
    });
  });

  describe('RemoteFileDownloadController', () => {
    it('owns remote file download progress and delayed downloading marker cleanup', 0, async () => {
      const client = new FakeRemoteFileDownloadClient();
      const scheduler = new FakeRemoteFileDownloadScheduler();
      const fileSaver = new FakeRemoteFileSaver();
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
        scheduler,
        fileSaver
      );

      await controller.download('/workspace/BitFun/README.md', 'session-1', false, true);

      expect(client.infoRequests[0]).assertEqual('/workspace/BitFun/README.md|session-1');
      expect(client.readRequests[0]).assertEqual('/workspace/BitFun/README.md|session-1');
      expect(fileSaver.savedNames[0]).assertEqual('README.md');
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
      await controller.approve('tool-1', 'session-1', true, updatedInput);
      await controller.reject('tool-2', 'session-1', true);
      await controller.cancel('tool-3', 'session-1', true);
      await controller.answer('tool-4', 'session-1', true, { answer: 'yes', '0': 'yes' });

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

    it('runs available tool actions and skips invalid route state', 0, async () => {
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

      await controller.approve('', 'session-1', true);
      await controller.approve('tool-1', '', true);
      await controller.approve('tool-1', 'session-1', true);
      await controller.approve('tool-1', 'session-1', false);

      expect(client.actions.length).assertEqual(1);
      expect(client.actions[0]).assertEqual('confirm:tool-1:{}');
      expect(busyEvents.length).assertEqual(2);
      expect(pollCount).assertEqual(1);
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

      await controller.reject('tool-1', 'session-1', true);

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

      const created = await controller.create('code', false, true, async (session: SessionSummary): Promise<void> => {
        openedSessions.push(session);
      });
      await controller.open(remoteSession('opened-session', 'Opened Session'), '/workspace', false, true,
        async (session: SessionSummary): Promise<void> => {
          openedSessions.push(session);
        });

      expect(client.createRequests[0]).assertEqual('code:::');
      expect(created?.sessionId || '').assertEqual('created-session');
      expect(activeSessions.length).assertEqual(2);
      expect(activeSessions[0].sessionId).assertEqual('created-session');
      expect(activeSessions[1].sessionId).assertEqual('opened-session');
      expect(activeSessions[1].workspacePath).assertEqual('/workspace');
      expect(openedSessions.length).assertEqual(2);
    });

    it('forwards an explicit workspace path without switching the host workspace', 0, async () => {
      const client = new FakeRemoteSessionClient();
      client.createdSession = {
        sessionId: 'created-in-other',
        title: 'Created Session',
        workspacePath: '/other',
        agentType: 'code'
      };
      const controller = new RemoteSessionController(client, 8, {
        onSessions: (_sessions: RemoteSession[], _hasMore: boolean) => {},
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

      const created = await controller.create(
        'code', false, true, async (_session: SessionSummary): Promise<void> => {},
        '', '', '/other'
      );

      expect(client.createRequests[0]).assertEqual('code:::/other');
      expect(created?.workspacePath || '').assertEqual('/other');
    });

    it('hands an allocated session to the conversation before its initial turn settles', 0, async () => {
      const client = new FakeRemoteSessionClient();
      client.emitAllocationBeforeCreateReturn = true;
      client.createdSession = {
        sessionId: 'allocated-session',
        title: 'Allocated Session',
        workspacePath: '/workspace',
        agentType: 'code',
        initialTurnId: 'turn-ready'
      };
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
      }, 'prompt', 'model-a', '/workspace');

      expect(activeSessions.length).assertEqual(2);
      expect(activeSessions[0].sessionId).assertEqual('allocated-session');
      expect(activeSessions[0].initialTurnId || '').assertEqual('');
      expect(activeSessions[1].initialTurnId || '').assertEqual('turn-ready');
      expect(openedSessions.length).assertEqual(1);
      expect(openedSessions[0].initialTurnId || '').assertEqual('');
    });

    it('returns no created session when the remote create command fails', 0, async () => {
      const client = new FakeRemoteSessionClient();
      client.shouldFailCreate = true;
      const controller = new RemoteSessionController(client, 8, {
        onSessions: (_sessions: RemoteSession[], _hasMore: boolean) => {},
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

      const created = await controller.create('code', false, true, async (_session: SessionSummary): Promise<void> => {});

      expect(created === undefined).assertTrue();
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

  describe('RemoteSessionViewModel', () => {
    it('reconciles a created session with the workspace named by the response', 0, async () => {
      const pageState = new RemotePageState();
      pageState.workspacePath = '/current';
      const sessionClient = new FakeRemoteSessionClient();
      sessionClient.createdSession = {
        sessionId: 'created-in-other',
        title: 'Created in Other',
        workspacePath: '/other',
        agentType: 'code'
      };
      let busy = false;
      const sessions = new RemoteSessionController(sessionClient, 8, {
        onSessions: (_items: RemoteSession[], _hasMore: boolean) => {},
        onActiveSession: (session: SessionSummary) => pageState.setActiveSession(session),
        onStatusText: (_statusText: string) => {},
        onBusy: (isBusy: boolean) => {
          busy = isBusy;
          pageState.setBusy(isBusy);
        },
        onLoading: (_isLoading: boolean) => {},
        onSessionError: (_errorText: string) => {},
        onReconnecting: () => {},
        onConnected: () => {},
        onConnectionFailed: (_err: Object) => {},
        onStartHeartbeat: () => {}
      });
      const chat = new RemoteChatCommandController(new FakeRemoteChatCommandClient(), {
        onMessagesLoaded: (_messages, _hasMoreMessages) => {},
        onMessageCountKnown: (_pollVersion, _knownMessageCount) => {},
        onTimelineReady: () => {},
        onSendSucceeded: (_turnId, _pendingActiveId) => {},
        onSendFailed: (_rawText, _images, _localMessageId, _pendingActiveId) => {},
        onActiveSession: (_session) => {},
        onSessionTitleChanged: (_sessionId, _title) => {},
        onStatusText: (_statusText) => {},
        onToast: (_message) => {},
        onBusy: (_isBusy) => {},
        onPollRequested: () => {}
      }, inertRemoteChatCache());
      const models = new RemoteModelController(
        new FakeRemoteModelClient(),
        new FakeRemoteModelPreferenceStore(),
        (_catalog, _selectedModelId, _version) => {},
        (_catalog, _selectedModelId, _version) => {},
        (_statusText) => {},
        (_isBusy) => {}
      );
      const files = new RemoteFileDownloadController(
        new FakeRemoteFileDownloadClient(),
        (_downloading, _downloaded, _status) => {},
        () => {},
        (_statusText) => {},
        (_isBusy) => {},
        2500,
        new FakeRemoteFileDownloadScheduler(),
        new FakeRemoteFileSaver()
      );
      const reconciled: SessionSummary[] = [];
      const routedSessions: string[] = [];
      const viewModel = new RemoteSessionViewModel(pageState, sessions, chat, models, files, {
        remoteAvailable: (): boolean => true,
        isConnected: (): boolean => true,
        isBusy: (): boolean => busy,
        onBusy: (isBusy: boolean): void => {
          busy = isBusy;
        },
        onRouteChat: (_sessionId: string): void => {},
        onRouteHome: (): void => {},
        onStopPolling: (): void => {},
        onStartPolling: (): void => {},
        onNudgePolling: (): void => {},
        onResetTimeline: (_sessionId: string): void => {},
        onClearRemoteFiles: (): void => {},
        onKnownStateReset: (): void => {},
        onLoadActiveMessages: async (): Promise<void> => {},
        onCreatedSession: async (session: SessionSummary): Promise<void> => {
          reconciled.push(session);
        },
        onRefreshSessions: async (): Promise<void> => {},
        onSelectWorkspace: async (_path: string): Promise<void> => {}
      });

      await viewModel.createSessionInWorkspace(
        '/other',
        'introduce this project',
        'code',
        (sessionId: string): void => {
          routedSessions.push(sessionId);
        },
        '',
        false
      );

      expect(reconciled.length).assertEqual(1);
      expect(reconciled[0].sessionId).assertEqual('created-in-other');
      expect(reconciled[0].workspacePath).assertEqual('/other');
      expect(sessionClient.listRequests.length).assertEqual(0);
      expect(routedSessions.length).assertEqual(1);
      expect(routedSessions[0]).assertEqual('created-in-other');
    });
  });

  describe('RemoteSessionListCache', () => {
    it('restores the list of the device it last stored one for', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      const cache = await readyRemoteSessionListCache(store, 'desktop-a');

      await cache.save([remoteSession('session-1', 'Session 1')], true);
      const restored = await cache.restoreLast();

      expect(restored.sessions.length).assertEqual(1);
      expect(restored.sessions[0].id).assertEqual('session-1');
      expect(restored.hasMore).assertTrue();
    });

    it('keeps each desktop list to itself', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      const first = await readyRemoteSessionListCache(store, 'desktop-a');
      const second = await readyRemoteSessionListCache(store, 'desktop-b');

      await first.save([remoteSession('session-1', 'Session 1')], false);
      await second.save([remoteSession('session-2', 'Session 2')], false);

      const fromFirst = await first.load();
      const fromSecond = await second.load();
      expect(fromFirst.sessions[0].id).assertEqual('session-1');
      expect(fromSecond.sessions[0].id).assertEqual('session-2');
    });

    it('does not restore or persist ACP sessions', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      const cache = await readyRemoteSessionListCache(store, 'desktop-a');
      const acp = remoteSession('acp-session', 'ACP');
      acp.agentType = 'acp:codex';
      const native = remoteSession('native-session', 'Native');
      await store.saveList('desktop-a', [acp, native], false);

      const restored = await cache.restoreLast();
      expect(restored.sessions.length).assertEqual(1);
      expect(restored.sessions[0].id).assertEqual('native-session');

      await cache.save([acp, native], false);
      const saved = await store.loadList('desktop-a');
      expect(saved.sessions.length).assertEqual(1);
      expect(saved.sessions[0].id).assertEqual('native-session');
    });

    it('writes once for a list that has not changed', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      const cache = await readyRemoteSessionListCache(store);

      await cache.save([remoteSession('session-1', 'Session 1')], false);
      await cache.save([remoteSession('session-1', 'Session 1')], false);
      expect(store.saveCount).assertEqual(1);

      await cache.save([remoteSession('session-1', 'Renamed')], false);
      expect(store.saveCount).assertEqual(2);
    });

    it('leaves a stored list alone when the sessions go away', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      const cache = await readyRemoteSessionListCache(store);

      await cache.save([remoteSession('session-1', 'Session 1')], false);
      await cache.save([], false);

      const restored = await cache.restoreLast();
      expect(restored.sessions.length).assertEqual(1);
    });

    it('reads nothing back once signing out clears it', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      const cache = await readyRemoteSessionListCache(store);

      await cache.save([remoteSession('session-1', 'Session 1')], false);
      await cache.clear();

      const restored = await cache.restoreLast();
      expect(restored.sessions.length).assertEqual(0);
      expect(store.clearCount).assertEqual(1);
    });

    it('reports a miss instead of failing when the store is unusable', 0, async () => {
      const store = new FakeRemoteSessionListStore();
      store.shouldFail = true;
      const cache = new RemoteSessionListCache(store, (): string => 'device-1');
      await cache.init({} as Context);

      await cache.save([remoteSession('session-1', 'Session 1')], false);
      const restored = await cache.restoreLast();

      expect(restored.sessions.length).assertEqual(0);
      expect(store.saveCount).assertEqual(0);
    });
  });

  describe('RemoteChatCommandController', () => {
    it('coalesces rapid transcript misses to the latest selected session', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const projection = new RemoteChatMessagesProjectionRecord();
      let activeSessionId = 'session-1';
      client.deferMessages = true;
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => {
          projection.messages = messages;
          projection.hasMoreMessages = hasMoreMessages;
        },
        onMessageCountKnown: (pollVersion: number, knownMessageCount: number) => {
          projection.pollVersion = pollVersion;
          projection.knownMessageCount = knownMessageCount;
        },
        onTimelineReady: () => {},
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
        onToast: (_message: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, inertRemoteChatCache());
      const canApply = (sessionId: string): boolean => sessionId === activeSessionId;

      const first = controller.loadMessages('session-1', canApply);
      await Promise.resolve();
      activeSessionId = 'session-2';
      const middle = controller.loadMessages('session-2', canApply);
      await Promise.resolve();
      activeSessionId = 'session-3';
      const latest = controller.loadMessages('session-3', canApply);
      await Promise.resolve();

      expect(client.messageRequests.length).assertEqual(1);
      expect(client.messageRequests[0]).assertEqual('session-1');
      client.resolveNextMessages({
        messages: [chatMessage('message-1', 'assistant', 'Stale')],
        hasMore: false
      });
      // The stale result is intentionally written to the cache before the
      // latest waiter gets the transport slot.
      await delay(0);

      expect(client.messageRequests.length).assertEqual(2);
      expect(client.messageRequests[1]).assertEqual('session-3');
      client.resolveNextMessages({
        messages: [chatMessage('message-3', 'assistant', 'Latest')],
        hasMore: false
      });
      await Promise.all([first, middle, latest]);

      expect(client.messageRequests.length).assertEqual(2);
      expect(projection.messages.length).assertEqual(1);
      expect(projection.messages[0].id).assertEqual('message-3');
    });

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
        onTimelineReady: () => {},
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
        onToast: (_message: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, inertRemoteChatCache());

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

    it('caches a fetched transcript and reopens it without a request', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const store = new FakeRemoteChatCacheStore();
      const projection = new RemoteChatMessagesProjectionRecord();
      const statuses: string[] = [];
      let timelineReadyCount = 0;
      client.messageResults = [{
        messages: [
          chatMessage('message-1', 'user', 'Hello'),
          chatMessage('message-2', 'assistant', 'Hi')
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
        onTimelineReady: () => {
          timelineReadyCount += 1;
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
        onToast: (_message: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, await readyRemoteChatCache(store));

      await controller.loadMessages('session-1', (_sessionId: string) => true);
      await controller.loadMessages('session-1', (_sessionId: string) => true);

      expect(client.messageRequests.length).assertEqual(1);
      expect(projection.messages.length).assertEqual(2);
      expect(projection.messages[1].id).assertEqual('message-2');
      // A cached open still resets the poll version: the desktop restarts its
      // version tracker, so a carried-over one would match by accident.
      expect(projection.pollVersion).assertEqual(0);
      expect(projection.knownMessageCount).assertEqual(2);
      expect(timelineReadyCount).assertEqual(2);
      expect(statuses[statuses.length - 1]).assertEqual(RemoteI18n.t('status.messagesRestored'));
    });

    it('refetches when a cached assistant has no body', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const store = new FakeRemoteChatCacheStore();
      const projection = new RemoteChatMessagesProjectionRecord();
      const cache = await readyRemoteChatCache(store);
      const hollow = chatMessage('message-2', 'assistant', '');
      hollow.thinking = '';
      hollow.tools = [];
      hollow.items = [];
      await cache.syncIncremental('session-1', [
        chatMessage('message-1', 'user', 'Hello'),
        hollow
      ]);
      client.messageResults = [{
        messages: [
          chatMessage('message-1', 'user', 'Hello'),
          chatMessage('message-2', 'assistant', 'Recovered reply')
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
        onTimelineReady: () => {},
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
        onToast: (_message: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, cache);

      await controller.loadMessages('session-1', (_sessionId: string) => true);

      expect(client.messageRequests.length).assertEqual(1);
      expect(projection.messages.length).assertEqual(2);
      expect(projection.messages[1].text).assertEqual('Recovered reply');

      await controller.loadMessages('session-1', (_sessionId: string) => true);
      expect(client.messageRequests.length).assertEqual(1);
    });

    it('caches a transcript the user switched away from before it arrived', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const store = new FakeRemoteChatCacheStore();
      const projection = new RemoteChatMessagesProjectionRecord();
      let timelineReadyCount = 0;
      client.messageResults = [{
        messages: [
          chatMessage('message-1', 'user', 'Hello'),
          chatMessage('message-2', 'assistant', 'Hi')
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
        onTimelineReady: () => {
          timelineReadyCount += 1;
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
        onToast: (_message: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, await readyRemoteChatCache(store));

      // Opened, then switched away from before the fetch landed. Nothing may
      // reach the timeline that now belongs to another session.
      await controller.loadMessages('session-1', (_sessionId: string) => false);
      expect(projection.messages.length).assertEqual(0);
      expect(timelineReadyCount).assertEqual(0);

      // Reopening it has to be free: the transcript was already paid for.
      await controller.loadMessages('session-1', (_sessionId: string) => true);

      expect(client.messageRequests.length).assertEqual(1);
      expect(projection.messages.length).assertEqual(2);
      expect(projection.messages[1].id).assertEqual('message-2');
      expect(projection.knownMessageCount).assertEqual(2);
      expect(timelineReadyCount).assertEqual(1);
    });

    it('appends polled messages to the cache instead of rewriting it', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const store = new FakeRemoteChatCacheStore();
      const cache = await readyRemoteChatCache(store);
      client.messageResults = [{
        messages: [chatMessage('message-1', 'user', 'Hello')],
        hasMore: false
      }];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onTimelineReady: () => {},
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
        onToast: (_message: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, cache);

      await controller.loadMessages('session-1', (_sessionId: string) => true);
      await cache.syncIncremental('session-1', [
        chatMessage('message-1', 'user', 'Hello'),
        chatMessage('message-2', 'assistant', 'Hi')
      ]);
      // Nothing new arrived, so the second sync must not touch the store.
      await cache.syncIncremental('session-1', [
        chatMessage('message-1', 'user', 'Hello'),
        chatMessage('message-2', 'assistant', 'Hi')
      ]);

      expect(store.replaceCount).assertEqual(1);
      expect(store.appendCount).assertEqual(1);
      expect((await cache.load('session-1')).length).assertEqual(2);
    });

    it('persists an authoritative body update when message ids and count stay unchanged', 0, async () => {
      const store = new FakeRemoteChatCacheStore();
      const cache = await readyRemoteChatCache(store);
      const partialAssistant = chatMessage('message-2', 'assistant', '');
      partialAssistant.status = 'done';
      partialAssistant.thinking = 'Finished the work.';

      await cache.syncIncremental('session-1', [
        chatMessage('message-1', 'user', 'Open the PR'),
        partialAssistant
      ]);

      const completedAssistant = chatMessage('message-2', 'assistant', 'PR created successfully.');
      completedAssistant.status = 'done';
      completedAssistant.thinking = 'Finished the work.';
      await cache.replaceSnapshot('session-1', [
        chatMessage('message-1', 'user', 'Open the PR'),
        completedAssistant
      ]);

      // A fresh cache instance models an app restart: the completed body must
      // come from RDB rather than from the in-memory resident transcript.
      const restarted = await readyRemoteChatCache(store);
      const restored = await restarted.load('session-1');
      expect(store.replaceCount).assertEqual(2);
      expect(restored.length).assertEqual(2);
      expect(restored[1].id).assertEqual('message-2');
      expect(restored[1].text).assertEqual('PR created successfully.');
    });

    it('rewrites the cache when the stored prefix stops matching', 0, async () => {
      const store = new FakeRemoteChatCacheStore();
      const cache = await readyRemoteChatCache(store);

      await cache.syncIncremental('session-1', [chatMessage('message-1', 'user', 'Hello')]);
      // Same length, different first message: an append here would splice two
      // different transcripts together.
      await cache.syncIncremental('session-1', [chatMessage('message-9', 'user', 'Rewritten')]);

      const restored = await cache.load('session-1');
      expect(store.appendCount).assertEqual(0);
      expect(store.replaceCount).assertEqual(2);
      expect(restored.length).assertEqual(1);
      expect(restored[0].id).assertEqual('message-9');
    });

    it('evicts only when a session enters the cache, not on every poll tick', 0, async () => {
      const store = new FakeRemoteChatCacheStore();
      const cache = await readyRemoteChatCache(store);

      await cache.syncIncremental('session-1', [chatMessage('message-1', 'user', 'Hello')]);
      await cache.syncIncremental('session-1', [
        chatMessage('message-1', 'user', 'Hello'),
        chatMessage('message-2', 'assistant', 'Hi')
      ]);

      expect(store.appendCount).assertEqual(1);
      expect(store.pruneRequests.length).assertEqual(1);
      expect(store.pruneRequests[0]).assertEqual(20);
    });

    it('drops the oldest transcript once the per-desktop budget is full', 0, async () => {
      const store = new FakeRemoteChatCacheStore();
      const cache = await readyRemoteChatCache(store);

      // One past the budget the prune above reported.
      for (let index = 0; index <= 20; index++) {
        await cache.syncIncremental(`session-${index}`, [chatMessage(`message-${index}`, 'user', 'Hello')]);
      }

      expect((await cache.load('session-0')).length).assertEqual(0);
      expect((await cache.load('session-1')).length).assertEqual(1);
      expect((await cache.load('session-20')).length).assertEqual(1);
    });

    it('keeps a reopened transcript ahead of ones only ever written once', 0, async () => {
      const store = new FakeRemoteChatCacheStore();
      const cache = await readyRemoteChatCache(store);

      for (let index = 0; index < 20; index++) {
        await cache.syncIncremental(`session-${index}`, [chatMessage(`message-${index}`, 'user', 'Hello')]);
      }
      // Reopening is what recency is supposed to be about: this read has to
      // outrank the nineteen sessions written after it.
      expect((await cache.load('session-0')).length).assertEqual(1);
      await cache.syncIncremental('session-20', [chatMessage('message-20', 'user', 'Hello')]);

      expect((await cache.load('session-0')).length).assertEqual(1);
      expect((await cache.load('session-1')).length).assertEqual(0);
    });

    it('drops a session from the cache when it is forgotten', 0, async () => {
      const store = new FakeRemoteChatCacheStore();
      const cache = await readyRemoteChatCache(store);

      await cache.syncIncremental('session-1', [chatMessage('message-1', 'user', 'Hello')]);
      await cache.forget('session-1');

      expect((await cache.load('session-1')).length).assertEqual(0);
    });

    it('keeps working when the cache store is unusable', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const store = new FakeRemoteChatCacheStore();
      const projection = new RemoteChatMessagesProjectionRecord();
      store.shouldFail = true;
      client.messageResults = [{
        messages: [chatMessage('message-1', 'assistant', 'Hello')],
        hasMore: false
      }];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (messages: ChatMessage[], hasMoreMessages: boolean) => {
          projection.messages = messages;
          projection.hasMoreMessages = hasMoreMessages;
        },
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onTimelineReady: () => {},
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
        onToast: (_message: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, await readyRemoteChatCache(store));

      await controller.loadMessages('session-1', (_sessionId: string) => true);

      expect(client.messageRequests.length).assertEqual(1);
      expect(projection.messages.length).assertEqual(1);
    });

    it('loads older messages with busy and cursor callbacks', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const store = new FakeRemoteChatCacheStore();
      const cache = await readyRemoteChatCache(store);
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
        onTimelineReady: () => {},
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
        onToast: (_message: string) => {},
        onBusy: (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        onPollRequested: () => {}
      }, cache);

      await controller.loadOlderMessages('session-1', 7, true, false);

      expect(client.messageRequests[0]).assertEqual('session-1');
      expect(projection.messages.length).assertEqual(2);
      expect(projection.hasMoreMessages).assertFalse();
      expect(projection.pollVersion).assertEqual(7);
      expect(projection.knownMessageCount).assertEqual(2);
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
      // What came back is a full transcript, so it belongs in the cache for the
      // same reason the first load's does.
      expect((await cache.load('session-1')).length).assertEqual(2);
    });

    it('sends prepared remote messages and reports failed payloads', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const succeeded: string[] = [];
      const failed: string[] = [];
      const busyEvents: boolean[] = [];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onTimelineReady: () => {},
        onSendSucceeded: (turnId: string, pendingActiveId: string, localMessageId: string) => {
          succeeded.push(`${turnId}:${pendingActiveId}:${localMessageId}`);
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
        onToast: (_message: string) => {},
        onBusy: (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        onPollRequested: () => {}
      }, inertRemoteChatCache());

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
      expect(client.sendTurnIds[0]).assertEqual('local-1');
      expect(client.sendTurnIds[1]).assertEqual('local-2');
      expect(succeeded[0]).assertEqual('turn-1:pending-1:local-1');
      expect(failed[0]).assertEqual('Retry text:0:local-2:pending-2');
      expect(busyEvents[0]).assertTrue();
      expect(busyEvents[1]).assertFalse();
      expect(busyEvents[2]).assertTrue();
      expect(busyEvents[3]).assertFalse();
    });

    it('steers the active turn and projects the accepted steering identity', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const accepted: string[] = [];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onTimelineReady: () => {},
        onSendSucceeded: (_turnId: string, _pendingActiveId: string) => {},
        onSteerSucceeded: (
          steeringId: string,
          turnId: string,
          displayText: string,
          images: SelectedImageAttachment[]
        ) => accepted.push(`${steeringId}:${turnId}:${displayText}:${images.length}`),
        onSendFailed: (
          _rawText: string,
          _images: SelectedImageAttachment[],
          _localMessageId: string,
          _pendingActiveId: string
        ) => {},
        onActiveSession: (_session: SessionSummary) => {},
        onSessionTitleChanged: (_sessionId: string, _title: string) => {},
        onStatusText: (_statusText: string) => {},
        onToast: (_message: string) => {},
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, inertRemoteChatCache());

      await controller.steerPreparedMessage(
        'session-1',
        'turn-1',
        'Continue with this',
        'Continue with this',
        [selectedImage('image-1')],
        [{ id: 'image-1', data_url: 'data:image/png;base64,AA==', mime_type: 'image/png' }],
        false,
        true
      );

      expect(client.steerRequests[0])
        .assertEqual('session-1:turn-1:Continue with this:Continue with this:1');
      expect(accepted[0]).assertEqual('steering-1:turn-1:Continue with this:1');
      expect(client.sendRequests.length).assertEqual(0);
    });

    it('stops active tasks and reports guard states', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const statuses: string[] = [];
      const toasts: string[] = [];
      let pollCount = 0;
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onTimelineReady: () => {},
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
        onToast: (message: string) => {
          toasts.push(message);
        },
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {
          pollCount += 1;
        }
      }, inertRemoteChatCache());

      await controller.stopTask('session-1', '', '', true);
      await controller.stopTask('session-1', 'active-turn', 'turn-1', true);

      expect(client.cancelRequests[0]).assertEqual('session-1:turn-1');
      expect(statuses[0]).assertEqual(RemoteI18n.t('status.noRunningTask'));
      expect(statuses[1]).assertEqual(RemoteI18n.t('status.stoppingTask'));
      expect(statuses[2]).assertEqual(RemoteI18n.t('status.stopRequested'));
      expect(pollCount).assertEqual(1);
      // The status line is hidden while connected, so a refused stop is only
      // ever visible as a toast.
      expect(toasts[0]).assertEqual(RemoteI18n.t('status.noRunningTask'));
      expect(toasts.length).assertEqual(1);
    });

    it('surfaces a refused stop instead of leaving the task silently running', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      client.shouldFailCancel = true;
      const statuses: string[] = [];
      const toasts: string[] = [];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onTimelineReady: () => {},
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
        onToast: (message: string) => {
          toasts.push(message);
        },
        onBusy: (_isBusy: boolean) => {},
        onPollRequested: () => {}
      }, inertRemoteChatCache());

      await controller.stopTask('session-1', 'active-turn', 'turn-1', true);

      expect(toasts.length).assertEqual(1);
      expect(toasts[0]).assertEqual(statuses[statuses.length - 1]);
      expect(toasts[0] === RemoteI18n.t('status.stopRequested')).assertFalse();
    });

    it('renames active sessions and updates active/list projections', 0, async () => {
      const client = new FakeRemoteChatCommandClient();
      const activeSessions: SessionSummary[] = [];
      const titleUpdates: string[] = [];
      const busyEvents: boolean[] = [];
      const controller = new RemoteChatCommandController(client, {
        onMessagesLoaded: (_messages: ChatMessage[], _hasMoreMessages: boolean) => {},
        onMessageCountKnown: (_pollVersion: number, _knownMessageCount: number) => {},
        onTimelineReady: () => {},
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
        onToast: (_message: string) => {},
        onBusy: (isBusy: boolean) => {
          busyEvents.push(isBusy);
        },
        onPollRequested: () => {}
      }, inertRemoteChatCache());

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
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.supportsAttachments).assertTrue();
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.requiresRemoteConnection).assertFalse();
      // Nothing queues a message on this surface, so a draft written mid-reply
      // has nowhere to go until the reply finishes.
      expect(GENERAL_CHAT_COMPOSER_CAPABILITIES.supportsMidRunSend).assertFalse();
    });

    it('keeps remote composer attachments behind the remote connection', 0, () => {
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.surface).assertEqual(ChatSurface.Remote);
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.supportsAttachments).assertTrue();
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.requiresRemoteConnection).assertTrue();
      // The desktop queues a mid-run message and lets the current turn yield.
      expect(REMOTE_CHAT_COMPOSER_CAPABILITIES.supportsMidRunSend).assertTrue();
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

    it('keeps user rows when a legacy remote transcript ends in a hollow assistant', 0, () => {
      const timelineItems = ChatTimelineProjector.project([
        chatMessage('remote-user-1', 'user', 'Hello'),
        chatMessage('remote-assistant-1', 'assistant', '')
      ], [], RemoteUiState.emptyActiveTurn(), false);

      const visibleItems = ConversationViewContract.visibleTimelineItems(timelineItems);

      expect(visibleItems.length).assertEqual(1);
      expect(visibleItems[0].type).assertEqual('user_message');
      expect(visibleItems[0].message!.text).assertEqual('Hello');
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

    it('splits remote routes from the general composer without overlap', 0, () => {
      // The two predicates are complements by construction. Asserting both
      // directions on every route is what keeps a later edit from creating a
      // route that is somehow neither, or both.
      expect(AppRouteContract.isRemoteRoute(AppRoute.ChatHome)).assertFalse();
      expect(AppRouteContract.isRemoteRoute(AppRoute.GeneralChat)).assertFalse();
      expect(AppRouteContract.isRemoteRoute(AppRoute.RemoteHome)).assertTrue();
      expect(AppRouteContract.isRemoteRoute(AppRoute.RemoteChat)).assertTrue();
      expect(AppRouteContract.isGeneralComposerRoute(AppRoute.ChatHome)).assertTrue();
    });

    it('resumes an in-flight remote session instead of landing on the picker', 0, () => {
      const offline = AppRouteContract.remoteSurfaceDestination(false, 'remote-1');
      const connectedWithoutSession = AppRouteContract.remoteSurfaceDestination(true, '');
      const resumable = AppRouteContract.remoteSurfaceDestination(true, 'remote-1');
      expect(offline.name).assertEqual(AppRoute.RemoteHome);
      expect(offline.hasSessionParam()).assertFalse();
      expect(connectedWithoutSession.name).assertEqual(AppRoute.RemoteHome);
      expect(connectedWithoutSession.hasSessionParam()).assertFalse();
      expect(resumable.name).assertEqual(AppRoute.RemoteChat);
      expect(resumable.routeParam().sessionId).assertEqual('remote-1');
    });

    it('routes an in-place remote session selection to an explicit chat destination', 0, () => {
      const target = AppRouteContract.remoteSessionDestination('remote-session-1');

      expect(target.name).assertEqual(AppRoute.RemoteChat);
      expect(target.hasSessionParam()).assertTrue();
      expect(target.routeParam().sessionId).assertEqual('remote-session-1');
    });
  });

  describe('SessionActionPolicy', () => {
    it('exposes only capabilities backed by each session scope', 0, () => {
      const generalChat = SessionActionPolicy.resolve(SessionActionScope.General, 'chat', false);
      const generalCode = SessionActionPolicy.resolve(SessionActionScope.General, 'code', false);
      const remote = SessionActionPolicy.resolve(SessionActionScope.Remote, 'code', false);
      const busyRemote = SessionActionPolicy.resolve(SessionActionScope.Remote, 'code', true);

      expect(generalChat.canArchive).assertTrue();
      expect(generalChat.canViewDetails).assertTrue();
      expect(generalChat.canExport).assertTrue();
      expect(generalChat.canDelete).assertTrue();
      expect(generalCode.canArchive).assertFalse();
      expect(generalCode.canExport).assertFalse();
      expect(generalCode.canDelete).assertTrue();
      expect(remote.canArchive).assertFalse();
      expect(remote.canViewDetails).assertTrue();
      expect(remote.canExport).assertFalse();
      expect(remote.canDelete).assertTrue();
      expect(busyRemote.canDelete).assertFalse();
      expect(busyRemote.canViewDetails).assertFalse();
    });
  });

  describe('ConversationSessionFilterPolicy', () => {
    const codeSession: RemoteSession = {
      id: 'code-1',
      title: 'Fix layout',
      agentType: 'code',
      status: 'idle',
      updatedAt: '',
      createdAt: '',
      messageCount: 3,
      workspacePath: '/workspace/bitfun'
    };
    const chatSession: RemoteSession = {
      id: 'chat-1',
      title: 'Product notes',
      agentType: 'claw',
      status: 'active',
      updatedAt: '',
      createdAt: '',
      messageCount: 2
    };

    it('combines query, workspace, agent and status filters', 0, () => {
      expect(ConversationSessionFilterPolicy.matches(
        codeSession, 'layout', '', '/workspace/bitfun', 'code', 'idle', false
      )).assertTrue();
      expect(ConversationSessionFilterPolicy.matches(
        codeSession, 'notes', '', '/workspace/bitfun', 'code', 'idle', false
      )).assertFalse();
      expect(ConversationSessionFilterPolicy.matches(
        codeSession, '', '', '/workspace/other', 'code', 'idle', false
      )).assertFalse();
      expect(ConversationSessionFilterPolicy.matches(
        codeSession, '', '', '/workspace/bitfun/', 'code', 'idle', false
      )).assertTrue();
      expect(ConversationSessionFilterPolicy.workspacePathsEqual(
        '/workspace/bitfun/', '/workspace/bitfun'
      )).assertTrue();
      expect(ConversationSessionFilterPolicy.matches(
        chatSession, '', '/workspace/assistant', '', 'chat', 'active', true
      )).assertTrue();
      expect(ConversationSessionFilterPolicy.matches(
        chatSession, '', '/workspace/assistant', '/workspace/assistant', 'chat', 'active', true
      )).assertFalse();
    });

    it('excludes invalid and archived sessions before presentation grouping', 0, () => {
      const archived: RemoteSession = {
        id: 'archived-1',
        title: 'Archived',
        agentType: 'cowork',
        status: 'archived',
        updatedAt: '',
        createdAt: '',
        messageCount: 1
      };
      expect(ConversationSessionFilterPolicy.matches(
        archived, '', '', '', '', '', false
      )).assertFalse();
      expect(ConversationSessionFilterPolicy.agentGroup(chatSession, true)).assertEqual('chat');
      expect(ConversationSessionFilterPolicy.agentGroup(archived, false)).assertEqual('cowork');
    });
  });

  describe('SessionListProjection', () => {
    const session = (
      id: string,
      title: string,
      agentType: string,
      workspacePath: string,
      status: string = 'idle'
    ): RemoteSession => {
      return {
        id,
        title,
        agentType,
        status,
        updatedAt: '',
        createdAt: '',
        messageCount: 1,
        workspacePath
      };
    };

    const inputs = (sessions: RemoteSession[], query: string = ''): SessionListInputs => {
      return {
        sessions,
        query,
        sortMode: 'project',
        workspaceName: 'BitFun',
        workspacePath: '/workspace/bitfun',
        workspaceKind: 'normal',
        recentWorkspaces: [
          { path: '/workspace/bitfun', name: 'BitFun', lastOpened: '', workspaceKind: 'normal' },
          { path: '/workspace/notes', name: 'Notes', lastOpened: '', workspaceKind: 'assistant' }
        ],
        workspaceFilter: '',
        agentFilter: '',
        statusFilter: '',
        showWorkspaceMetadata: false,
        showUpdatedMetadata: false,
        showStatusMetadata: false
      };
    };

    const all: RemoteSession[] = [
      session('code-1', 'Fix layout', 'code', '/workspace/bitfun'),
      session('code-2', 'Trailing slash', 'code', '/workspace/bitfun/'),
      session('chat-1', 'Product notes', 'claw', ''),
      session('chat-2', 'Assistant workspace', 'code', '/workspace/notes'),
      session('gone-1', 'Archived', 'code', '/workspace/bitfun', 'archived')
    ];

    it('does not invent a workspace group from a name without a path', 0, () => {
      const named = inputs([]);
      named.workspaceName = 'BitFun';
      named.workspacePath = '';
      named.recentWorkspaces = [];
      const projection = SessionListProjector.project(named);
      expect(projection.projects.length).assertEqual(0);
    });

    it('does not turn a disconnected placeholder into a workspace group', 0, () => {
      const disconnected = inputs([
        session('code-1', 'Cached', 'code', '/workspace/bitfun')
      ]);
      disconnected.workspaceName = '未连接';
      disconnected.workspacePath = '';
      disconnected.recentWorkspaces = [
        { path: '/workspace/bitfun', name: 'BitFun', lastOpened: '', workspaceKind: 'normal' }
      ];
      const projection = SessionListProjector.project(disconnected);
      expect(projection.projects.map((item: RecentWorkspaceEntry) => item.name).join(','))
        .assertEqual('BitFun');
    });

    it('splits chats from project sessions in one pass', 0, () => {
      const projection = SessionListProjector.project(inputs(all));
      expect(projection.filtered.length).assertEqual(4);
      expect(projection.chats.map((item: RemoteSession) => item.id).join(',')).assertEqual('chat-1,chat-2');
      // A trailing slash is the same workspace, so both code sessions land in
      // the one bucket the sidebar renders under that project.
      const bucket = SessionListProjector.sessionsForProject(projection, '/workspace/bitfun/');
      expect(bucket.map((item: RemoteSession) => item.id).join(',')).assertEqual('code-1,code-2');
      expect(projection.projects.map((item: RecentWorkspaceEntry) => item.path).join(','))
        .assertEqual('/workspace/bitfun');
      expect(projection.hasActiveFilter).assertFalse();
      expect(SessionListProjector.sessionsForProject(projection, '/nowhere').length).assertEqual(0);
    });

    it('drops projects a filter has emptied', 0, () => {
      const filtered = SessionListProjector.project(inputs(all, 'product'));
      expect(filtered.hasActiveFilter).assertTrue();
      expect(filtered.chats.length).assertEqual(1);
      expect(filtered.projects.length).assertEqual(0);
    });

    it('reuses the projection until an input changes', 0, () => {
      const cache = new SessionListProjectionCache();
      const first = cache.get(inputs(all));
      expect(cache.get(inputs(all)) === first).assertTrue();

      // A caller that rebuilds the array every call — which is what a filtered
      // getter bound to a @Param does — still gets the cached projection.
      const copied = all.map((item: RemoteSession) => item);
      expect(cache.get(inputs(copied)) === first).assertTrue();

      expect(cache.get(inputs(all, 'layout')) === first).assertFalse();

      const renamed = all.map((item: RemoteSession): RemoteSession => {
        return item.id === 'code-1' ? session('code-1', 'Renamed', 'code', '/workspace/bitfun') : item;
      });
      const afterRename = cache.get(inputs(renamed));
      expect(afterRename === first).assertFalse();
      expect(afterRename.filtered[0].title).assertEqual('Renamed');
    });

    it('buckets by day only when sorting by time', 0, () => {
      const now = new Date();
      const recent = session('recent-1', 'Today', 'code', '/workspace/bitfun');
      recent.updatedAt = now.toISOString();
      const old = session('old-1', 'Older', 'code', '/workspace/bitfun');
      old.updatedAt = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000).toISOString();

      const byProject = SessionListProjector.project(inputs([recent, old]));
      expect(byProject.byTime.length).assertEqual(0);

      const timeInputs = inputs([old, recent]);
      timeInputs.sortMode = 'time';
      const byTime = SessionListProjector.project(timeInputs);
      expect(byTime.byTime.map((item: RemoteSession) => item.id).join(',')).assertEqual('recent-1,old-1');
      expect(byTime.today.map((item: RemoteSession) => item.id).join(',')).assertEqual('recent-1');
      expect(byTime.earlier.map((item: RemoteSession) => item.id).join(',')).assertEqual('old-1');
    });

    it('labels rows only for the metadata that is switched on', 0, () => {
      const plain = SessionListProjector.project(inputs(all));
      expect(SessionListProjector.metadataFor(plain, 'code-1')).assertEqual('');

      const labelled = inputs(all);
      labelled.showStatusMetadata = true;
      const projection = SessionListProjector.project(labelled);
      expect(SessionListProjector.metadataFor(projection, 'code-1')).assertEqual('idle');
    });
  });

  describe('ConversationModelPresentationPolicy', () => {
    const primaryModel: ConversationUiModel = {
      id: 'anthropic/claude-sonnet-4',
      name: 'Anthropic',
      provider: 'anthropic',
      base_url: '',
      model_name: 'anthropic/claude-sonnet-4',
      enabled: true,
      capabilities: []
    };
    const fastModel: ConversationUiModel = {
      id: 'openbitfun:gpt-5-mini',
      name: 'Fast',
      provider: 'openai',
      base_url: '',
      model_name: 'openbitfun:gpt-5-mini',
      enabled: true,
      capabilities: []
    };
    const disabledModel: ConversationUiModel = {
      id: 'disabled-model',
      name: 'Disabled',
      provider: 'test',
      base_url: '',
      model_name: 'disabled-model',
      enabled: false,
      capabilities: []
    };
    const catalog: ConversationUiModelCatalog = {
      version: 1,
      models: [primaryModel, fastModel, disabledModel],
      default_models: { primary: primaryModel.id },
      session_model_id: fastModel.id
    };

    it('filters disabled models and resolves selection precedence', 0, () => {
      const enabled = ConversationModelPresentationPolicy.enabledModels(catalog);
      expect(enabled.length).assertEqual(2);
      expect(ConversationModelPresentationPolicy.selectedModel(catalog, primaryModel.id)?.id)
        .assertEqual(primaryModel.id);
      expect(ConversationModelPresentationPolicy.selectedModel(catalog, '')?.id)
        .assertEqual(fastModel.id);

      const defaultCatalog: ConversationUiModelCatalog = {
        version: 1,
        models: catalog.models,
        default_models: catalog.default_models
      };
      expect(ConversationModelPresentationPolicy.selectedModel(defaultCatalog, '')?.id)
        .assertEqual(primaryModel.id);
      expect(ConversationModelPresentationPolicy.selectedModel(catalog, disabledModel.id)?.id)
        .assertEqual(fastModel.id);
    });

    it('uses specific model names and preserves useful provider context', 0, () => {
      expect(ConversationModelPresentationPolicy.primaryLabel(primaryModel, 'Model'))
        .assertEqual('claude-sonnet-4');
      expect(ConversationModelPresentationPolicy.primaryLabel(fastModel, 'Model'))
        .assertEqual('gpt-5-mini');
      expect(ConversationModelPresentationPolicy.secondaryLabel(fastModel, 'Model'))
        .assertEqual('openai · Fast');
    });
  });

  describe('ConversationLayoutPolicy', () => {
    it('treats only tent statuses as hover candidates', 0, () => {
      expect(ConversationLayoutPolicy.isHoverStatus(
        ConversationLayoutPolicy.FOLD_STATUS_HALF_FOLDED
      )).assertTrue();
      expect(ConversationLayoutPolicy.isHoverStatus(
        ConversationLayoutPolicy.FOLD_STATUS_HALF_FOLDED_WITH_SECOND_HALF_FOLDED
      )).assertTrue();
      expect(ConversationLayoutPolicy.isHoverStatus(
        ConversationLayoutPolicy.FOLD_STATUS_HALF_FOLDED_WITH_SECOND_EXPANDED
      )).assertFalse();
      expect(ConversationLayoutPolicy.isHoverStatus(
        ConversationLayoutPolicy.FOLD_STATUS_EXPANDED_WITH_SECOND_HALF_FOLDED
      )).assertFalse();
      expect(ConversationLayoutPolicy.isHoverStatus(
        ConversationLayoutPolicy.FOLD_STATUS_EXPANDED_WITH_SECOND_EXPANDED
      )).assertFalse();
    });

    it('keeps one-axis half-fold landscape windows on master-detail', 0, () => {
      expect(ConversationLayoutPolicy.useHoverOperate(
        ConversationLayoutPolicy.FOLD_STATUS_EXPANDED_WITH_SECOND_HALF_FOLDED,
        1107,
        776
      )).assertFalse();
      expect(ConversationLayoutPolicy.useHoverOperate(
        ConversationLayoutPolicy.FOLD_STATUS_HALF_FOLDED_WITH_SECOND_EXPANDED,
        1107,
        776
      )).assertFalse();
      expect(ConversationLayoutPolicy.useMasterDetail(
        1107,
        true,
        false,
        [new ConversationLayoutCrease(351, 44)],
        true,
        ConversationLayoutPolicy.useHoverOperate(
          ConversationLayoutPolicy.FOLD_STATUS_EXPANDED_WITH_SECOND_HALF_FOLDED,
          1107,
          776
        )
      )).assertTrue();
    });

    it('uses FolderStack only for tent hover that is not still landscape-wide', 0, () => {
      expect(ConversationLayoutPolicy.useHoverOperate(
        ConversationLayoutPolicy.FOLD_STATUS_HALF_FOLDED,
        480,
        800
      )).assertTrue();
      expect(ConversationLayoutPolicy.useHoverOperate(
        ConversationLayoutPolicy.FOLD_STATUS_HALF_FOLDED,
        1107,
        776
      )).assertFalse();
    });

    it('keeps folded, hover and narrow surfaces compact', 0, () => {
      expect(ConversationLayoutPolicy.useMasterDetail(480, false, false, [])).assertFalse();
      expect(ConversationLayoutPolicy.useMasterDetail(900, true, true, [])).assertFalse();
      expect(ConversationLayoutPolicy.useMasterDetail(900, false, true, [
        new ConversationLayoutCrease(300, 8),
        new ConversationLayoutCrease(600, 8)
      ])).assertFalse();
      expect(ConversationLayoutPolicy.useMasterDetail(
        2210,
        true,
        false,
        [new ConversationLayoutCrease(1100, 16)],
        true,
        true
      )).assertFalse();
    });

    it('uses official md width rather than device type for master-detail', 0, () => {
      expect(ConversationLayoutPolicy.useMasterDetail(
        ConversationLayoutPolicy.MD_MIN_WIDTH +
          ConversationLayoutPolicy.MIN_DETAIL_PANE_WIDTH,
        false,
        false,
        []
      )).assertTrue();
      expect(ConversationLayoutPolicy.useMasterDetail(480, true, false, [])).assertFalse();
      expect(ConversationLayoutPolicy.useMasterDetail(
        ConversationLayoutPolicy.LG_MIN_WIDTH,
        true,
        false,
        []
      )).assertTrue();
    });

    it('uses master-detail for unfolded dual-fold surfaces', 0, () => {
      expect(ConversationLayoutPolicy.useMasterDetail(
        ConversationLayoutPolicy.LG_MIN_WIDTH,
        true,
        false,
        [new ConversationLayoutCrease(520, 8)]
      )).assertTrue();
      expect(ConversationLayoutPolicy.useMasterDetail(
        2210,
        false,
        false,
        [new ConversationLayoutCrease(1100, 16)]
      )).assertTrue();
      expect(ConversationLayoutPolicy.useMasterDetail(
        2210,
        false,
        false,
        [],
        true
      )).assertTrue();
    });

    it('keeps an unfolded crease compact when master-detail cannot fit', 0, () => {
      expect(ConversationLayoutPolicy.useMasterDetail(
        500,
        true,
        false,
        [new ConversationLayoutCrease(200, 8)]
      )).assertFalse();
    });

    it('uses master-detail for unfolded tri-fold surfaces', 0, () => {
      expect(ConversationLayoutPolicy.useMasterDetail(
        ConversationLayoutPolicy.LG_MIN_WIDTH,
        false,
        false,
        [new ConversationLayoutCrease(352, 8), new ConversationLayoutCrease(712, 8)]
      )).assertTrue();
      expect(ConversationLayoutPolicy.useMasterDetail(
        720,
        false,
        false,
        [new ConversationLayoutCrease(230, 8), new ConversationLayoutCrease(480, 8)]
      )).assertTrue();
    });

    it('synthesizes a centered hinge when expanded foldables omit crease data', 0, () => {
      const synthetic = ConversationLayoutPolicy.syntheticCenterCrease(2210);
      expect(synthetic !== undefined).assertTrue();
      expect(synthetic ? synthetic.left : 0).assertEqual(1097);
      expect(synthetic ? synthetic.width : 0).assertEqual(ConversationLayoutPolicy.SYNTHETIC_HINGE_WIDTH);
      expect(ConversationLayoutPolicy.syntheticCenterCrease(500) === undefined).assertTrue();
      expect(ConversationLayoutPolicy.effectiveVerticalCreases(500, [], true).length).assertEqual(0);
      const creases = ConversationLayoutPolicy.effectiveVerticalCreases(2210, [], true);
      expect(creases.length).assertEqual(1);
      expect(creases[0].left).assertEqual(1097);
      const realCrease = ConversationLayoutPolicy.effectiveVerticalCreases(
        2210,
        [new ConversationLayoutCrease(1100, 16)],
        true
      );
      expect(realCrease[0].left).assertEqual(1100);
    });

    it('keeps the existing master width when no usable crease is available', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(820, []);
      const invalidCrease = ConversationLayoutPolicy.resolveWideGeometry(
        820,
        [new ConversationLayoutCrease(120, 10)]
      );
      expect(geometry.masterPaneWidth).assertEqual(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH);
      expect(geometry.masterDetailGap).assertEqual(0);
      expect(geometry.isExtraWide).assertFalse();
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(476);
      expect(geometry.collapsedDetailContentOffset).assertEqual(0);
      expect(geometry.collapsedDetailContentWidth).assertEqual(820);
      expect(invalidCrease.masterPaneWidth).assertEqual(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH);
    });

    it('returns no detail content band when the viewport cannot fit the fallback master', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(300, []);
      expect(geometry.masterPaneWidth).assertEqual(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH);
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(0);
      expect(geometry.collapsedDetailContentOffset).assertEqual(0);
      expect(geometry.collapsedDetailContentWidth).assertEqual(300);
      const emptyViewport = ConversationLayoutPolicy.resolveWideGeometry(0, []);
      expect(emptyViewport.detailContentWidth).assertEqual(0);
      expect(emptyViewport.collapsedDetailContentWidth).assertEqual(0);
    });

    it('aligns the master boundary with the first usable fold crease', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        820,
        [new ConversationLayoutCrease(382, 8)]
      );
      expect(geometry.masterPaneWidth).assertEqual(382);
      expect(geometry.masterDetailGap).assertEqual(8);
      expect(geometry.isExtraWide).assertFalse();
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(430);
      expect(geometry.collapsedDetailContentOffset).assertEqual(0);
      expect(geometry.collapsedDetailContentWidth).assertEqual(820);
    });

    it('keeps collapsed dual-fold content full-width so restore stays at the window origin', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        2210,
        [new ConversationLayoutCrease(1097, 16)]
      );
      expect(geometry.collapsedDetailContentOffset).assertEqual(0);
      expect(geometry.collapsedDetailContentWidth).assertEqual(2210);
    });

    it('keeps collapsed tri-fold content full-width so restore stays at the window origin', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        1107,
        [new ConversationLayoutCrease(351, 44)]
      );
      expect(geometry.collapsedDetailContentOffset).assertEqual(0);
      expect(geometry.collapsedDetailContentWidth).assertEqual(1107);
    });

    it('treats dual-axis expanded fold status as an unfolded foldable', 0, () => {
      expect(ConversationLayoutPolicy.isFoldedStatus(
        ConversationLayoutPolicy.FOLD_STATUS_FOLDED
      )).assertTrue();
      expect(ConversationLayoutPolicy.isFoldedStatus(
        ConversationLayoutPolicy.FOLD_STATUS_EXPANDED_WITH_SECOND_EXPANDED
      )).assertFalse();
      expect(ConversationLayoutPolicy.isExpandedFoldableStatus(
        ConversationLayoutPolicy.FOLD_STATUS_EXPANDED_WITH_SECOND_EXPANDED
      )).assertTrue();
      expect(ConversationLayoutPolicy.isExpandedFoldableStatus(
        ConversationLayoutPolicy.FOLD_STATUS_FOLDED
      )).assertFalse();
    });

    it('uses the landscape display width when area reports the short side', 0, () => {
      expect(ConversationLayoutPolicy.resolveLayoutViewportWidth(776, 1107, 1107, 776))
        .assertEqual(1107);
      expect(ConversationLayoutPolicy.resolveLayoutViewportWidth(776, 1107, 776, 1107, 270))
        .assertEqual(1107);
      expect(ConversationLayoutPolicy.resolveLayoutViewportWidth(360, 780, 360, 780, 0))
        .assertEqual(360);
      expect(ConversationLayoutPolicy.resolveLayoutViewportWidth(0, 780, 360, 780, 0))
        .assertEqual(360);
    });

    it('rotates a physical horizontal crease into a vertical layout crease', 0, () => {
      const creases = ConversationLayoutPolicy.verticalCreasesFromRects(
        1107,
        776,
        [new ConversationLayoutRect(0, 351, 776, 44)]
      );
      expect(creases.length).assertEqual(1);
      expect(creases[0].left).assertEqual(351);
      expect(creases[0].width).assertEqual(44);
      const vertical = ConversationLayoutPolicy.verticalCreasesFromRects(
        680,
        740,
        [new ConversationLayoutRect(352, 0, 16, 740)]
      );
      expect(vertical.length).assertEqual(1);
      expect(vertical[0].left).assertEqual(352);
      expect(vertical[0].width).assertEqual(16);
      const rejected = ConversationLayoutPolicy.verticalCreasesFromRects(
        680,
        740,
        [
          new ConversationLayoutRect(10, 10, -4, 20),
          new ConversationLayoutRect(10, 10, 20, -4)
        ]
      );
      expect(rejected.length).assertEqual(0);
      const emptyViewport = ConversationLayoutPolicy.verticalCreasesFromRects(
        0,
        0,
        [new ConversationLayoutRect(0, 351, 776, 44)]
      );
      expect(emptyViewport.length).assertEqual(0);
      const notARotatedHinge = ConversationLayoutPolicy.verticalCreasesFromRects(
        680,
        740,
        [new ConversationLayoutRect(0, 670, 680, 20)]
      );
      expect(notARotatedHinge.length).assertEqual(0);
      const mateX7Expand = ConversationLayoutPolicy.classifyCreaseRects(
        707.2,
        706.56,
        [new ConversationLayoutRect(0, 340.48, 798.72, 54.72)]
      );
      expect(mateX7Expand.vertical.length).assertEqual(1);
      expect(mateX7Expand.vertical[0].left).assertEqual(340.48);
      expect(mateX7Expand.vertical[0].width).assertEqual(54.72);
      expect(mateX7Expand.horizontal.length).assertEqual(0);
    });

    it('keeps a true horizontal crease on its own axis', 0, () => {
      const creases = ConversationLayoutPolicy.horizontalCreasesFromRects(
        390,
        844,
        [
          new ConversationLayoutRect(0, 400, 390, 16),
          new ConversationLayoutRect(0, 120, 390, -2),
          new ConversationLayoutRect(-4, 80, -8, 12),
          new ConversationLayoutRect(10, 200, 40, 80),
          new ConversationLayoutRect(0, 90, 200, 12),
          new ConversationLayoutRect(0, 0, 390, 20),
          new ConversationLayoutRect(0, 830, 390, 20)
        ]
      );
      expect(creases.length).assertEqual(1);
      expect(creases[0].top).assertEqual(400);
      expect(creases[0].height).assertEqual(16);
      const emptyViewport = ConversationLayoutPolicy.horizontalCreasesFromRects(
        0,
        844,
        [new ConversationLayoutRect(0, 220, 200, 12)]
      );
      expect(emptyViewport.length).assertEqual(1);
      expect(emptyViewport[0].top).assertEqual(220);
      const ordered = ConversationLayoutPolicy.horizontalCreasesFromRects(
        390,
        844,
        [
          new ConversationLayoutRect(0, 520, 390, 12),
          new ConversationLayoutRect(0, 180, 390, 12)
        ]
      );
      expect(ordered.length).assertEqual(2);
      expect(ordered[0].top).assertEqual(180);
      expect(ordered[1].top).assertEqual(520);
      expect(new ConversationLayoutHorizontalCrease(180, 12).height).assertEqual(12);
      const square = ConversationLayoutPolicy.horizontalCreasesFromRects(
        390,
        844,
        [new ConversationLayoutRect(0, 200, 80, 80)]
      );
      expect(square.length).assertEqual(0);
      const zeroHeight = ConversationLayoutPolicy.horizontalCreasesFromRects(
        390,
        844,
        [new ConversationLayoutRect(0, 220, 390, 0)]
      );
      expect(zeroHeight.length).assertEqual(1);
      expect(zeroHeight[0].top).assertEqual(220);
      expect(zeroHeight[0].height).assertEqual(0);
      const tallRejected = ConversationLayoutPolicy.horizontalCreasesFromRects(
        390,
        844,
        [new ConversationLayoutRect(10, 80, 16, 400)]
      );
      expect(tallRejected.length).assertEqual(0);
      const trueHorizontal = ConversationLayoutPolicy.classifyCreaseRects(
        390,
        844,
        [new ConversationLayoutRect(0, 400, 390, 16)]
      );
      expect(trueHorizontal.vertical.length).assertEqual(0);
      expect(trueHorizontal.horizontal.length).assertEqual(1);
      expect(trueHorizontal.horizontal[0].top).assertEqual(400);
    });

    it('accepts a zero-width crease without creating an empty detail segment', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        900,
        [new ConversationLayoutCrease(352, 0)]
      );
      expect(geometry.masterPaneWidth).assertEqual(352);
      expect(geometry.masterDetailGap).assertEqual(0);
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(548);
    });

    it('keeps tri-fold detail across both remaining screens instead of one hinge-free band', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        1080,
        [new ConversationLayoutCrease(352, 8), new ConversationLayoutCrease(712, 8)]
      );
      expect(geometry.masterPaneWidth).assertEqual(352);
      expect(geometry.masterDetailGap).assertEqual(8);
      expect(geometry.isExtraWide).assertTrue();
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(720);
      expect(geometry.collapsedDetailContentOffset).assertEqual(0);
      expect(geometry.collapsedDetailContentWidth).assertEqual(1080);
    });

    it('keeps asymmetric tri-fold detail on the full remaining span after the first crease', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        1080,
        [new ConversationLayoutCrease(352, 8), new ConversationLayoutCrease(800, 8)]
      );
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(720);
      expect(geometry.collapsedDetailContentOffset).assertEqual(0);
      expect(geometry.collapsedDetailContentWidth).assertEqual(1080);
    });

    it('spans remaining screens when a second crease sits inside the detail pane', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        1107,
        [new ConversationLayoutCrease(351, 44), new ConversationLayoutCrease(773, 44)]
      );
      expect(geometry.masterPaneWidth).assertEqual(351);
      expect(geometry.masterDetailGap).assertEqual(44);
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(712);
      expect(geometry.collapsedDetailContentOffset).assertEqual(0);
      expect(geometry.collapsedDetailContentWidth).assertEqual(1107);
    });

    it('uses width as the extra-wide fallback when crease data is missing', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        ConversationLayoutPolicy.LG_MIN_WIDTH,
        []
      );
      expect(geometry.isExtraWide).assertTrue();
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(496);
    });

    it('ignores crease regions outside the application viewport', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        900,
        [
          new ConversationLayoutCrease(-12, 8),
          new ConversationLayoutCrease(420, -2),
          new ConversationLayoutCrease(910, 8)
        ]
      );
      expect(geometry.masterPaneWidth).assertEqual(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH);
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(556);
    });

    it('rejects a crease that leaves too little detail space', 0, () => {
      const geometry = ConversationLayoutPolicy.resolveWideGeometry(
        900,
        [new ConversationLayoutCrease(600, 8)]
      );
      expect(geometry.masterPaneWidth).assertEqual(ConversationLayoutPolicy.FALLBACK_MASTER_PANE_WIDTH);
      expect(geometry.detailContentOffset).assertEqual(0);
      expect(geometry.detailContentWidth).assertEqual(556);
    });
  });

  describe('ChatSessionController', () => {
    it('ignores a poll response that completes after the controller stopped', 0, async () => {
      const manager = new BlockingPollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      const errors: Object[] = [];
      const controller = new ChatSessionController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => {
          snapshots.push(snapshot);
        },
        onError: (error: Object) => {
          errors.push(error);
        },
        canPoll: (_sessionId: string) => true
      });

      controller.start('session-stale', {
        pollVersion: 0,
        knownMessageCount: 0,
        knownModelCatalogVersion: 0
      });
      await delay(20);
      controller.stop();
      manager.resolve(pollResult({
        version: 1,
        changed: true,
        sessionState: 'idle',
        title: 'Stale',
        newMessages: [chatMessage('stale-message', 'assistant', 'must not render')],
        totalMessageCount: 1
      }));
      await delay(20);

      expect(snapshots.length).assertEqual(0);
      expect(errors.length).assertEqual(0);
    });

    it('flags a transcript the desktop shortened', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      manager.results = [pollResult({
        version: 2,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [],
        totalMessageCount: 2
      }), pollResult({
        version: 3,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [chatMessage('message-6', 'assistant', 'Reply')],
        totalMessageCount: 6
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
        knownMessageCount: 5,
        knownModelCatalogVersion: 0
      });
      await delay(20);
      await controller.pollNow();
      controller.stop();

      // 2 < 5: whatever the desktop is counting is not what this session had.
      expect(snapshots[0].historyRewritten).assertTrue();
      // 6 >= 2: an ordinary tail, resumable from the offset already agreed on.
      expect(snapshots[1].historyRewritten).assertFalse();
    });

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
      const persistedAssistant = chatMessage('assistant-final-1', 'assistant', 'Final text');
      persistedAssistant.turnId = 'turn-2';
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
        activeTurn: activeChatMessage('turn-2', 'Final text', 'completed')
      })];
      controller.start('session-1', {
        pollVersion: 1,
        knownMessageCount: 1,
        knownModelCatalogVersion: 0
      }, activeChatMessage('turn-2', 'Final text', 'active'));
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
        newMessages: [persistedAssistant],
        totalMessageCount: 2
      })];
      await controller.pollNow();
      controller.stop(false);

      expect(snapshots.length).assertEqual(3);
      expect(snapshots[0].activeTurn ? snapshots[0].activeTurn.status : '').assertEqual('completed');
      expect(snapshots[0].completedTurnId).assertEqual('');
      expect(snapshots[1].activeTurn ? snapshots[1].activeTurn.id : '').assertEqual('active-turn-2');
      expect(snapshots[2].activeTurn ? snapshots[2].activeTurn.id : '').assertEqual('');
      expect(snapshots[2].completedTurnId).assertEqual('turn-2');
    });

    it('recognizes a durable final assistant from the full message snapshot', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      const persistedAssistant = chatMessage('assistant-final-3', 'assistant', 'Final text');
      persistedAssistant.turnId = 'turn-3';
      manager.results = [pollResult({
        version: 3,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [],
        messageSnapshot: [persistedAssistant],
        totalMessageCount: 2,
        activeTurn: activeChatMessage('turn-3', 'Final text', 'completed')
      })];
      const controller = new ChatSessionController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => snapshots.push(snapshot),
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      controller.start('session-3', {
        pollVersion: 2,
        knownMessageCount: 1,
        knownModelCatalogVersion: 0
      }, activeChatMessage('turn-3', 'Final text', 'active'));
      await delay(20);
      controller.stop(false);

      expect(snapshots.length).assertEqual(1);
      expect(snapshots[0].completedTurnId).assertEqual('turn-3');
      expect(snapshots[0].messageSnapshot ? snapshots[0].messageSnapshot.length : 0).assertEqual(1);
    });

    it('recognizes persisted completion when running turn transitions directly to idle', 0, async () => {
      const manager = new FakePollSessionManager();
      const snapshots: ChatSessionSnapshot[] = [];
      const persistedAssistant = chatMessage('turn-4-assistant', 'assistant', 'Final text');
      persistedAssistant.turnId = 'turn-4';
      manager.results = [pollResult({
        version: 4,
        changed: true,
        sessionState: 'idle',
        title: 'Session',
        newMessages: [persistedAssistant],
        messageSnapshot: [
          chatMessage('turn-4-user', 'user', 'Question'),
          persistedAssistant
        ],
        totalMessageCount: 2
      })];
      const controller = new ChatSessionController(manager, {
        onSnapshot: (snapshot: ChatSessionSnapshot) => snapshots.push(snapshot),
        onError: (_error: Object) => {},
        canPoll: (_sessionId: string) => true
      });

      controller.start('session-4', {
        pollVersion: 3,
        knownMessageCount: 1,
        knownModelCatalogVersion: 0
      }, activeChatMessage('turn-4', '', 'active'));
      await delay(20);
      controller.stop(false);

      expect(snapshots.length).assertEqual(1);
      expect(snapshots[0].sessionState).assertEqual('idle');
      expect(snapshots[0].activeTurn ? snapshots[0].activeTurn.id : '').assertEqual('');
      expect(snapshots[0].completedTurnId).assertEqual('turn-4');
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
