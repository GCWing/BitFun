import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import {
  GeneralChatServiceStatus
} from '../../services/general-chat/GeneralChatServiceState';
import { RemoteUiState } from '../../services/RemoteUiState';
import {
  ChatComposerCapabilities,
  GENERAL_CHAT_COMPOSER_CAPABILITIES,
  REMOTE_CHAT_COMPOSER_CAPABILITIES
} from '../components/ChatComposerCapabilities';
import { ChatSurface } from '../components/ChatSurface';
import {
  ConversationUiModelCatalog,
  ConversationUiSelectedImage,
  ConversationUiSession,
  toConversationUiModelCatalog,
  toConversationUiSelectedImage,
  toConversationUiSession
} from '../components/ConversationUiModels';
import { AppRoute } from '../navigation/AppRouteContract';
import { GeneralChatPageState } from './GeneralChatPageState';
import { RemotePageState } from './RemotePageState';

/** Immutable presentation state consumed by ConversationView. */
export class ConversationViewState {
  activeSession: ConversationUiSession = ConversationViewState.emptySession();
  surface: ChatSurface = ChatSurface.General;
  desktopName: string = '';
  workspaceBranch: string = '';
  statusText: string = '';
  inlineStatusText: string = '';
  connectionState: string = 'idle';
  composerCapabilities: ChatComposerCapabilities = GENERAL_CHAT_COMPOSER_CAPABILITIES;
  isBusy: boolean = false;
  canStop: boolean = false;
  hasMoreMessages: boolean = false;
  timelineItems: ChatTimelineItem[] = [];
  timelineRevision: number = 0;
  showSuggestionsWhenEmpty: boolean = true;
  supportsSearch: boolean = false;
  supportsImages: boolean = false;
  supportsFiles: boolean = false;
  modelCatalog: ConversationUiModelCatalog = toConversationUiModelCatalog(RemoteUiState.emptyModelCatalog());
  selectedModelId: string = '';
  isSessionPinned: boolean = false;
  downloadingFilePath: string = '';
  downloadedFilePath: string = '';
  fileDownloadStatus: string = '';
  selectedImages: ConversationUiSelectedImage[] = [];
  isVoiceListening: boolean = false;
  chatInput: string = '';

  static project(
    route: AppRoute,
    remote: RemotePageState,
    general: GeneralChatPageState,
    generalInlineStatus: string
  ): ConversationViewState {
    if (route === AppRoute.RemoteChat) {
      return ConversationViewState.remote(remote);
    }
    return ConversationViewState.general(general, generalInlineStatus);
  }

  private static remote(remote: RemotePageState): ConversationViewState {
    const state = new ConversationViewState();
    state.activeSession = toConversationUiSession(remote.activeSession);
    state.surface = ChatSurface.Remote;
    state.desktopName = remote.desktopName;
    state.workspaceBranch = remote.workspaceBranch;
    state.statusText = remote.statusText;
    state.connectionState = remote.connectionState;
    state.composerCapabilities = REMOTE_CHAT_COMPOSER_CAPABILITIES;
    state.isBusy = remote.isBusy;
    state.canStop = remote.hasRunningActiveTurn();
    state.hasMoreMessages = remote.hasMoreMessages;
    state.timelineItems = remote.timelineItems;
    state.timelineRevision = remote.timelineRevision;
    state.showSuggestionsWhenEmpty = false;
    state.modelCatalog = toConversationUiModelCatalog(remote.modelCatalog);
    state.selectedModelId = remote.selectedModelId;
    state.downloadingFilePath = remote.downloadingFilePath;
    state.downloadedFilePath = remote.downloadedFilePath;
    state.fileDownloadStatus = remote.fileDownloadStatus;
    state.selectedImages = remote.selectedImages.map((image) => toConversationUiSelectedImage(image));
    state.isVoiceListening = remote.isVoiceListening;
    state.chatInput = remote.chatInput;
    return state;
  }

  private static general(general: GeneralChatPageState, inlineStatus: string): ConversationViewState {
    const state = new ConversationViewState();
    state.activeSession = toConversationUiSession(general.activeSession);
    state.statusText = general.statusText;
    state.inlineStatusText = inlineStatus;
    state.connectionState = GeneralChatServiceStatus.connectionState(general.serviceState);
    state.isBusy = general.isBusy;
    state.canStop = general.hasRunningActiveTurn();
    state.hasMoreMessages = general.hasMoreMessages;
    state.timelineItems = general.timelineItems;
    state.timelineRevision = general.timelineRevision;
    state.modelCatalog = toConversationUiModelCatalog(general.modelCatalog);
    state.selectedModelId = general.selectedModelId;
    state.isSessionPinned = general.activeSession.sessionId.length > 0 &&
      general.pinnedSessionId() === general.activeSession.sessionId;
    state.selectedImages = general.selectedImages.map((image) => toConversationUiSelectedImage(image));
    state.isVoiceListening = general.isVoiceListening;
    state.chatInput = general.chatInput;
    return state;
  }

  private static emptySession(): ConversationUiSession {
    return {
      sessionId: '',
      title: '',
      workspacePath: '',
      agentType: 'chat'
    };
  }
}
