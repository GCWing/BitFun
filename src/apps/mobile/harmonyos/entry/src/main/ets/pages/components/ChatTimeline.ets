import { ConversationUiMessage, ConversationUiMessageItem, ConversationUiQuestionAnswer, toConversationUiMessage } from '../state/ConversationUiModels';
import { ConversationMessageRenderPolicy } from '../policy/ConversationMessageRenderPolicy';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ChatTimelineItem, ChatTimelineRevisionTracker } from '../../services/ChatTimelineProjector';
import { ChatSurface } from '../state/ChatSurface';
import { CARD, INK, LINE, MUTED, RED } from './Theme';
import { ChatMessageBubble } from './ChatMessageBubble';
import { RemoteLogger } from '../../services/RemoteLogger';

@ComponentV2
export struct ChatTimeline {
  private readonly listScroller: Scroller = new Scroller();
  // Whether new content should pull the viewport down with it. True until the
  // user scrolls up to read back through the transcript: at that point taking
  // the viewport away from them would be worse than letting the reply grow off
  // screen, and the chevron button below gives them the way back.
  @Local stickToBottom: boolean = true;
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
  @Param activeFilePreviewPath: string = '';
  @Param activeFilePreviewLoading: boolean = false;
  @Param maxContentWidth: number = 0;
  @Event onLoadOlder: () => void = () => {};
  @Event onApproveTool: (toolId: string, updatedInput?: Object) => void =
    (_toolId: string, _updatedInput?: Object) => {};
  @Event onRejectTool: (toolId: string) => void = (_toolId: string) => {};
  @Event onCancelTool: (toolId: string) => void = (_toolId: string) => {};
  @Event onAnswerQuestion: (toolId: string, answers: ConversationUiQuestionAnswer) => void =
    (_toolId: string, _answers: ConversationUiQuestionAnswer) => {};
  @Event onCopyMessage: (text: string) => void = (_text: string) => {};
  @Event onRetryMessage: (text: string) => void = (_text: string) => {};
  @Event onOpenFilePreview: (path: string, label: string) => void = (_path: string, _label: string) => {};
  @Event onDownloadFile: (path: string) => void = (_path: string) => {};

  // Says whether an open reuses this subtree or rebuilds it. A rebuild means
  // every bubble is constructed from nothing, and the cost of that is the
  // difference between the two readings.
  aboutToAppear(): void {
    RemoteLogger.info(`chat timeline mounted surface=${this.surface} items=${this.timelineItems.length}`);
    this.logTimelineProjection('mount');
  }

  aboutToDisappear(): void {
    RemoteLogger.info(`chat timeline disposed items=${this.timelineItems.length}`);
  }

  // `stackFromEnd` covers content that grows inside the last item, but a whole
  // new bubble arriving is a layout change the list does not chase on its own.
  @Monitor('timelineRevision')
  onTimelineChanged(): void {
    this.logTimelineProjection('revision');
    if (!this.stickToBottom) {
      return;
    }
    this.listScroller.scrollEdge(Edge.Bottom);
  }

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
        }, (item: ChatTimelineItem) => ChatTimelineRevisionTracker.itemSignature(item))
      }
      .width('100%')
      .height('100%')
      .padding({ left: 20, right: 20, top: 0, bottom: 12 })
      // A streaming reply grows in place rather than arriving as a new item, so
      // the bottom of the list has to be the anchor — otherwise the text the
      // agent is writing right now renders below the fold and never comes back.
      .stackFromEnd(true)
      // Keeps the read position when older messages are prepended above.
      .maintainVisibleContentPosition(true)
      .scrollBar(BarState.Off)
      .onDidScroll((_scrollOffset: number, scrollState: ScrollState) => {
        if (this.listScroller.isAtEnd()) {
          this.stickToBottom = true;
        } else if (scrollState !== ScrollState.Idle) {
          this.stickToBottom = false;
        }
      })

      if (this.surface === ChatSurface.Remote && !this.stickToBottom) {
        Stack({ alignContent: Alignment.Center }) {
          SymbolGlyph($r('sys.symbol.chevron_down'))
            .fontSize(18)
            .fontColor([INK])
        }
        .width(42)
        .height(42)
        .backgroundColor(CARD)
        .borderRadius(21)
        .shadow({ radius: 14, color: '#14000000', offsetY: 5 })
        .margin({ bottom: 4 })
        .onClick(() => {
          this.stickToBottom = true;
          this.listScroller.scrollEdge(Edge.Bottom);
        })
      }
    }
    .layoutWeight(1)
    .width('100%')
    .constraintSize({ maxWidth: this.maxContentWidth > 0 ? this.maxContentWidth : 10000 })
    .alignSelf(ItemAlign.Center)
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
    } else if (item.message && this.shouldRenderTimelineMessage(item)) {
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
    const uiMessage = message ? toConversationUiMessage(message) : undefined;
    const thinkingLength = uiMessage && uiMessage.thinking ? uiMessage.thinking.length : 0;
    const toolCount = uiMessage && uiMessage.tools ? uiMessage.tools.length : 0;
    const itemCount = uiMessage && uiMessage.items ? uiMessage.items.length : 0;
    RemoteLogger.info(
      `chat timeline item appear revision=${this.timelineRevision} type=${item.type} id=${item.id} ` +
      `status=${uiMessage ? uiMessage.status : ''} textLength=${uiMessage ? uiMessage.text.length : 0} ` +
      `thinkingLength=${thinkingLength} toolCount=${toolCount} itemCount=${itemCount} ` +
      `itemTypes=${this.itemTypeSummary(uiMessage && uiMessage.items ? uiMessage.items : [])}`
    );
  }

  private itemTypeSummary(items: ConversationUiMessageItem[]): string {
    if (items.length === 0) {
      return '';
    }
    const parts: string[] = [];
    items.forEach((entry: ConversationUiMessageItem) => {
      if (entry.tool && entry.tool.name) {
        parts.push(entry.tool.name);
      } else if ((entry.type || '').length > 0) {
        parts.push(entry.type || '');
      } else {
        parts.push('item');
      }
    });
    const summary = parts.join(',');
    if (summary.length > 80) {
      return summary.substring(0, 80);
    }
    return summary;
  }

  @Builder
  MessageBubble(item: ChatTimelineItem) {
    ChatMessageBubble({
      item: toConversationUiMessage(item.message!),
      isStreaming: item.isStreaming,
      isFinalizing: item.isFinalizing,
      showRetryAction: item.showRetryAction === true,
      isBusy: this.isBusy,
      downloadingFilePath: this.downloadingFilePath,
      downloadedFilePath: this.downloadedFilePath,
      fileDownloadStatus: this.fileDownloadStatus,
      activeFilePreviewPath: this.activeFilePreviewPath,
      activeFilePreviewLoading: this.activeFilePreviewLoading,
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

  private shouldRenderTimelineMessage(item: ChatTimelineItem): boolean {
    if (!item.message) {
      return false;
    }
    try {
      const uiMessage = toConversationUiMessage(item.message);
      const render = ConversationMessageRenderPolicy.shouldRender(uiMessage);
      RemoteLogger.info(
        `chat timeline decide type=${item.type} id=${item.id} role=${uiMessage.role} ` +
        `render=${render ? 1 : 0} text=${uiMessage.text ? uiMessage.text.length : -1} ` +
        `thinking=${uiMessage.thinking ? uiMessage.thinking.length : 0} ` +
        `tools=${uiMessage.tools ? uiMessage.tools.length : 0} ` +
        `items=${uiMessage.items ? uiMessage.items.length : 0}`
      );
      return render;
    } catch (err) {
      RemoteLogger.error(`chat timeline decide failed type=${item.type} id=${item.id} err=${err}`);
      return false;
    }
  }

  private logTimelineProjection(reason: string): void {
    const parts: string[] = [];
    this.timelineItems.forEach((item: ChatTimelineItem) => {
      const message = item.message;
      parts.push(
        `${item.type}:${item.id}:role=${message ? message.role : 'none'}:` +
        `text=${message && message.text ? message.text.length : -1}:` +
        `thinking=${message && message.thinking ? message.thinking.length : 0}:` +
        `tools=${message && message.tools ? message.tools.length : 0}:` +
        `items=${message && message.items ? message.items.length : 0}`
      );
    });
    const summary = parts.join(' | ');
    RemoteLogger.info(
      `chat timeline project reason=${reason} revision=${this.timelineRevision} ` +
      `count=${this.timelineItems.length} ${summary.length > 600 ? summary.substring(0, 600) : summary}`
    );
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
