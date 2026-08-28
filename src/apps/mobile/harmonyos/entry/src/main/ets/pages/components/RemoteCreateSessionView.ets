import { RecentWorkspaceEntry } from '../../model/RemoteModels';
import { CloudAccountDevice } from '../../services/CloudAccountClient';
import { RemoteCreateSessionState, RemoteHarnessProfile } from '../state/RemoteCreateSessionState';
import { ConversationViewState } from '../state/ConversationViewState';
import { ConversationUiModelCatalog } from '../state/ConversationUiModels';
import { REMOTE_CREATE_COMPOSER_CAPABILITIES } from '../state/ChatComposerCapabilities';
import { ChatSurface } from '../state/ChatSurface';
import { ConversationIntent, ConversationIntentType } from '../actions/ConversationIntent';
import { ComposerPresentation } from './ComposerBar';
import { ConversationView } from './ConversationView';
import { ConversationViewOptions, RemoteCreateViewActions } from './ConversationViewContract';
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
        state: this.conversationState(),
        options: this.viewOptions(),
        remoteCreateState: this.state,
        createActions: this.createActions(),
        onRestoreSidebar: this.onRestoreSidebar,
        onIntent: (intent: ConversationIntent) => this.handleIntent(intent)
      })
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  private keepComposerFocused(): void {
    setTimeout(() => focusControl.requestFocus('remote-create-composer'), 30);
  }

  private conversationState(): ConversationViewState {
    const viewState = new ConversationViewState();
    viewState.surface = ChatSurface.Create;
    viewState.composerCapabilities = REMOTE_CREATE_COMPOSER_CAPABILITIES;
    viewState.chatInput = this.state.draft;
    viewState.isBusy = this.state.isSubmitting;
    viewState.inlineStatusText = this.state.errorText;
    viewState.connectionState = 'connected';
    viewState.isVoiceListening = this.isVoiceListening;
    viewState.modelCatalog = this.modelCatalog;
    viewState.selectedModelId = this.selectedModelId;
    return viewState;
  }

  private viewOptions(): ConversationViewOptions {
    const options = new ConversationViewOptions();
    options.composerPresentation = this.presentation === ComposerPresentation.Floating ?
      ComposerPresentation.Floating : ComposerPresentation.Create;
    options.composerInputId = 'remote-create-composer';
    options.showBackButton = true;
    options.showSidebarButton = false;
    options.showSidebarRestoreButton = this.showSidebarRestoreButton;
    options.hoverOperateMode = this.hoverOperateMode;
    options.showRemoteCreateContext = true;
    return options;
  }

  private createActions(): RemoteCreateViewActions {
    const actions = new RemoteCreateViewActions();
    actions.toggleDeviceMenu = (): void => this.onToggleDeviceMenu();
    actions.toggleWorkspaceMenu = (): void => this.onToggleWorkspaceMenu();
    actions.selectDevice = (device: CloudAccountDevice): void => this.onSelectDevice(device);
    actions.selectWorkspace = (workspace?: RecentWorkspaceEntry): void => this.onSelectWorkspace(workspace);
    actions.selectHarness = (profile: RemoteHarnessProfile): void => this.onSelectHarness(profile);
    actions.backgroundTap = (): void => {
      this.state.closeMenu();
      this.keepComposerFocused();
    };
    return actions;
  }

  private handleIntent(intent: ConversationIntent): void {
    if (intent.type === ConversationIntentType.Back) {
      this.onBack();
    } else if (intent.type === ConversationIntentType.Send) {
      this.onSend();
    } else if (intent.type === ConversationIntentType.ChatInputChanged) {
      this.onDraftChange(intent.value);
    } else if (intent.type === ConversationIntentType.VoiceInput) {
      this.onVoiceInput();
    } else if (intent.type === ConversationIntentType.SelectModel) {
      this.onSelectModel(intent.value);
    }
  }
}
