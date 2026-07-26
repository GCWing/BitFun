import { ChatMessage, ChatMessageItemResponse, RemoteQuestionAnswerPayload } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ChatTimelineItem } from '../../services/ChatTimelineProjector';
import { ChatSurface } from './ChatSurface';
import { CARD, INK, LINE, MUTED, RED } from './Theme';
import { ChatMessageBubble } from './ChatMessageBubble';
import { RemoteLogger } from '../../services/RemoteLogger';

@ComponentV2
export struct ChatTimeline {
  private readonly listScroller: Scroller = new Scroller();
  @Param surface: ChatSurface = ChatSurface.Remote;
  @Param timelineItems: ChatTimelineItem[] = [];
  @Param timelineRevision: number = 0;
  @Param hasMoreMessages: boolean = false;
  @Param isBusy: boolean = false;
  @Param connectionState: string = 'connected';
  @Param statusText: string = '';
  @Param downloadingFilePath: string = '';
  @Param downloadedFilePath: string = '';
  @Param fileDownloadStatus: string = '';
  @Event onLoadOlder: () => void = () => {};
  @Event onApproveTool: (toolId: string, updatedInput?: Object) => void =
    (_toolId: string, _updatedInput?: Object) => {};
  @Event onRejectTool: (toolId: string) => void = (_toolId: string) => {};
  @Event onCancelTool: (toolId: string) => void = (_toolId: string) => {};
  @Event onAnswerQuestion: (toolId: string, answers: RemoteQuestionAnswerPayload) => void =
    (_toolId: string, _answers: RemoteQuestionAnswerPayload) => {};
  @Event onCopyMessage: (text: string) => void = (_text: string) => {};
  @Event onRetryMessage: (text: string) => void = (_text: string) => {};
  @Event onDownloadFile: (path: string) => void = (_path: string) => {};

  build() {
    Stack({ alignContent: Alignment.Bottom }) {
      List({ space: 12, scroller: this.listScroller }) {
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
          this.TimelineItem(item)
        }, (item: ChatTimelineItem) => `${this.timelineRevision}-${item.id}`)
      }
      .width('100%')
      .height('100%')
      .padding({ left: 20, right: 20, top: 0, bottom: 12 })
      .stackFromEnd(true)
      .scrollBar(BarState.Off)

      if (this.surface === ChatSurface.Remote && this.timelineItems.length > 2) {
        Stack({ alignContent: Alignment.Center }) {
          Image($r('app.media.remote_ref_down'))
            .width(18)
            .height(18)
            .objectFit(ImageFit.Contain)
        }
        .width(42)
        .height(42)
        .backgroundColor(CARD)
        .borderRadius(21)
        .shadow({ radius: 14, color: '#14000000', offsetY: 5 })
        .margin({ bottom: 4 })
        .onClick(() => {
          this.listScroller.scrollEdge(Edge.Bottom);
        })
      }
    }
    .layoutWeight(1)
    .width('100%')
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
  TimelineItem(item: ChatTimelineItem) {
    if (item.type === 'empty_state') {
      ListItem() {
        this.EmptyChatState()
      }
      .onAppear(() => {
        this.logRenderedItem(item);
      })
    } else if (item.message && this.shouldRenderMessage(item.message)) {
      ListItem() {
        this.MessageBubble(item)
      }
      .onAppear(() => {
        this.logRenderedItem(item);
      })
    }
  }

  private logRenderedItem(item: ChatTimelineItem): void {
    const message = item.message;
    RemoteLogger.info(`chat timeline item appear revision=${this.timelineRevision} type=${item.type} id=${item.id} status=${message ? message.status : ''} textLength=${message ? message.text.length : 0}`);
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
