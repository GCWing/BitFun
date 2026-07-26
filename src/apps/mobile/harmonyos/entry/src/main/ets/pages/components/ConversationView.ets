import {
  RemoteModelCatalog,
  RemoteQuestionAnswerPayload,
  SelectedImageAttachment,
  SessionSummary
} from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { ConnectionStatusPresenter } from '../../services/ConnectionStatusPresenter';
import { ChatComposerCapabilities, REMOTE_CHAT_COMPOSER_CAPABILITIES } from './ChatComposerCapabilities';
import { ChatSurface } from './ChatSurface';
import { ChatStatusBar } from './ChatStatusBar';
import { ChatTimeline } from './ChatTimeline';
import { ConversationViewContract } from './ConversationViewContract';
import { ComposerBar } from './ComposerBar';
import { GeneralChatHeader } from './GeneralChatHeader';
import { RemoteChatHeader } from './RemoteChatHeader';
import { CARD, GREEN, INK, LINE, MUTED, PAGE_BG, RED } from './Theme';

@Component
export struct ConversationView {
  @Prop surface: ChatSurface = ChatSurface.Remote;
  @Prop activeSession: SessionSummary = {
    sessionId: '',
    title: '',
    workspacePath: '',
    agentType: 'code'
  };
  @Prop desktopName: string = '';
  @Prop workspaceBranch: string = '';
  @Prop statusText: string = '';
  @Prop inlineStatusText: string = '';
  @Prop connectionState: string = 'connected';
  @Prop composerCapabilities: ChatComposerCapabilities = REMOTE_CHAT_COMPOSER_CAPABILITIES;
  @Prop isBusy: boolean = false;
  @Prop canStop: boolean = false;
  @Prop hasMoreMessages: boolean = false;
  @Prop timelineItems: ChatTimelineItem[] = [];
  @Prop timelineRevision: number = 0;
  @Prop showSuggestionsWhenEmpty: boolean = false;
  @Prop supportsSearch: boolean = false;
  @Prop supportsImages: boolean = false;
  @Prop supportsFiles: boolean = false;
  @Prop modelCatalog: RemoteModelCatalog = {
    version: 0,
    models: [],
    default_models: {}
  };
  @Prop selectedModelId: string = '';
  @Prop downloadingFilePath: string = '';
  @Prop downloadedFilePath: string = '';
  @Prop fileDownloadStatus: string = '';
  @Prop selectedImages: SelectedImageAttachment[] = [];
  @Prop isVoiceListening: boolean = false;
  @Prop chatInput: string = '';
  onOpenSidebar: () => void = () => {};
  onBack: () => void = () => {};
  onNewSession: () => void = () => {};
  @Prop isSessionPinned: boolean = false;
  onTogglePinSession: () => void = () => {};
  onArchiveSession: () => void = () => {};
  onDeleteSession: () => void = () => {};
  onShowUploadedFiles: () => void = () => {};
  onStop: () => void = () => {};
  onLoadOlder: () => void = () => {};
  onApproveTool: (toolId: string, updatedInput?: Object) => void =
    (_toolId: string, _updatedInput?: Object) => {};
  onRejectTool: (toolId: string) => void = (_toolId: string) => {};
  onCancelTool: (toolId: string) => void = (_toolId: string) => {};
  onAnswerQuestion: (toolId: string, answers: RemoteQuestionAnswerPayload) => void =
    (_toolId: string, _answers: RemoteQuestionAnswerPayload) => {};
  onRenameSession: (title: string) => void = (_title: string) => {};
  onCopyMessage: (text: string) => void = (_text: string) => {};
  onRetryMessage: (text: string) => void = (_text: string) => {};
  onSelectModel: (modelId: string) => void = (_modelId: string) => {};
  onPickImages: () => void = () => {};
  onRemoveImage: (imageId: string) => void = (_imageId: string) => {};
  onDownloadFile: (path: string) => void = (_path: string) => {};
  onSend: () => void = () => {};
  onVoiceInput: () => void = () => {};
  onChatInputChange: (value: string) => void = (_value: string) => {};
  @State showQuickActions: boolean = false;
  @State showHeaderActions: boolean = false;

  build() {
    Stack() {
      Column() {
        this.Header()
        if (this.shouldShowStatusBar()) {
          this.ExecutionStatusBar()
        }
        if (this.shouldShowSuggestions()) {
          Blank().layoutWeight(1)
          if (!this.isVoiceListening) {
            this.PromptArea()
          }
        } else {
          this.MessageList()
        }
        if (this.inlineStatusText.length > 0) {
          Text(this.inlineStatusText)
            .fontSize(12).lineHeight(18).fontColor(MUTED).width('100%')
            .padding({ left: 20, right: 20, bottom: 8 })
        }
        this.Composer()
      }
      .width('100%').height('100%').backgroundColor(PAGE_BG)
      if (this.showQuickActions && this.composerCapabilities.supportsAttachments) {
        this.MenuBackdrop(() => { this.showQuickActions = false; })
        this.QuickActionsMenu()
      }
      if (this.showHeaderActions) {
        this.MenuBackdrop(() => { this.showHeaderActions = false; })
        this.HeaderActionsMenu()
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  Header() {
    if (this.surface === ChatSurface.General) {
      GeneralChatHeader({
        showActions: ConversationViewContract.hasRealTimelineItem(this.timelineItems),
        onOpenSidebar: () => {
          this.onOpenSidebar();
        },
        onNewSession: () => {
          this.showQuickActions = false;
          this.showHeaderActions = false;
          this.onNewSession();
        },
        onOpenActions: () => {
          this.showQuickActions = false;
          this.showHeaderActions = !this.showHeaderActions;
        }
      })
    } else {
      RemoteChatHeader({
        activeSession: this.activeSession,
        workspaceBranch: this.workspaceBranch,
        desktopName: this.desktopName,
        canStop: this.canStop,
        modelCatalog: this.modelCatalog,
        selectedModelId: this.selectedModelId,
        onBack: () => {
          this.onBack();
        },
        onNewSession: () => {
          this.onNewSession();
        },
        onStop: () => {
          this.onStop();
        },
        onRenameSession: (title: string) => {
          this.onRenameSession(title);
        },
        onSelectModel: (modelId: string) => {
          this.onSelectModel(modelId);
        }
      })
    }
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
      isBusy: this.isBusy,
      connectionState: this.connectionState,
      statusText: this.statusText,
      downloadingFilePath: this.downloadingFilePath,
      downloadedFilePath: this.downloadedFilePath,
      fileDownloadStatus: this.fileDownloadStatus,
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
      onAnswerQuestion: (toolId: string, answers: RemoteQuestionAnswerPayload) => {
        this.onAnswerQuestion(toolId, answers);
      },
      onCopyMessage: (text: string) => {
        this.onCopyMessage(text);
      },
      onRetryMessage: (text: string) => {
        this.onRetryMessage(text);
      },
      onDownloadFile: (path: string) => {
        this.onDownloadFile(path);
      }
    })
  }

  @Builder
  PromptArea() {
    Column({ space: 15 }) {
      if (this.supportsSearch) {
        this.SuggestionRow('globe', RemoteI18n.t('chatHome.research'), RemoteI18n.t('chatHome.researchPrompt'))
      }
      if (this.supportsImages) {
        this.SuggestionRow('image', RemoteI18n.t('chatHome.image'), RemoteI18n.t('chatHome.imagePrompt'))
      }
      if (this.supportsFiles) {
        this.SuggestionRow('file', RemoteI18n.t('chatHome.file'), RemoteI18n.t('chatHome.filePrompt'))
      }
    }
    .width('100%')
    .padding({ left: 24, right: 36, bottom: 18 })
  }

  @Builder
  SuggestionRow(icon: string, label: string, prompt: string) {
    Row({ space: 14 }) {
      this.SuggestionIcon(icon)
      Text(label)
        .fontSize(17)
        .fontWeight(FontWeight.Medium)
        .fontColor(MUTED)
        .layoutWeight(1)
    }
    .width('100%')
    .height(38)
    .onClick(() => {
      this.onChatInputChange(prompt);
    })
  }

  @Builder
  Composer() {
    ComposerBar({
      capabilities: this.composerCapabilities,
      chatInput: this.chatInput,
      showQuickActions: $showQuickActions,
      selectedImages: this.selectedImages,
      isBusy: this.isBusy,
      connectionState: this.connectionState,
      isVoiceListening: this.isVoiceListening,
      onPickImages: () => {
        this.onPickImages();
      },
      onRemoveImage: (imageId: string) => {
        this.onRemoveImage(imageId);
      },
      onSend: () => {
        this.onSend();
      },
      onVoiceInput: () => {
        this.onVoiceInput();
      },
      onChatInputChange: (value: string) => {
        this.onChatInputChange(value);
      }
    })
  }

  @Builder
  AttachmentPanel() {
    Column({ space: 8 }) {
      Row() {
        Text(RemoteI18n.t('chat.attachments'))
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Blank()
        Text(RemoteI18n.t('common.close'))
          .fontSize(12)
          .fontColor(MUTED)
          .onClick(() => {
            this.showQuickActions = false;
          })
      }
      .width('100%')
      this.ImageActionRow()
      Text(RemoteI18n.t('chat.quickPrompts'))
        .fontSize(11)
        .fontColor(MUTED)
        .width('100%')
      Row({ space: 8 }) {
        this.QuickActionChip(RemoteI18n.t('chat.quickExplain'), RemoteI18n.t('chat.quickExplainPrompt'))
        this.QuickActionChip(RemoteI18n.t('chat.quickContinue'), RemoteI18n.t('chat.quickContinuePrompt'))
        this.QuickActionChip(RemoteI18n.t('chat.quickSummary'), RemoteI18n.t('chat.quickSummaryPrompt'))
      }
      .width('100%')
    }
    .width('100%')
    .padding({ left: 18, right: 18, top: 12, bottom: 10 })
    .backgroundColor(CARD)
    .border({ width: { top: 1 }, color: LINE })
  }

  @Builder
  MenuBackdrop(onClose: () => void) {
    Text('')
      .width('100%').height('100%')
      .backgroundColor('#18000000')
      .zIndex(5)
      .onClick(onClose)
  }

  @Builder
  QuickActionsMenu() {
    Column({ space: 4 }) {
      this.MenuItem('gpt_home_image_glyph', '相机', () => this.onPickImages())
      this.MenuItem('gpt_home_image_glyph', '照片', () => this.onPickImages())
      this.MenuItem('remote_actions_link', '文件', () => {})
      this.MenuItem('remote_actions_chat', '插件', () => {})
      this.MenuItem('remote_actions_settings', '智能', () => {})
    }
    .width(300).padding({ left: 18, right: 18, top: 14, bottom: 14 })
    .backgroundColor('#FDFDFD')
    .borderRadius(24)
    .shadow({ radius: 24, color: '#26000000', offsetY: 8 })
    .position({ left: 28, bottom: 72 })
    .zIndex(6)
    .transition(TransitionEffect.translate({ x: 0, y: 18 })
      .combine(TransitionEffect.opacity(0))
      .animation({ duration: 220, curve: Curve.EaseOut }))
  }

  @Builder
  HeaderActionsMenu() {
    Column() {
      Text('会话')
        .fontSize(13).fontWeight(FontWeight.Medium).fontColor('#8E8E93')
        .width('100%').height(28).padding({ left: 8 })
          this.RemoteStyleMenuItem('remote_actions_check',
            this.isSessionPinned ? '取消置顶' : '置顶', () => this.onTogglePinSession(), this.isSessionPinned)
      this.RemoteStyleMenuItem('remote_actions_cloud', '已上传的文件', () => this.onShowUploadedFiles())
      Divider().strokeWidth(1).color(LINE).margin({ top: 8, bottom: 8 })
      this.RemoteStyleMenuItem('remote_actions_folder', '归档', () => this.onArchiveSession())
      this.RemoteStyleMenuItem('remote_actions_settings', '删除', () => this.onDeleteSession())
    }
    .width(330)
    .padding({ left: 16, right: 16, top: 14, bottom: 14 })
    .backgroundColor(CARD)
    .border({ width: 1, color: '#0D000000' })
    .borderRadius(20)
    .shadow({ radius: 20, color: '#1A000000', offsetY: 8 })
    .position({ right: 18, top: 14 })
    .zIndex(6)
    .transition(TransitionEffect.translate({ x: 18, y: -12 })
      .combine(TransitionEffect.opacity(0))
      .animation({ duration: 220, curve: Curve.EaseOut }))
  }

  @Builder
  MenuItem(icon: string, label: string, action: () => void) {
    Row({ space: 18 }) {
      Stack({ alignContent: Alignment.Center }) {
        Image($r('app.media.' + icon))
          .width(25).height(25).objectFit(ImageFit.Contain)
      }
      .width(40).height(40).backgroundColor('#F2F2F2').borderRadius(20)
      Text(label).fontSize(17).fontColor(INK).layoutWeight(1)
    }
    .width('100%').height(56)
    .onClick(() => {
      action();
      this.showQuickActions = false;
      this.showHeaderActions = false;
    })
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
    .backgroundColor(selected ? '#F3F3F3' : '#00000000')
    .onClick(() => {
      action()
      this.showHeaderActions = false
    })
  }

  @Builder
  MenuIcon(icon: string) {
    if (icon === 'remote_actions_link') {
      Image($r('app.media.remote_actions_link')).width(23).height(23).objectFit(ImageFit.Contain)
    } else if (icon === 'remote_actions_check') {
      Image($r('app.media.remote_actions_check')).width(23).height(23).objectFit(ImageFit.Contain)
    } else if (icon === 'remote_actions_folder') {
      Image($r('app.media.remote_actions_folder')).width(23).height(23).objectFit(ImageFit.Contain)
    } else if (icon === 'remote_actions_cloud') {
      Image($r('app.media.remote_actions_cloud')).width(23).height(23).objectFit(ImageFit.Contain)
    } else {
      Image($r('app.media.remote_actions_settings')).width(23).height(23).objectFit(ImageFit.Contain)
    }
  }

  @Builder
  ImageActionRow() {
    Row({ space: 10 }) {
      Text('+')
        .width(32)
        .height(32)
        .fontSize(21)
        .fontColor(INK)
        .textAlign(TextAlign.Center)
        .backgroundColor('#F2F1EE')
        .borderRadius(16)
      Column({ space: 3 }) {
        Text(RemoteI18n.t('chat.pickImage'))
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Text(RemoteI18n.t('chat.pickImageDesc'))
          .fontSize(11)
          .fontColor(MUTED)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
      Text('>')
        .fontSize(20)
        .fontColor(MUTED)
    }
    .width('100%')
    .height(48)
    .padding({ left: 12, right: 12 })
    .backgroundColor('#FAFAF8')
    .borderRadius(14)
    .border({ width: 1, color: LINE })
    .onClick(() => {
      this.showQuickActions = false;
      this.onPickImages();
    })
  }

  @Builder
  QuickActionChip(label: string, prompt: string) {
    Text(label)
      .fontSize(11)
      .fontColor(INK)
      .textAlign(TextAlign.Center)
      .layoutWeight(1)
      .height(30)
      .backgroundColor(CARD)
      .borderRadius(15)
      .border({ width: 1, color: LINE })
      .onClick(() => {
        this.onChatInputChange(prompt);
        this.showQuickActions = false;
      })
  }

  @Builder
  SuggestionIcon(kind: string) {
    Stack({ alignContent: Alignment.Center }) {
      if (kind === 'write') {
        Text('Aa')
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(MUTED)
      } else if (kind === 'organize') {
        Text('=')
          .fontSize(23)
          .fontColor(MUTED)
      } else if (kind === 'plan') {
        Text('✓')
          .fontSize(18)
          .fontWeight(FontWeight.Medium)
          .fontColor(MUTED)
      } else if (kind === 'globe') {
        Image($r('app.media.gpt_home_globe_glyph'))
          .width(24)
          .height(24)
          .objectFit(ImageFit.Contain)
      } else if (kind === 'image') {
        Image($r('app.media.gpt_home_image_glyph'))
          .width(24)
          .height(24)
          .objectFit(ImageFit.Contain)
      } else {
        this.FileGlyph()
      }
    }
    .width(26)
    .height(26)
  }

  @Builder
  FileGlyph() {
    Stack() {
      Text('')
        .width(19)
        .height(23)
        .borderRadius(3)
        .border({ width: 1.5, color: MUTED })
        .position({ x: 3, y: 2 })
      Text('')
        .width(7)
        .height(7)
        .border({ width: { right: 1.5, bottom: 1.5 }, color: MUTED })
        .position({ x: 15, y: 3 })
    }
    .width(26)
    .height(26)
  }

  private shouldShowSuggestions(): boolean {
    return ConversationViewContract.shouldShowSuggestions(
      this.showSuggestionsWhenEmpty,
      this.isBusy,
      this.timelineItems
    );
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

  private statusColor(): string {
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
    return this.surface === ChatSurface.Remote && this.connectionState !== 'connected';
  }

  private connectionColor(): string {
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
