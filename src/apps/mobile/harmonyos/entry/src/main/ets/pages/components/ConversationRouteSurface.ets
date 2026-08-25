import { ConversationIntent } from '../actions/ConversationIntent';
import {
  AppRootPresentationActions,
  emptyAppRootPresentationActions
} from '../actions/AppRootPresentationActions';
import { AppRoute } from '../navigation/AppRouteContract';
import { FilePreviewPhase, FilePreviewState } from '../state/FilePreviewState';
import { GeneralChatPageState } from '../state/GeneralChatPageState';
import { RemoteCreateSessionState } from '../state/RemoteCreateSessionState';
import { RemotePageState } from '../state/RemotePageState';
import { ComposerPresentation } from './ComposerBar';
import { ConversationViewHost } from './ConversationViewHost';
import { toConversationUiModelCatalog } from '../state/ConversationUiModels';
import { RemoteCreateSessionView } from './RemoteCreateSessionView';
import {
  RemoteSurfaceHost,
  RemoteSurfaceMode,
  RemoteSurfaceState
} from './remote/RemoteSurfaceHost';
import { ConversationViewState } from '../state/ConversationViewState';
import { PAGE_BG } from './Theme';

@ComponentV2
export struct ConversationRouteSurface {
  @Param route: AppRoute = AppRoute.ChatHome;
  @Param remotePageState: RemotePageState = new RemotePageState();
  @Param remoteCreateState: RemoteCreateSessionState = new RemoteCreateSessionState();
  @Param generalPageState: GeneralChatPageState = new GeneralChatPageState();
  @Param filePreviewState: FilePreviewState = new FilePreviewState();
  @Param remoteSurfaceState: RemoteSurfaceState = new RemoteSurfaceState();
  @Param actions: AppRootPresentationActions = emptyAppRootPresentationActions();
  @Param showSidebarButton: boolean = true;
  @Param showBackButton: boolean = false;
  @Param showSidebarRestoreButton: boolean = false;
  @Param useWidePresentation: boolean = false;
  @Param hoverOperateMode: boolean = false;
  @Event onRestoreSidebar: () => void = () => {};

  build() {
    Column() {
      if (this.route === AppRoute.RemoteHome) {
        RemoteSurfaceHost({
          mode: RemoteSurfaceMode.CompactHome,
          remotePageState: this.remotePageState,
          presentationState: this.remoteSurfaceState,
          actions: this.actions,
          onOpenSidebar: this.actions.onRemoteHome.openSidebar
        })
      } else if (this.route === AppRoute.RemoteCreate) {
        RemoteCreateSessionView({
          state: this.remoteCreateState,
          presentation: this.useWidePresentation ? ComposerPresentation.Floating : ComposerPresentation.Create,
          isVoiceListening: this.remoteCreateState.isVoiceListening,
          modelCatalog: toConversationUiModelCatalog(this.remotePageState.conversation.modelCatalog),
          selectedModelId: this.remoteCreateState.selectedModelId,
          showSidebarRestoreButton: this.showSidebarRestoreButton,
          hoverOperateMode: this.hoverOperateMode,
          onRestoreSidebar: this.onRestoreSidebar,
          onBack: this.actions.onRemoteCreate.back,
          onToggleDeviceMenu: this.actions.onRemoteCreate.toggleDevices,
          onToggleWorkspaceMenu: this.actions.onRemoteCreate.toggleWorkspaces,
          onSelectDevice: this.actions.onRemoteCreate.selectDevice,
          onSelectWorkspace: (workspace) => this.actions.onRemoteCreate.selectWorkspace(workspace?.path || ''),
          onDraftChange: this.actions.onRemoteCreate.draftChanged,
          onVoiceInput: this.actions.onRemoteCreate.voiceInput,
          onSelectModel: this.actions.onRemoteCreate.selectModel,
          onSend: this.actions.onRemoteCreate.send
        })
      } else {
        ConversationViewHost({
          viewState: ConversationViewState.project(
            this.route,
            this.remotePageState,
            this.generalPageState,
            this.actions.generalStatus(),
            this.remoteCreateState
          ),
          activeFilePreviewPath: this.route === AppRoute.RemoteChat && this.filePreviewState.visible ?
            this.filePreviewState.target.remotePath : '',
          activeFilePreviewLoading: this.route === AppRoute.RemoteChat && this.filePreviewState.visible &&
            this.filePreviewState.phase === FilePreviewPhase.Loading,
          showSidebarButton: this.showSidebarButton,
          showBackButton: this.showBackButton,
          showSidebarRestoreButton: this.showSidebarRestoreButton,
          composerPresentation: this.useWidePresentation ? ComposerPresentation.Floating : ComposerPresentation.Compact,
          hoverOperateMode: this.hoverOperateMode,
          onRestoreSidebar: this.onRestoreSidebar,
          onIntent: (intent: ConversationIntent) => this.actions.onConversationIntent(this.route, intent)
        })
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }
}
