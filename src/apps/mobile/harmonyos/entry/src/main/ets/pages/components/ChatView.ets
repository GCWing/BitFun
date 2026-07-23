import { RemoteModelCatalog, RemoteQuestionAnswerPayload, SelectedImageAttachment, SessionSummary } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { ConnectionStatusPresenter } from '../../services/ConnectionStatusPresenter';
import { ChatHeader } from './ChatHeader';
import { ChatStatusBar } from './ChatStatusBar';
import { ChatTimeline } from './ChatTimeline';
import { ComposerBar } from './ComposerBar';
import { CARD, GREEN, INK, LINE, MUTED, PAGE_BG, RED } from './Theme';

@Component
export struct ChatView {
  @Prop activeSession: SessionSummary = {
    sessionId: '',
    title: '',
    workspacePath: '',
    agentType: 'code'
  };
  @Prop workspaceName: string = '';
  @Prop workspaceBranch: string = '';
  @Prop statusText: string = '';
  @Prop connectionState: string = 'connected';
  @Prop isBusy: boolean = false;
  @Prop canStop: boolean = false;
  @Prop hasMoreMessages: boolean = false;
  @Prop timelineItems: ChatTimelineItem[] = [];
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
  @Link chatInput: string;
  onBack: () => void = () => {};
  onStop: () => void = () => {};
  onLoadOlder: () => void = () => {};
  onApproveTool: (toolId: string) => void = (_toolId: string) => {};
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
  @State showQuickActions: boolean = false;

  build() {
    Column() {
      this.Header()
      if (this.shouldShowStatusBar()) {
        this.ExecutionStatusBar()
      }
      this.MessageList()
      if (this.showQuickActions) {
        this.AttachmentPanel()
      }
      this.Composer()
    }
    .width('100%')
    .height('100%')
    .backgroundColor(PAGE_BG)
  }

  @Builder
  Header() {
    ChatHeader({
      activeSession: this.activeSession,
      workspaceName: this.workspaceName,
      workspaceBranch: this.workspaceBranch,
      canStop: this.canStop,
      modelCatalog: this.modelCatalog,
      selectedModelId: this.selectedModelId,
      onBack: () => {
        this.onBack();
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
      timelineItems: this.timelineItems,
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
      onApproveTool: (toolId: string) => {
        this.onApproveTool(toolId);
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
  Composer() {
    ComposerBar({
      chatInput: $chatInput,
      showQuickActions: $showQuickActions,
      selectedImages: this.selectedImages,
      isBusy: this.isBusy,
      connectionState: this.connectionState,
      isVoiceListening: this.isVoiceListening,
      onRemoveImage: (imageId: string) => {
        this.onRemoveImage(imageId);
      },
      onSend: () => {
        this.onSend();
      },
      onVoiceInput: () => {
        this.onVoiceInput();
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
      Text('›')
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
        this.chatInput = prompt;
        this.showQuickActions = false;
      })
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
    return this.connectionState !== 'connected';
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
