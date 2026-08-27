import { RecentWorkspaceEntry } from '../../model/RemoteModels';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { RemoteCreateSessionState, RemoteHarnessProfile } from '../state/RemoteCreateSessionState';
import { ConversationUiModelCatalog } from '../state/ConversationUiModels';
import { REMOTE_CREATE_COMPOSER_CAPABILITIES } from '../state/ChatComposerCapabilities';
import { ChatSurface } from '../state/ChatSurface';
import { ComposerPresentation } from './ComposerBar';
import { ConversationView } from './ConversationView';
import { PAGE_BG } from './Theme';

@ComponentV2
export struct RemoteCreateSessionView {
  @Param state: RemoteCreateSessionState = new RemoteCreateSessionState();
  @Param presentation: ComposerPresentation = ComposerPresentation.Create;
  @Param showSidebarRestoreButton: boolean = false;
  @Param hoverOperateMode: boolean = false;
  @Param isVoiceListening: boolean = false;
  @Param modelCatalog: ConversationUiModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @Param selectedModelId: string = '';
  @Event onBack: () => void = () => {};
  @Event onRestoreSidebar: () => void = () => {};
  @Event onToggleDeviceMenu: () => void = () => {};
  @Event onToggleWorkspaceMenu: () => void = () => {};
  @Event onSelectDevice: (device: CloudAccountDevice) => void = (_device: CloudAccountDevice) => {};
  @Event onSelectWorkspace: (workspace?: RecentWorkspaceEntry) => void = (_workspace?: RecentWorkspaceEntry) => {};
  @Event onDraftChange: (value: string) => void = (_value: string) => {};
  @Event onSend: () => void = () => {};
  @Event onVoiceInput: () => void = () => {};
  @Event onSelectModel: (modelId: string) => void = (_modelId: string) => {};
  @Event onSelectHarness: (profile: RemoteHarnessProfile) => void = (_profile: RemoteHarnessProfile) => {};

  aboutToAppear(): void {
    setTimeout(() => this.keepComposerFocused(), 180);
  }

  build() {
    Column() {
      ConversationView({
        surface: ChatSurface.Create,
        composerCapabilities: REMOTE_CREATE_COMPOSER_CAPABILITIES,
        composerPresentation: this.presentation === ComposerPresentation.Floating ?
          ComposerPresentation.Floating : ComposerPresentation.Create,
        composerInputId: 'remote-create-composer',
        chatInput: this.state.draft,
        isBusy: this.state.isSubmitting,
        inlineStatusText: this.state.errorText,
        connectionState: 'connected',
        isVoiceListening: this.isVoiceListening,
        modelCatalog: this.modelCatalog,
        selectedModelId: this.selectedModelId,
        showBackButton: true,
        showSidebarButton: false,
        showSidebarRestoreButton: this.showSidebarRestoreButton,
        hoverOperateMode: this.hoverOperateMode,
        showRemoteCreateContext: true,
        remoteCreateState: this.state,
        onToggleCreateDeviceMenu: this.onToggleDeviceMenu,
        onToggleCreateWorkspaceMenu: this.onToggleWorkspaceMenu,
        onSelectCreateDevice: this.onSelectDevice,
        onSelectCreateWorkspace: this.onSelectWorkspace,
        onSelectCreateHarness: this.onSelectHarness,
        onBack: this.onBack,
        onRestoreSidebar: this.onRestoreSidebar,
        onSend: this.onSend,
        onChatInputChange: this.onDraftChange,
        onVoiceInput: this.onVoiceInput,
        onSelectModel: this.onSelectModel,
        onBackgroundTap: () => {
          this.state.closeMenu();
          this.keepComposerFocused();
        }
      })
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  private keepComposerFocused(): void {
    setTimeout(() => focusControl.requestFocus('remote-create-composer'), 30);
  }
}
