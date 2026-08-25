import { KeyboardAvoidMode } from '@kit.ArkUI';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { ConnectionStatusPresenter } from '../../services/ConnectionStatusPresenter';
import { RemoteLogger } from '../../services/RemoteLogger';
import { ChatComposerCapabilities, REMOTE_CHAT_COMPOSER_CAPABILITIES } from '../state/ChatComposerCapabilities';
import { ChatSurface } from '../state/ChatSurface';
import { ChatStatusBar } from './ChatStatusBar';
import { ChatTimeline } from './ChatTimeline';
import { ConversationLoadingState } from './ConversationLoadingState';
import { ConversationViewContract } from './ConversationViewContract';
import {
  ConversationUiModelCatalog,
  ConversationUiQuestionAnswer,
  ConversationUiSelectedImage,
  ConversationUiSession
} from '../state/ConversationUiModels';
import { ComposerBar, ComposerPresentation } from './ComposerBar';
import { ConversationHeader } from './ConversationHeader';
import { ConversationHeaderPolicy, ConversationHeaderPresentation } from '../policy/ConversationHeaderPolicy';
import { FLOATING_PANEL_BG, GREEN, INK, LINE, MUTED, PAGE_BG, RED, SOFT } from './Theme';

@ComponentV2
export struct ConversationView {
  @Param surface: ChatSurface = ChatSurface.Remote;
  @Param activeSession: ConversationUiSession = {
    sessionId: '',
    title: '',
    workspacePath: '',
    agentType: 'code'
  };
  @Param desktopName: string = '';
  @Param workspaceBranch: string = '';
  @Param statusText: string = '';
  @Param inlineStatusText: string = '';
  @Param connectionState: string = 'connected';
  @Param composerCapabilities: ChatComposerCapabilities = REMOTE_CHAT_COMPOSER_CAPABILITIES;
  @Param isBusy: boolean = false;
  @Param isLoadingConversation: boolean = false;
  @Param canStop: boolean = false;
  @Param hasMoreMessages: boolean = false;
  @Param timelineItems: ChatTimelineItem[] = [];
  @Param timelineRevision: number = 0;
  @Param modelCatalog: ConversationUiModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @Param selectedModelId: string = '';
  @Param downloadingFilePath: string = '';
  @Param downloadedFilePath: string = '';
  @Param fileDownloadStatus: string = '';
  @Param activeFilePreviewPath: string = '';
  @Param activeFilePreviewLoading: boolean = false;
  @Param selectedImages: ConversationUiSelectedImage[] = [];
  @Param isVoiceListening: boolean = false;
  @Param chatInput: string = '';
  @Param showSidebarButton: boolean = true;
  @Param showBackButton: boolean = true;
  @Param showSidebarRestoreButton: boolean = false;
  @Param composerPresentation: ComposerPresentation = ComposerPresentation.Compact;
  @Param composerInputId: string = 'conversation-composer-input';
  @Param hoverOperateMode: boolean = false;
  @BuilderParam aboveComposer: () => void = this.EmptyBuilder;
  @Event onOpenSidebar: () => void = () => {};
  @Event onRestoreSidebar: () => void = () => {};
  @Event onBack: () => void = () => {};
  @Param isSessionPinned: boolean = false;
  @Event onTogglePinSession: () => void = () => {};
  @Event onArchiveSession: () => void = () => {};
  @Event onDeleteSession: () => void = () => {};
  @Event onShowUploadedFiles: () => void = () => {};
  @Event onStop: () => void = () => {};
  @Event onLoadOlder: () => void = () => {};
  @Event onApproveTool: (toolId: string, updatedInput?: Object) => void =
    (_toolId: string, _updatedInput?: Object) => {};
  @Event onRejectTool: (toolId: string) => void = (_toolId: string) => {};
  @Event onCancelTool: (toolId: string) => void = (_toolId: string) => {};
  @Event onAnswerQuestion: (toolId: string, answers: ConversationUiQuestionAnswer) => void =
    (_toolId: string, _answers: ConversationUiQuestionAnswer) => {};
  @Event onRenameSession: (title: string) => void = (_title: string) => {};
  @Event onCopyMessage: (text: string) => void = (_text: string) => {};
  @Event onRetryMessage: (text: string) => void = (_text: string) => {};
  @Event onOpenFilePreview: (path: string, label: string) => void = (_path: string, _label: string) => {};
  @Event onSelectModel: (modelId: string) => void = (_modelId: string) => {};
  @Event onPickImages: () => void = () => {};
  @Event onRemoveImage: (imageId: string) => void = (_imageId: string) => {};
  @Event onDownloadFile: (path: string) => void = (_path: string) => {};
  @Event onSend: () => void = () => {};
  @Event onVoiceInput: () => void = () => {};
  @Event onChatInputChange: (value: string) => void = (_value: string) => {};
  @Event onBackgroundTap: () => void = () => {};
  @Local showHeaderActions: boolean = false;
  private previousKeyboardAvoidMode: KeyboardAvoidMode = KeyboardAvoidMode.OFFSET;

  aboutToAppear(): void {
    RemoteLogger.info(`conversation view mounted surface=${this.surface}`);
    this.previousKeyboardAvoidMode = this.getUIContext().getKeyboardAvoidMode();
    this.getUIContext().setKeyboardAvoidMode(KeyboardAvoidMode.RESIZE);
  }

  aboutToDisappear(): void {
    RemoteLogger.info('conversation view disposed');
    this.getUIContext().setKeyboardAvoidMode(this.previousKeyboardAvoidMode);
  }

  build() {
    Stack() {
      Column() {
        this.Header()
        if (this.hoverOperateMode) {
          this.HoverBody()
        } else {
          this.StandardBody()
        }
      }
      .width('100%').height('100%').backgroundColor(PAGE_BG)
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  private StandardBody() {
    Column() {
      this.ConversationBody()
      this.Composer()
    }
    .layoutWeight(1)
    .width('100%')
  }

  @Builder
  private HoverBody() {
    FolderStack() {
      Column() {
        this.ConversationBody()
      }
      .width('100%')
      .height('100%')
      .padding({ bottom: 12 })
      Column() {
        this.Composer()
      }
      .width('100%')
    }
    .alignContent(Alignment.Bottom)
    .layoutWeight(1)
    .width('100%')
    .enableAnimation(true)
    .autoHalfFold(true)
  }

  @Builder
  private ConversationBody() {
    if (this.shouldShowStatusBar()) {
      this.ExecutionStatusBar()
    }
    if (this.isLoadingConversation) {
      ConversationLoadingState({
        maxContentWidth: this.composerPresentation === ComposerPresentation.Floating ? 800 : 0
      })
        .layoutWeight(1)
    } else if (this.surface === ChatSurface.Create) {
      Blank()
        .layoutWeight(1)
        .width('100%')
        .onClick(() => {
          this.onBackgroundTap();
        })
    } else if (this.shouldCenterInlineStatus()) {
      this.CenteredInlineStatus()
    } else {
      this.MessageList()
    }
    if (this.inlineStatusText.length > 0 && !this.shouldCenterInlineStatus()) {
      Text(this.inlineStatusText)
        .fontSize(12).lineHeight(18).fontColor(MUTED).width('100%')
        .padding({ left: 20, right: 20, bottom: 8 })
    }
  }

  @Builder
  Header() {
    ConversationHeader({
      title: this.headerTitle(),
      subtitle: this.headerSubtitle(),
      titleFallback: this.headerFallback(),
      allowRename: this.headerAllowRename(),
      showActions: this.headerShowActions(),
      showSidebarButton: this.showSidebarButton,
      showBackButton: this.showBackButton,
      showSidebarRestoreButton: this.showSidebarRestoreButton,
      showActionsMenu: this.showHeaderActions,
      actionsMenu: () => {
        this.HeaderActionsPopover();
      },
      onOpenSidebar: () => {
        this.onOpenSidebar();
      },
      onRestoreSidebar: () => {
        this.onRestoreSidebar();
      },
      onBack: () => {
        this.onBack();
      },
      onOpenActions: () => {
        this.showHeaderActions = !this.showHeaderActions;
      },
      onActionsMenuStateChange: (visible: boolean) => {
        this.showHeaderActions = visible;
      },
      onRenameSession: (title: string) => {
        this.onRenameSession(title);
      }
    })
  }

  @Builder
  ExecutionStatusBar() {
    ChatStatusBar({
      title: this.taskSummaryTitle(),
      detail: this.taskSummaryDetail(),
      color: this.statusColor(),
      canStop: this.canStop,
      onStop: () => {
        this.onStop();
      }
    })
  }

  @Builder
  MessageList() {
    ChatTimeline({
      surface: this.surface,
      timelineItems: this.visibleTimelineItems(),
      timelineRevision: this.timelineRevision,
      hasMoreMessages: this.hasMoreMessages,
      isBusy: this.isBusy || this.isLoadingConversation,
      connectionState: this.connectionState,
      statusText: this.statusText,
      downloadingFilePath: this.downloadingFilePath,
      downloadedFilePath: this.downloadedFilePath,
      fileDownloadStatus: this.fileDownloadStatus,
      activeFilePreviewPath: this.activeFilePreviewPath,
      activeFilePreviewLoading: this.activeFilePreviewLoading,
      maxContentWidth: this.composerPresentation === ComposerPresentation.Floating ? 800 : 0,
      onLoadOlder: () => {
        this.onLoadOlder();
      },
      onApproveTool: (toolId: string, updatedInput?: Object) => {
        this.onApproveTool(toolId, updatedInput);
      },
      onRejectTool: (toolId: string) => {
        this.onRejectTool(toolId);
      },
      onCancelTool: (toolId: string) => {
        this.onCancelTool(toolId);
      },
      onAnswerQuestion: (toolId: string, answers: ConversationUiQuestionAnswer) => {
        this.onAnswerQuestion(toolId, answers);
      },
      onCopyMessage: (text: string) => {
        this.onCopyMessage(text);
      },
      onRetryMessage: (text: string) => {
        this.onRetryMessage(text);
      },
      onOpenFilePreview: (path: string, label: string) => {
        this.onOpenFilePreview(path, label);
      },
      onDownloadFile: (path: string) => {
        this.onDownloadFile(path);
      }
    })
  }

  @Builder
  CenteredInlineStatus() {
    Stack({ alignContent: Alignment.Center }) {
      Text(this.inlineStatusText)
        .fontSize(14)
        .lineHeight(20)
        .fontColor(MUTED)
        .textAlign(TextAlign.Center)
        .maxLines(2)
    }
    .layoutWeight(1)
    .width('100%')
    .height('100%')
    .padding({ left: 32, right: 32, bottom: 48 })
  }

  @Builder
  Composer() {
    Column() {
      this.aboveComposer()
      ComposerBar({
      presentation: this.composerPresentation,
      capabilities: this.composerCapabilities,
      inputId: this.composerInputId,
      chatInput: this.chatInput,
      selectedImages: this.selectedImages,
      isBusy: this.isBusy,
      canStop: this.canStop,
      connectionState: this.connectionState,
      isVoiceListening: this.isVoiceListening,
      modelCatalog: this.modelCatalog,
      selectedModelId: this.selectedModelId,
      onPickImages: () => {
        this.onPickImages();
      },
      onRemoveImage: (imageId: string) => {
        this.onRemoveImage(imageId);
      },
      onSend: () => {
        this.onSend();
      },
      onStop: () => {
        this.onStop();
      },
      onVoiceInput: () => {
        this.onVoiceInput();
      },
      onSelectModel: (modelId: string) => {
        this.onSelectModel(modelId);
      },
      onChatInputChange: (value: string) => {
        this.onChatInputChange(value);
      }
      })
    }
    .width('100%')
  }

  @Builder
  private EmptyBuilder() {
  }

  @Builder
  HeaderActionsPopover() {
    Column() {
      this.HeaderActionsContent()
    }
    .width(292)
    .padding({ left: 12, right: 12, top: 10, bottom: 10 })
    .backgroundColor(FLOATING_PANEL_BG)
    .border({ width: 1, color: LINE })
    .borderRadius(16)
    .shadow({ radius: 18, color: LINE, offsetY: 7 })
    .transition(TransitionEffect.translate({ x: 8, y: -8 })
      .combine(TransitionEffect.opacity(0))
      .animation({ duration: 180, curve: Curve.EaseOut }))
  }

  @Builder
  HeaderActionsContent() {
    Text(RemoteI18n.t('chat.sessionSection'))
      .fontSize(13).fontWeight(FontWeight.Medium).fontColor(MUTED)
      .width('100%').height(28).padding({ left: 8 })
    if (this.surface === ChatSurface.General) {
      this.RemoteStyleMenuItem('remote_actions_check',
        this.isSessionPinned ? RemoteI18n.t('chat.unpin') : RemoteI18n.t('chat.pin'),
        () => this.onTogglePinSession(), this.isSessionPinned)
    }
    this.RemoteStyleMenuItem('remote_actions_cloud', RemoteI18n.t('chat.uploadedFiles'),
      () => this.onShowUploadedFiles())
    if (this.surface === ChatSurface.General) {
      Divider().strokeWidth(1).color(LINE).margin({ top: 8, bottom: 8 })
      this.RemoteStyleMenuItem('remote_actions_folder', RemoteI18n.t('sidebar.archive'),
        () => this.onArchiveSession())
      this.RemoteStyleMenuItem('remote_actions_settings', RemoteI18n.t('common.delete'),
        () => this.onDeleteSession())
    } else if (this.canStop) {
      Divider().strokeWidth(1).color(LINE).margin({ top: 8, bottom: 8 })
      this.RemoteStyleMenuItem('remote_actions_settings', RemoteI18n.t('chat.stop'), () => this.onStop())
    }
  }

  @Builder
  RemoteStyleMenuItem(icon: string, label: string, action: () => void, selected: boolean = false) {
    Row({ space: 10 }) {
      this.MenuIcon(icon)
      Text(label)
        .fontSize(15).fontColor(INK).layoutWeight(1)
        .maxLines(1).textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%').height(48)
    .padding({ left: 8, right: 8 })
    .borderRadius(10)
    .backgroundColor(selected ? SOFT : '#00000000')
    .onClick(() => {
      action()
      this.showHeaderActions = false
    })
  }

  @Builder
  MenuIcon(icon: string) {
    if (icon === 'remote_actions_link') {
      SymbolGlyph($r('sys.symbol.link')).fontSize(20).fontColor([MUTED]).width(23).height(23)
    } else if (icon === 'remote_actions_check') {
      SymbolGlyph($r('sys.symbol.checkmark_circle')).fontSize(20).fontColor([MUTED]).width(23).height(23)
    } else if (icon === 'remote_actions_folder') {
      SymbolGlyph($r('sys.symbol.folder')).fontSize(20).fontColor([MUTED]).width(23).height(23)
    } else if (icon === 'remote_actions_cloud') {
      SymbolGlyph($r('sys.symbol.cloud')).fontSize(20).fontColor([MUTED]).width(23).height(23)
    } else {
      SymbolGlyph($r('sys.symbol.gearshape')).fontSize(20).fontColor([MUTED]).width(23).height(23)
    }
  }

  private shouldCenterInlineStatus(): boolean {
    return this.inlineStatusText.length > 0 &&
      !this.isBusy &&
      !ConversationViewContract.hasRealTimelineItem(this.timelineItems);
  }

  private visibleTimelineItems(): ChatTimelineItem[] {
    return ConversationViewContract.visibleTimelineItems(this.timelineItems);
  }

  private statusLine(): string {
    if (this.canStop && this.connectionState === 'connected') {
      return RemoteI18n.t('chat.executing');
    }
    return ConnectionStatusPresenter.detail(this.connectionState, this.statusText, '', '');
  }

  private statusColor(): ResourceColor {
    if (this.canStop && this.connectionState === 'connected') {
      return GREEN;
    }
    return this.connectionColor();
  }

  private taskSummaryTitle(): string {
    if (this.canStop) {
      return RemoteI18n.t('chat.executing');
    }
    if (this.connectionState === 'failed') {
      return RemoteI18n.t('chat.emptyConnectionError');
    }
    if (this.connectionState === 'reconnecting') {
      return RemoteI18n.t('chat.emptyReconnecting');
    }
    return RemoteI18n.t('status.messagesSynced');
  }

  private taskSummaryDetail(): string {
    const detail = this.statusLine();
    return detail === this.taskSummaryTitle() ? '' : detail;
  }

  private shouldShowStatusBar(): boolean {
    return this.surface === ChatSurface.Remote &&
      !this.isLoadingConversation &&
      this.connectionState !== 'connected';
  }

  private headerTitle(): string {
    return this.headerPresentation().title;
  }

  private headerSubtitle(): string {
    return this.headerPresentation().subtitle;
  }

  private headerFallback(): string {
    return RemoteI18n.t(this.headerPresentation().fallbackKey);
  }

  private headerAllowRename(): boolean {
    return this.headerPresentation().allowRename;
  }

  private headerShowActions(): boolean {
    return this.headerPresentation().showActions;
  }

  private headerPresentation(): ConversationHeaderPresentation {
    return ConversationHeaderPolicy.present(
      this.surface,
      this.activeSession.title,
      this.desktopName,
      this.workspaceBranch,
      ConversationViewContract.hasRealTimelineItem(this.timelineItems)
    );
  }

  private connectionColor(): ResourceColor {
    const tone = ConnectionStatusPresenter.tone(this.connectionState);
    if (tone === 'ok') {
      return GREEN;
    }
    if (tone === 'error') {
      return RED;
    }
    return MUTED;
  }
}
