import { ConversationViewState } from '../state/ConversationViewState';
import { ConversationView } from './ConversationView';
import {
  ConversationIntent,
  ConversationIntents,
  ConversationIntentType
} from '../actions/ConversationIntent';
import { ConversationUiQuestionAnswer } from '../state/ConversationUiModels';
import { ComposerPresentation } from './ComposerBar';

@ComponentV2
export struct ConversationViewHost {
  @Param viewState: ConversationViewState = new ConversationViewState();
  @Param activeFilePreviewPath: string = '';
  @Param activeFilePreviewLoading: boolean = false;
  @Param showSidebarButton: boolean = true;
  @Param showBackButton: boolean = true;
  @Param showSidebarRestoreButton: boolean = false;
  @Param composerPresentation: ComposerPresentation = ComposerPresentation.Compact;
  @Param hoverOperateMode: boolean = false;
  @Param onRestoreSidebar: () => void = () => {};
  @Param onIntent: (intent: ConversationIntent) => void = (_intent: ConversationIntent) => {};

  build() {
    ConversationView({
      surface: this.viewState.surface,
      activeSession: this.viewState.activeSession,
      desktopName: this.viewState.desktopName,
      workspaceBranch: this.viewState.workspaceBranch,
      statusText: this.viewState.statusText,
      inlineStatusText: this.viewState.inlineStatusText,
      connectionState: this.viewState.connectionState,
      composerCapabilities: this.viewState.composerCapabilities,
      isBusy: this.viewState.isBusy,
      isLoadingConversation: this.viewState.isLoadingConversation,
      canStop: this.viewState.canStop,
      hasMoreMessages: this.viewState.hasMoreMessages,
      timelineItems: this.viewState.timelineItems,
      timelineRevision: this.viewState.timelineRevision,
      modelCatalog: this.viewState.modelCatalog,
      selectedModelId: this.viewState.selectedModelId,
      downloadingFilePath: this.viewState.downloadingFilePath,
      downloadedFilePath: this.viewState.downloadedFilePath,
      fileDownloadStatus: this.viewState.fileDownloadStatus,
      activeFilePreviewPath: this.activeFilePreviewPath,
      activeFilePreviewLoading: this.activeFilePreviewLoading,
      selectedImages: this.viewState.selectedImages,
      isVoiceListening: this.viewState.isVoiceListening,
      chatInput: this.viewState.chatInput,
      isSessionPinned: this.viewState.isSessionPinned,
      showSidebarButton: this.showSidebarButton,
      showBackButton: this.showBackButton,
      showSidebarRestoreButton: this.showSidebarRestoreButton,
      composerPresentation: this.composerPresentation,
      hoverOperateMode: this.hoverOperateMode,
      onOpenSidebar: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.OpenSidebar)),
      onRestoreSidebar: this.onRestoreSidebar,
      onBack: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.Back)),
      onTogglePinSession: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.TogglePin)),
      onArchiveSession: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.Archive)),
      onDeleteSession: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.Delete)),
      onShowUploadedFiles: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.ShowUploadedFiles)),
      onStop: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.Stop)),
      onLoadOlder: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.LoadOlder)),
      onApproveTool: (id: string, input?: Object) => this.dispatch(ConversationIntents.approveTool(id, input)),
      onRejectTool: (id: string) => this.dispatch(ConversationIntents.tool(ConversationIntentType.RejectTool, id)),
      onCancelTool: (id: string) => this.dispatch(ConversationIntents.tool(ConversationIntentType.CancelTool, id)),
      onAnswerQuestion: (id: string, answers: ConversationUiQuestionAnswer) =>
        this.dispatch(ConversationIntents.answerQuestion(id, answers)),
      onRenameSession: (title: string) => this.dispatch(ConversationIntents.value(ConversationIntentType.RenameSession, title)),
      onCopyMessage: (text: string) => this.dispatch(ConversationIntents.value(ConversationIntentType.CopyMessage, text)),
      onRetryMessage: (text: string) => this.dispatch(ConversationIntents.value(ConversationIntentType.RetryMessage, text)),
      onOpenFilePreview: (path: string, label: string) =>
        this.dispatch(ConversationIntents.openFilePreview(path, label)),
      onSelectModel: (id: string) => this.dispatch(ConversationIntents.value(ConversationIntentType.SelectModel, id)),
      onPickImages: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.PickImages)),
      onRemoveImage: (id: string) => this.dispatch(ConversationIntents.value(ConversationIntentType.RemoveImage, id)),
      onDownloadFile: (path: string) => this.dispatch(ConversationIntents.value(ConversationIntentType.DownloadFile, path)),
      onSend: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.Send)),
      onVoiceInput: () => this.dispatch(ConversationIntents.simple(ConversationIntentType.VoiceInput)),
      onChatInputChange: (value: string) => this.dispatch(ConversationIntents.value(ConversationIntentType.ChatInputChanged, value))
    })
  }

  private dispatch(intent: ConversationIntent): void {
    this.onIntent(intent);
  }
}
