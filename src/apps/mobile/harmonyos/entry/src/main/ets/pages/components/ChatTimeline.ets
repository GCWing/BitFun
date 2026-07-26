import { ChatMessage, ChatMessageItemResponse, RemoteQuestionAnswerPayload } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { CARD, INK, LINE, MUTED, RED } from './Theme';
import { ChatMessageBubble } from './ChatMessageBubble';

@Component
export struct ChatTimeline {
  @Prop timelineItems: ChatTimelineItem[] = [];
  @Prop hasMoreMessages: boolean = false;
  @Prop isBusy: boolean = false;
  @Prop connectionState: string = 'connected';
  @Prop statusText: string = '';
  @Prop downloadingFilePath: string = '';
  @Prop downloadedFilePath: string = '';
  @Prop fileDownloadStatus: string = '';
  onLoadOlder: () => void = () => {};
  onApproveTool: (toolId: string) => void = (_toolId: string) => {};
  onRejectTool: (toolId: string) => void = (_toolId: string) => {};
  onCancelTool: (toolId: string) => void = (_toolId: string) => {};
  onAnswerQuestion: (toolId: string, answers: RemoteQuestionAnswerPayload) => void =
    (_toolId: string, _answers: RemoteQuestionAnswerPayload) => {};
  onCopyMessage: (text: string) => void = (_text: string) => {};
  onRetryMessage: (text: string) => void = (_text: string) => {};
  onDownloadFile: (path: string) => void = (_path: string) => {};

  build() {
    List({ space: 12 }) {
      if (this.hasMoreMessages) {
        ListItem() {
          Text(this.isBusy ? RemoteI18n.t('common.loading') : RemoteI18n.t('chat.loadOlder'))
            .fontSize(13)
            .fontColor(MUTED)
            .textAlign(TextAlign.Center)
            .width('100%')
            .padding({ top: 10, bottom: 10 })
            .onClick(() => {
              this.onLoadOlder();
            })
        }
      }
      ForEach(this.timelineItems, (item: ChatTimelineItem) => {
        if (item.type === 'empty_state') {
          ListItem() {
            this.EmptyChatState()
          }
        } else if (item.message && this.shouldRenderMessage(item.message)) {
          ListItem() {
            this.MessageBubble(item)
          }
        }
      }, (item: ChatTimelineItem) => item.id)
    }
    .layoutWeight(1)
    .padding({ left: 20, right: 20, top: 0, bottom: 12 })
    .stackFromEnd(true)
    .scrollBar(BarState.Off)
  }

  @Builder
  EmptyChatState() {
    Row({ space: 12 }) {
      Text(this.emptyChatIcon())
        .width(38)
        .height(38)
        .fontSize(18)
        .fontColor(this.connectionState === 'failed' ? RED : MUTED)
        .textAlign(TextAlign.Center)
        .backgroundColor(CARD)
        .borderRadius(14)
        .border({ width: 1, color: LINE })
      Column({ space: 5 }) {
        Text(this.emptyChatTitle())
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
        Text(this.emptyChatText())
          .fontSize(13)
          .lineHeight(18)
          .fontColor(MUTED)
      }
      .layoutWeight(1)
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .padding(14)
    .backgroundColor(CARD)
    .borderRadius(18)
    .border({ width: 1, color: LINE })
  }

  @Builder
  MessageBubble(item: ChatTimelineItem) {
    ChatMessageBubble({
      item: item.message as ChatMessage,
      isStreaming: item.isStreaming,
      isFinalizing: item.isFinalizing,
      isBusy: this.isBusy,
      downloadingFilePath: this.downloadingFilePath,
      downloadedFilePath: this.downloadedFilePath,
      fileDownloadStatus: this.fileDownloadStatus,
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

  private shouldRenderMessage(item: ChatMessage): boolean {
    return item.role === 'user' ||
      this.visibleMessageText(item).length > 0 ||
      (item.thinking || '').trim().length > 0 ||
      (item.tools && item.tools.length > 0) ||
      this.hasRenderableItems(item.items || []) ||
      (item.images && item.images.length > 0) ||
      (item.status || '').toLowerCase() === 'active' ||
      item.status === 'failed';
  }

  private hasRenderableItems(items: ChatMessageItemResponse[]): boolean {
    return items.some((entry: ChatMessageItemResponse) => {
      if ((entry.content || '').trim().length > 0 || !!entry.tool) {
        return true;
      }
      return !!entry.subItems && this.hasRenderableItems(entry.subItems);
    });
  }

  private visibleMessageText(item: ChatMessage): string {
    const text = item.text.trim();
    if (text === '(空消息)' || text === '(empty message)') {
      return '';
    }
    return text;
  }

  private emptyChatIcon(): string {
    if (this.isBusy) {
      return '◌';
    }
    if (this.connectionState === 'failed') {
      return '!';
    }
    if (this.connectionState === 'reconnecting') {
      return '◌';
    }
    return '∅';
  }

  private emptyChatTitle(): string {
    if (this.isBusy) {
      return RemoteI18n.t('chat.emptySyncing');
    }
    if (this.connectionState === 'failed') {
      return RemoteI18n.t('chat.emptyConnectionError');
    }
    if (this.connectionState === 'reconnecting') {
      return RemoteI18n.t('chat.emptyReconnecting');
    }
    return RemoteI18n.t('chat.emptyNoMessages');
  }

  private emptyChatText(): string {
    if (this.isBusy) {
      return this.statusText || RemoteI18n.t('chat.emptyLoadingMessages');
    }
    if (this.connectionState === 'failed') {
      return this.statusText || RemoteI18n.t('chat.emptyReconnectFromHome');
    }
    if (this.connectionState === 'reconnecting') {
      return RemoteI18n.t('chat.emptyReconnectSync');
    }
    return RemoteI18n.t('chat.emptyFirstMessage');
  }
}
