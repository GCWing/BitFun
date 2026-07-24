import { ChatMessage, ChatMessageItemResponse, ImageAttachment, RemoteQuestionAnswerPayload, RemoteToolStatusResponse } from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ACCENT, CARD, INK, LINE, MUTED, RED, SOFT } from './Theme';
import { FileReferenceCard } from './FileReferenceCard';
import { MarkdownContent } from './MarkdownContent';
import { MarkdownParser, ParsedMarkdownBlock, ParsedMarkdownInline, ParsedMarkdownListItem } from '../../services/MarkdownParser';
import { StreamingMarkdownContent } from './StreamingMarkdownContent';
import { ThinkingBlock } from './ThinkingBlock';
import { ToolStatusList } from './ToolStatusList';

interface FileReference {
  id: string;
  path: string;
  label: string;
}

interface StructuredRenderGroup {
  type: string;
  items: ChatMessageItemResponse[];
  itemStatuses: string[];
  itemStreaming: boolean[];
  itemChildActiveScopes: boolean[];
  tools: RemoteToolStatusResponse[];
  key: string;
  path: string;
}

interface ActivityGroupStats {
  thinkingCount: number;
  readCount: number;
  searchCount: number;
  otherCount: number;
}

@Component
export struct ChatMessageBubble {
  @Prop item: ChatMessage = {
    id: '',
    role: 'assistant',
    text: '',
    status: '',
    detail: ''
  };
  @Prop isStreaming: boolean = false;
  @Prop isFinalizing: boolean = false;
  @Prop isBusy: boolean = false;
  @Prop downloadingFilePath: string = '';
  @Prop downloadedFilePath: string = '';
  @Prop fileDownloadStatus: string = '';
  onApproveTool: (toolId: string) => void = (_toolId: string) => {};
  onRejectTool: (toolId: string) => void = (_toolId: string) => {};
  onCancelTool: (toolId: string) => void = (_toolId: string) => {};
  onAnswerQuestion: (toolId: string, answers: RemoteQuestionAnswerPayload) => void =
    (_toolId: string, _answers: RemoteQuestionAnswerPayload) => {};
  onCopyMessage: (text: string) => void = (_text: string) => {};
  onRetryMessage: (text: string) => void = (_text: string) => {};
  onDownloadFile: (path: string) => void = (_path: string) => {};
  @State expandedActivityPath: string = '';

  build() {
    if (this.item.role === 'user') {
      this.UserBubble()
    } else {
      this.AssistantBubble()
    }
  }

  @Builder
  UserBubble() {
    Row() {
      Blank()
      Column({ space: 8 }) {
        if (this.visibleMessageText(this.item).length > 0) {
          Text(this.visibleMessageText(this.item))
            .fontSize(14)
            .lineHeight(20)
            .fontColor(INK)
            .padding({ left: 16, right: 16, top: 10, bottom: 10 })
            .backgroundColor(SOFT)
            .borderRadius(21)
        }
        if (this.item.images && this.item.images.length > 0) {
          this.MessageImages(this.item.images)
        }
        if (this.item.status === 'failed') {
          Row({ space: 8 }) {
            Text(RemoteI18n.t('chat.sendFailed'))
              .fontSize(12)
              .fontColor(RED)
            Text(RemoteI18n.t('common.retry'))
              .fontSize(12)
              .fontColor(CARD)
              .height(28)
              .padding({ left: 10, right: 10 })
              .backgroundColor(ACCENT)
              .borderRadius(14)
              .onClick(() => {
                this.onRetryMessage(this.item.text);
              })
          }
        }
      }
      .width('70%')
      .alignItems(HorizontalAlign.End)
    }
    .width('100%')
    .padding({ top: 8, bottom: 12 })
  }

  @Builder
  AssistantBubble() {
    Row() {
      Column({ space: 10 }) {
        if (this.shouldRenderFinalOnly(this.item)) {
          this.FinalContent(this.item)
        } else {
          this.ProcessContent(this.item)
        }
        if (this.item.images && this.item.images.length > 0) {
          this.MessageImages(this.item.images)
        }
      }
      .layoutWeight(1)
      .padding({ right: 2 })
      .alignItems(HorizontalAlign.Start)
    }
    .width('100%')
    .alignItems(VerticalAlign.Top)
    .padding({ top: 2, bottom: 10 })
  }

  @Builder
  FinalContent(item: ChatMessage) {
    this.MessageText(this.finalMessageText(item), false, `${item.id}-final`)
    if (this.fileReferences(this.finalMessageText(item)).length > 0) {
      this.FileCards(this.finalMessageText(item))
    }
  }

  @Builder
  ProcessContent(item: ChatMessage) {
    if (this.shouldRenderStructuredItems(item)) {
      this.StructuredItems(item.items || [], this.shouldOmitStructuredThinking(item))
      if (this.uncoveredTools(item).length > 0) {
        this.Tools(this.uncoveredTools(item))
      }
    } else {
      if (item.role !== 'user' && this.visibleMessageText(item).length > 0 && this.visibleMessageText(item) !== item.thinking) {
        this.MessageText(this.visibleMessageText(item), this.isStreamingAssistant(), `${item.id}-body`)
        if (this.fileReferences(this.visibleMessageText(item)).length > 0) {
          this.FileCards(this.visibleMessageText(item))
        }
      }
      if (this.shouldShowTypingDots(item)) {
        this.TypingDots()
      }
      if (item.tools && item.tools.length > 0) {
        this.Tools(item.tools)
      }
    }
    if (this.pinnedThinkingText(item).length > 0) {
      this.Thinking(
        this.pinnedThinkingText(item),
        item.status,
        `${item.id}-pinned-thinking`,
        this.isStreamingAssistant()
      )
    }
  }

  @Builder
  AssistantAvatar() {
    Row({ space: 4 }) {
      Text('')
        .width(6)
        .height(6)
        .backgroundColor(CARD)
        .borderRadius(3)
      Text('')
        .width(6)
        .height(6)
        .backgroundColor(CARD)
        .borderRadius(3)
    }
    .width(32)
    .height(32)
    .backgroundColor(ACCENT)
    .borderRadius(16)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  StructuredItems(items: ChatMessageItemResponse[], omitActiveThinking: boolean = false) {
    Column({ space: 10 }) {
      ForEach(this.structuredGroups(items, 'item'), (group: StructuredRenderGroup) => {
        this.StructuredGroup(group, omitActiveThinking)
      }, (group: StructuredRenderGroup) => group.key)
    }
    .width('100%')
  }

  @Builder
  StructuredGroup(group: StructuredRenderGroup, omitActiveThinking: boolean = false) {
    if (group.type === 'activity_group') {
      this.ActivityGroup(group, omitActiveThinking)
    } else if (group.type === 'tool_group') {
      this.Tools(group.tools)
      if (group.items.length > 0) {
        ForEach(group.items, (entry: ChatMessageItemResponse, index: number) => {
          this.StructuredItem(
            entry,
            `${group.path}-adjacent-${index}`,
            group.itemStatuses[index] || '',
            group.itemStreaming[index] || false,
            group.itemChildActiveScopes[index] || false
          )
        }, (entry: ChatMessageItemResponse, index: number) => {
          return this.structuredItemKey(entry, `${group.path}-adjacent-${index}`);
        })
      }
    } else if (group.items.length > 0) {
      this.StructuredItem(
        group.items[0],
        group.path,
        group.itemStatuses[0] || '',
        group.itemStreaming[0] || false,
        group.itemChildActiveScopes[0] || false
      )
    }
  }

  @Builder
  ActivityGroup(group: StructuredRenderGroup, omitActiveThinking: boolean = false) {
    Column({ space: 6 }) {
      if (this.activityGroupTools(group).length > 0) {
        this.Tools(this.activityGroupTools(group))
      }
      ForEach(group.items, (entry: ChatMessageItemResponse, index: number) => {
        if (!omitActiveThinking &&
          this.isRenderableActivityThinking(entry, group.itemStatuses[index] || '', group.itemStreaming[index] || false)) {
          this.Thinking(
            entry.content || '',
            group.itemStatuses[index] || this.item.status,
            `${this.item.id}-${group.path}-activity-${index}-thinking`,
            group.itemStreaming[index] || false
          )
        }
      }, (entry: ChatMessageItemResponse, index: number) => {
        return this.structuredItemKey(entry, `${group.path}-activity-thinking-${index}`);
      })
    }
    .width('100%')
  }

  @Builder
  StructuredItem(
    entry: ChatMessageItemResponse,
    path: string,
    statusOverride: string = '',
    streaming: boolean = false,
    childActiveScope: boolean = false
  ) {
    if (this.isSubagentEntry(entry)) {
      this.SubagentGroup(entry, path, childActiveScope)
    } else {
      if (this.isThinkingEntry(entry)) {
        this.Thinking(
          entry.content || '',
          statusOverride || this.item.status,
          `${this.item.id}-${path}-thinking`,
          streaming
        )
      } else if (this.isTextEntry(entry)) {
        this.MessageText(entry.content || '', streaming, `${this.item.id}-${path}-text`)
        if (this.fileReferences(entry.content || '').length > 0) {
          this.FileCards(entry.content || '')
        }
      } else if (entry.tool) {
        this.Tools([entry.tool])
      }
      if (entry.subItems && entry.subItems.length > 0) {
        this.ChildItems(entry.subItems, path, childActiveScope)
      }
    }
  }

  @Builder
  SubagentGroup(entry: ChatMessageItemResponse, path: string, activeScope: boolean = false) {
    Column({ space: 8 }) {
      Row({ space: 8 }) {
        Text('↳')
          .width(18)
          .fontSize(13)
          .fontColor(MUTED)
          .textAlign(TextAlign.Center)
        Text(this.subagentTitle(entry))
          .fontSize(13)
          .fontWeight(FontWeight.Medium)
          .fontColor(INK)
          .layoutWeight(1)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }
      .width('100%')
      if (entry.content && entry.content.trim().length > 0 && !this.isTextEntry(entry) && !this.isThinkingEntry(entry)) {
        this.MessageText(entry.content, activeScope && (!entry.subItems || entry.subItems.length === 0), `${this.item.id}-${path}-subagent`)
      }
      if (entry.subItems && entry.subItems.length > 0) {
        this.ChildItems(entry.subItems, path, activeScope)
      }
    }
    .width('100%')
    .padding({ left: 10, right: 10, top: 8, bottom: 8 })
    .backgroundColor('#F7F7F4')
    .borderRadius(12)
    .border({ width: 1, color: LINE })
  }

  @Builder
  ChildItems(items: ChatMessageItemResponse[], parentPath: string, activeScope: boolean = false) {
    Column({ space: 8 }) {
      ForEach(this.structuredGroups(items, parentPath, activeScope), (group: StructuredRenderGroup) => {
        this.StructuredGroup(group)
      }, (group: StructuredRenderGroup) => group.key)
    }
    .width('100%')
    .padding({ left: 12 })
    .border({ width: { left: 1 }, color: LINE })
  }

  @Builder
  Thinking(text: string, status: string, streamKey: string = '', streaming: boolean = false) {
    ThinkingBlock({
      text,
      status,
      keepExpandedWhenDone: false,
      streaming,
      streamKey,
      onCopyText: (body: string) => {
        this.onCopyMessage(body);
      }
    })
  }

  @Builder
  Tools(tools: RemoteToolStatusResponse[]) {
    ToolStatusList({
      tools,
      isBusy: this.isBusy,
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
      }
    })
  }

  @Builder
  TypingDots() {
    Row({ space: 5 }) {
      Text('•')
        .fontSize(16)
        .fontColor(MUTED)
      Text('•')
        .fontSize(16)
        .fontColor(MUTED)
      Text('•')
        .fontSize(16)
        .fontColor(MUTED)
    }
    .height(24)
    .padding({ left: 2 })
  }

  @Builder
  MessageImages(images: ImageAttachment[]) {
    Flex({ direction: FlexDirection.Row, wrap: FlexWrap.Wrap }) {
      ForEach(images, (image: ImageAttachment) => {
        Image(image.data_url)
          .width(92)
          .height(92)
          .objectFit(ImageFit.Cover)
          .borderRadius(14)
          .border({ width: 1, color: LINE })
          .margin({ right: 8, bottom: 8 })
      }, (image: ImageAttachment) => image.name)
    }
    .width('100%')
  }

  @Builder
  MessageText(text: string, active: boolean = false, streamKey: string = '') {
    if (active) {
      StreamingMarkdownContent({
        text,
        active,
        streamKey,
        onCopyText: (body: string) => {
          this.onCopyMessage(body);
        }
      })
    } else {
      MarkdownContent({
        text,
        onCopyText: (body: string) => {
          this.onCopyMessage(body);
        }
      })
    }
  }

  @Builder
  FileCards(text: string) {
    Column({ space: 8 }) {
      ForEach(this.fileReferences(text), (file: FileReference) => {
        FileReferenceCard({
          path: file.path,
          label: file.label,
          status: this.fileStatus(file.path),
          buttonLabel: this.fileButtonLabel(file.path),
          disabled: this.downloadingFilePath === file.path,
          onDownload: (path: string) => {
            this.onDownloadFile(path);
          }
        })
      }, (file: FileReference) => file.id)
    }
    .width('100%')
  }

  private visibleMessageText(item: ChatMessage): string {
    const text = item.text.trim();
    if (text === '(空消息)' || text === '(empty message)') {
      return '';
    }
    return text;
  }

  private shouldShowTypingDots(item: ChatMessage): boolean {
    if (item.role === 'user' || (item.status || '').toLowerCase() !== 'active') {
      return false;
    }
    return this.visibleMessageText(item).length === 0 &&
      (!item.thinking || item.thinking.trim().length === 0) &&
      (!item.tools || item.tools.length === 0);
  }

  private isStreamingAssistant(): boolean {
    return this.item.role !== 'user' && (this.isStreaming || (this.item.status || '').toLowerCase() === 'active');
  }

  private shouldRenderStructuredItems(item: ChatMessage): boolean {
    if (item.role === 'user' || !item.items || item.items.length === 0) {
      return false;
    }
    return item.items.some((entry: ChatMessageItemResponse) => {
      return this.isRenderableStructuredEntry(entry);
    });
  }

  private uncoveredTools(item: ChatMessage): RemoteToolStatusResponse[] {
    return this.toolsNotCoveredByItems(item.tools || [], item.items || []);
  }

  private shouldPinThinkingToBottom(item: ChatMessage): boolean {
    return this.isStreamingAssistant() &&
      !this.hasVisibleAssistantOutput(item) &&
      this.currentThinkingText(item).length > 0;
  }

  private pinnedThinkingText(item: ChatMessage): string {
    if (!this.shouldPinThinkingToBottom(item)) {
      return '';
    }
    return this.currentThinkingText(item);
  }

  private shouldOmitStructuredThinking(item: ChatMessage): boolean {
    return this.shouldPinThinkingToBottom(item) ||
      (this.isStreamingAssistant() && this.hasVisibleAssistantOutput(item));
  }

  private hasVisibleAssistantOutput(item: ChatMessage): boolean {
    if (this.lastTopLevelText(item.items || []).length > 0) {
      return true;
    }
    const visible = this.visibleMessageText(item);
    return visible.length > 0 && visible !== (item.thinking || '').trim();
  }

  private currentThinkingText(item: ChatMessage): string {
    const structured = this.lastThinkingText(item.items || []);
    if (structured.length > 0) {
      return structured;
    }
    return (item.thinking || '').trim();
  }

  private lastThinkingText(items: ChatMessageItemResponse[]): string {
    for (let index = items.length - 1; index >= 0; index--) {
      const entry = items[index];
      if (this.isThinkingEntry(entry)) {
        return (entry.content || '').trim();
      }
      if (!this.isSubagentEntry(entry) && entry.subItems) {
        const childText = this.lastThinkingText(entry.subItems);
        if (childText.length > 0) {
          return childText;
        }
      }
    }
    return '';
  }

  private shouldRenderFinalOnly(item: ChatMessage): boolean {
    if (item.role === 'user') {
      return false;
    }
    if (this.shouldRenderStructuredItems(item)) {
      return false;
    }
    if (!this.isFinishedAssistant(item)) {
      return false;
    }
    return this.finalMessageText(item).length > 0;
  }

  private isFinishedAssistant(item: ChatMessage): boolean {
    const status = (item.status || '').toLowerCase();
    return status === 'done' ||
      status === 'completed' ||
      status === 'success' ||
      status === 'sent' ||
      status.length === 0;
  }

  private finalMessageText(item: ChatMessage): string {
    const structuredText = this.lastTopLevelText(item.items || []);
    if (structuredText.length > 0) {
      return structuredText;
    }
    const visible = this.visibleMessageText(item);
    if (visible.length > 0 && visible !== (item.thinking || '').trim()) {
      return visible;
    }
    return '';
  }

  private lastTopLevelText(items: ChatMessageItemResponse[]): string {
    for (let index = items.length - 1; index >= 0; index--) {
      if (this.isMainTextEntry(items[index])) {
        const text = (items[index].content || '').trim();
        if (text.length > 0) {
          return text;
        }
      }
    }
    return '';
  }

  private hasVisibleProcessAttention(item: ChatMessage): boolean {
    return this.hasAttentionTools(item.tools || []) || this.hasAttentionItems(item.items || []);
  }

  private hasAttentionItems(items: ChatMessageItemResponse[]): boolean {
    return items.some((entry: ChatMessageItemResponse) => {
      if (entry.tool && this.isAttentionTool(entry.tool)) {
        return true;
      }
      return !!entry.subItems && this.hasAttentionItems(entry.subItems);
    });
  }

  private hasAttentionTools(tools: RemoteToolStatusResponse[]): boolean {
    return tools.some((tool: RemoteToolStatusResponse) => {
      return this.isAttentionTool(tool);
    });
  }

  private isAttentionTool(tool: RemoteToolStatusResponse): boolean {
    return this.hasToolError(tool) ||
      this.isPendingTool(tool) ||
      this.isRunningTool(tool) ||
      this.isQuestionTool(tool);
  }

  private structuredGroups(
    items: ChatMessageItemResponse[],
    prefix: string,
    activeScope: boolean = this.isStreamingAssistant()
  ): StructuredRenderGroup[] {
    const groups: StructuredRenderGroup[] = [];
    let toolBuffer: RemoteToolStatusResponse[] = [];
    let toolStart = 0;
    let activityItems: ChatMessageItemResponse[] = [];
    let activityStatuses: string[] = [];
    let activityStreaming: boolean[] = [];
    let activityChildScopes: boolean[] = [];
    let activityStart = 0;
    let activityHasThinking = false;
    const activeItemIndex = activeScope ? this.lastRenderableItemIndex(items) : -1;

    const flushTools = () => {
      if (toolBuffer.length === 0) {
        return;
      }
      groups.push({
        type: 'tool_group',
        items: [],
        itemStatuses: [],
        itemStreaming: [],
        itemChildActiveScopes: [],
        tools: toolBuffer,
        key: `${prefix}-tools-${toolStart}-${toolBuffer.length}-${this.stableToolHash(toolBuffer)}`,
        path: `${prefix}-tools-${toolStart}`
      });
      toolBuffer = [];
    };

    const flushActivity = () => {
      if (activityItems.length === 0) {
        return;
      }
      if (!activityHasThinking) {
        activityItems.forEach((entry: ChatMessageItemResponse, offset: number) => {
          if (entry.tool) {
            if (toolBuffer.length === 0) {
              toolStart = activityStart + offset;
            }
            toolBuffer.push(entry.tool);
          }
        });
      } else if (activityItems.length === 1) {
        const entry = activityItems[0];
        groups.push({
          type: 'item',
          items: [entry],
          itemStatuses: [activityStatuses[0] || ''],
          itemStreaming: [activityStreaming[0] || false],
          itemChildActiveScopes: [activityChildScopes[0] || false],
          tools: [],
          key: this.structuredItemKey(entry, `${prefix}-${activityStart}`),
          path: `${prefix}-${activityStart}`
        });
      } else {
        groups.push({
          type: 'activity_group',
          items: activityItems,
          itemStatuses: activityStatuses,
          itemStreaming: activityStreaming,
          itemChildActiveScopes: activityChildScopes,
          tools: [],
          key: this.activityGroupKey(activityItems, `${prefix}-activity-${activityStart}`),
          path: `${prefix}-activity-${activityStart}`
        });
      }
      activityItems = [];
      activityStatuses = [];
      activityStreaming = [];
      activityChildScopes = [];
      activityHasThinking = false;
    };

    items.forEach((entry: ChatMessageItemResponse, index: number) => {
      if (this.shouldFoldIntoActivityGroup(entry)) {
        if (activityItems.length === 0) {
          activityStart = index;
        }
        activityItems.push(entry);
        activityStatuses.push(this.itemStatusForEntry(entry, index, activeItemIndex));
        activityStreaming.push(this.itemShouldStream(entry, index, activeItemIndex));
        activityChildScopes.push(this.itemChildActiveScope(entry, index, activeItemIndex));
        activityHasThinking = activityHasThinking || this.isThinkingEntry(entry);
        return;
      }

      flushActivity();
      if (entry.tool) {
        if (toolBuffer.length === 0) {
          toolStart = index;
        }
        toolBuffer.push(entry.tool);
        return;
      }

      flushTools();
      groups.push({
        type: 'item',
        items: [entry],
        itemStatuses: [this.itemStatusForEntry(entry, index, activeItemIndex)],
        itemStreaming: [this.itemShouldStream(entry, index, activeItemIndex)],
        itemChildActiveScopes: [this.itemChildActiveScope(entry, index, activeItemIndex)],
        tools: [],
        key: this.structuredItemKey(entry, `${prefix}-${index}`),
        path: `${prefix}-${index}`
      });
    });

    flushActivity();
    flushTools();
    return groups;
  }

  private lastRenderableItemIndex(items: ChatMessageItemResponse[]): number {
    for (let index = items.length - 1; index >= 0; index--) {
      const entry = items[index];
      if (this.isRenderableStructuredEntry(entry)) {
        return index;
      }
    }
    return -1;
  }

  private itemStatusForEntry(entry: ChatMessageItemResponse, index: number, activeItemIndex: number): string {
    if (this.isThinkingEntry(entry) && index === activeItemIndex && this.isStreamingAssistant()) {
      return this.item.status || 'active';
    }
    if (this.isThinkingEntry(entry)) {
      return 'done';
    }
    return '';
  }

  private itemShouldStream(entry: ChatMessageItemResponse, index: number, activeItemIndex: number): boolean {
    if (index !== activeItemIndex || !this.isStreamingAssistant()) {
      return false;
    }
    return this.isThinkingEntry(entry) || this.isTextEntry(entry) ||
      (this.isSubagentEntry(entry) && (!entry.subItems || entry.subItems.length === 0));
  }

  private itemChildActiveScope(entry: ChatMessageItemResponse, index: number, activeItemIndex: number): boolean {
    return index === activeItemIndex && this.isStreamingAssistant() &&
      !!entry.subItems && entry.subItems.length > 0;
  }

  private shouldFoldIntoActivityGroup(entry: ChatMessageItemResponse): boolean {
    if (this.isThinkingEntry(entry)) {
      return true;
    }
    if (!entry.tool) {
      return false;
    }
    return this.shouldFoldToolWithThinking(entry.tool);
  }

  private shouldFoldToolWithThinking(tool: RemoteToolStatusResponse): boolean {
    if (!this.isCollapsibleActivityTool(tool)) {
      return false;
    }
    if (this.hasToolError(tool) || this.isPendingTool(tool) || this.isRunningTool(tool) || this.isQuestionTool(tool)) {
      return false;
    }
    return this.isCompletedTool(tool) || this.isCancelledTool(tool);
  }

  private isCollapsibleActivityTool(tool: RemoteToolStatusResponse): boolean {
    const kind = this.activityToolKind(tool);
    return kind === 'read' || kind === 'search';
  }

  private activityToolKind(tool: RemoteToolStatusResponse): string {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    if (raw === 'Read' || raw === 'LS' ||
      normalized === 'read' ||
      normalized === 'read_file' ||
      normalized === 'ls' ||
      normalized === 'list_directory') {
      return 'read';
    }
    if (raw === 'Grep' ||
      raw === 'Glob' ||
      raw === 'WebSearch' ||
      raw === 'SemanticSearch' ||
      normalized === 'grep' ||
      normalized === 'glob' ||
      normalized === 'websearch' ||
      normalized === 'web_search' ||
      normalized === 'semanticsearch' ||
      normalized === 'semantic_search' ||
      normalized.indexOf('search') >= 0) {
      return 'search';
    }
    return '';
  }

  private activityGroupTitle(group: StructuredRenderGroup): string {
    const stats = this.activityGroupStats(group.items);
    const total = stats.thinkingCount + stats.readCount + stats.searchCount + stats.otherCount;
    return `已折叠 ${total} 个思考和工具调用`;
  }

  private activityGroupDetail(group: StructuredRenderGroup): string {
    const stats = this.activityGroupStats(group.items);
    const parts: string[] = [];
    if (stats.thinkingCount > 0) {
      parts.push(`思考 ${stats.thinkingCount}`);
    }
    if (stats.readCount > 0) {
      parts.push(`读取 ${stats.readCount}`);
    }
    if (stats.searchCount > 0) {
      parts.push(`搜索 ${stats.searchCount}`);
    }
    if (stats.otherCount > 0) {
      parts.push(`其他 ${stats.otherCount}`);
    }
    return parts.join(' · ');
  }

  private activityGroupStats(items: ChatMessageItemResponse[]): ActivityGroupStats {
    const stats: ActivityGroupStats = {
      thinkingCount: 0,
      readCount: 0,
      searchCount: 0,
      otherCount: 0
    };
    items.forEach((entry: ChatMessageItemResponse) => {
      if (this.isThinkingEntry(entry)) {
        stats.thinkingCount += 1;
        return;
      }
      if (entry.tool) {
        const kind = this.activityToolKind(entry.tool);
        if (kind === 'read') {
          stats.readCount += 1;
        } else if (kind === 'search') {
          stats.searchCount += 1;
        } else {
          stats.otherCount += 1;
        }
      }
    });
    return stats;
  }

  private activityGroupTools(group: StructuredRenderGroup): RemoteToolStatusResponse[] {
    const tools: RemoteToolStatusResponse[] = [];
    group.items.forEach((entry: ChatMessageItemResponse) => {
      if (entry.tool) {
        tools.push(entry.tool);
      }
    });
    return tools;
  }

  private isRenderableActivityThinking(
    entry: ChatMessageItemResponse,
    status: string,
    streaming: boolean
  ): boolean {
    if (!this.isThinkingEntry(entry)) {
      return false;
    }
    if (this.hasVisibleAssistantOutput(this.item)) {
      return false;
    }
    const normalized = (status || '').toLowerCase();
    return streaming || normalized === 'active' || normalized === 'running';
  }

  private stableToolHash(tools: RemoteToolStatusResponse[]): string {
    const parts: string[] = [];
    tools.forEach((tool: RemoteToolStatusResponse) => {
      parts.push(`${tool.id || ''}:${tool.name || ''}:${tool.status || ''}`);
    });
    return this.stableTextHash(parts.join('|'));
  }

  private toolsNotCoveredByItems(
    tools: RemoteToolStatusResponse[],
    items: ChatMessageItemResponse[]
  ): RemoteToolStatusResponse[] {
    const coveredTools = this.itemToolCoverageGroups(items);
    const matched: boolean[] = coveredTools.map((_keys: string[]) => false);
    return tools.filter((tool: RemoteToolStatusResponse) => {
      const matchIndex = this.matchCoveredToolIndex(this.toolCoverageKeys(tool), coveredTools, matched);
      if (matchIndex >= 0) {
        matched[matchIndex] = true;
        return false;
      }
      return true;
    });
  }

  private itemToolCoverageGroups(items: ChatMessageItemResponse[]): string[][] {
    const groups: string[][] = [];
    items.forEach((entry: ChatMessageItemResponse) => {
      if (entry.tool) {
        groups.push(this.toolCoverageKeys(entry.tool));
      }
      if (entry.subItems) {
        this.itemToolCoverageGroups(entry.subItems).forEach((keys: string[]) => {
          groups.push(keys);
        });
      }
    });
    return groups;
  }

  private matchCoveredToolIndex(keys: string[], groups: string[][], matched: boolean[]): number {
    for (let groupIndex = 0; groupIndex < groups.length; groupIndex++) {
      if (matched[groupIndex]) {
        continue;
      }
      const group = groups[groupIndex];
      for (let keyIndex = 0; keyIndex < keys.length; keyIndex++) {
        if (group.indexOf(keys[keyIndex]) >= 0) {
          return groupIndex;
        }
      }
    }
    return -1;
  }

  private toolCoverageKeys(tool: RemoteToolStatusResponse): string[] {
    const keys: string[] = [];
    const toolId = tool.id || '';
    if (toolId.length > 0) {
      keys.push(`id:${toolId}`);
    }
    keys.push(`full:${this.toolFingerprint(tool, true)}`);
    if (toolId.length === 0 || this.hasToolFingerprintData(tool)) {
      keys.push(`stable:${this.toolFingerprint(tool, false)}`);
    }
    return keys;
  }

  private toolFingerprint(tool: RemoteToolStatusResponse, includeStatus: boolean): string {
    const parts: string[] = [
      this.normalizedToolName(tool),
      includeStatus ? (tool.status || '') : '',
      tool.input_preview || '',
      tool.result_preview || '',
      tool.error_preview || '',
      tool.stdout || '',
      tool.stderr || '',
      `${tool.exit_code || 0}`,
      this.objectSignature(tool.tool_input),
      this.objectSignature(tool.tool_output)
    ];
    return this.stableTextHash(parts.join('|'));
  }

  private hasToolFingerprintData(tool: RemoteToolStatusResponse): boolean {
    return (tool.input_preview || '').length > 0 ||
      (tool.result_preview || '').length > 0 ||
      (tool.error_preview || '').length > 0 ||
      (tool.stdout || '').length > 0 ||
      (tool.stderr || '').length > 0 ||
      !!tool.tool_input ||
      !!tool.tool_output;
  }

  private objectSignature(value?: Object): string {
    if (!value) {
      return '';
    }
    try {
      return JSON.stringify(value);
    } catch (_err) {
      return 'object';
    }
  }

  private isRenderableStructuredEntry(entry: ChatMessageItemResponse): boolean {
    if (this.isThinkingEntry(entry) || this.isTextEntry(entry) || this.isSubagentEntry(entry) || !!entry.tool) {
      return true;
    }
    return !!entry.subItems && entry.subItems.some((child: ChatMessageItemResponse) => {
      return this.isRenderableStructuredEntry(child);
    });
  }

  private isThinkingEntry(entry: ChatMessageItemResponse): boolean {
    return (entry.type || '').toLowerCase() === 'thinking' && (entry.content || '').trim().length > 0;
  }

  private isTextEntry(entry: ChatMessageItemResponse): boolean {
    return this.isMainTextEntry(entry);
  }

  private isMainTextEntry(entry: ChatMessageItemResponse): boolean {
    const type = (entry.type || '').toLowerCase();
    if ((entry.content || '').trim().length === 0) {
      return false;
    }
    if (entry.tool || this.isThinkingEntry(entry) || this.isSubagentEntry(entry)) {
      return false;
    }
    return type === 'text' || type === 'message' || type.length === 0;
  }

  private isSubagentEntry(entry: ChatMessageItemResponse): boolean {
    const type = (entry.type || '').toLowerCase();
    return entry.is_subagent === true || type === 'subagent' || type === 'agent';
  }

  private subagentTitle(entry: ChatMessageItemResponse): string {
    const content = (entry.content || '').trim();
    if (content.length > 0 && content.length <= 80) {
      return content;
    }
    if (entry.tool && entry.tool.name) {
      return entry.tool.name;
    }
    return 'Subagent';
  }

  private structuredItemKey(entry: ChatMessageItemResponse, path: string): string {
    if (entry.tool && entry.tool.id) {
      return `${path}-tool-${entry.tool.id}`;
    }
    const content = entry.content || '';
    const childCount = entry.subItems ? entry.subItems.length : 0;
    return `${path}-${entry.type || 'item'}-${entry.is_subagent === true ? 'subagent' : 'main'}-${content.length}-${this.stableTextHash(content)}-${childCount}`;
  }

  private stableTextHash(text: string): string {
    let hash = 0;
    for (let index = 0; index < text.length; index++) {
      hash = ((hash << 5) - hash + text.charCodeAt(index)) | 0;
    }
    return Math.abs(hash).toString(36);
  }

  private activityGroupKey(items: ChatMessageItemResponse[], path: string): string {
    const parts: string[] = [];
    items.forEach((entry: ChatMessageItemResponse, index: number) => {
      const content = entry.content || '';
      const tool = entry.tool;
      parts.push([
        `${index}`,
        entry.type || '',
        `${content.length}:${this.stableTextHash(content)}`,
        tool ? `${tool.id || ''}:${tool.name || ''}:${tool.status || ''}` : ''
      ].join(':'));
    });
    return `${path}-${items.length}-${this.stableTextHash(parts.join('|'))}`;
  }

  private normalizedToolName(tool: RemoteToolStatusResponse): string {
    return (tool.name || 'Tool').replace(/[\s-]/g, '_').toLowerCase();
  }

  private isCompletedTool(tool: RemoteToolStatusResponse): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'completed' || normalized === 'done' || normalized === 'success';
  }

  private isCancelledTool(tool: RemoteToolStatusResponse): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'cancelled' || normalized === 'canceled';
  }

  private isRunningTool(tool: RemoteToolStatusResponse): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'running' || normalized === 'active';
  }

  private isPendingTool(tool: RemoteToolStatusResponse): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'pending' || normalized === 'queued' ||
      normalized === 'pending_confirmation' || normalized === 'needs_confirmation';
  }

  private hasToolError(tool: RemoteToolStatusResponse): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'failed' || normalized === 'error' || normalized === 'timeout' ||
      (tool.error_preview || '').length > 0;
  }

  private isQuestionTool(tool: RemoteToolStatusResponse): boolean {
    const normalized = this.normalizedToolName(tool);
    return (tool.name || '') === 'AskUserQuestion' || normalized === 'askuserquestion' ||
      normalized === 'ask_user_question';
  }

  private fileReferences(text: string): FileReference[] {
    const matches: FileReference[] = [];
    const seen = new Set<string>();
    MarkdownParser.parse(text).forEach((block: ParsedMarkdownBlock) => {
      this.collectInlineFileReferences(block.inlines, matches, seen);
      block.items.forEach((item: ParsedMarkdownListItem) => {
        this.collectInlineFileReferences(item.inlines, matches, seen);
      });
    });
    return matches.slice(0, 4);
  }

  private collectInlineFileReferences(
    inlines: ParsedMarkdownInline[],
    matches: FileReference[],
    seen: Set<string>
  ): void {
    inlines.forEach((inline: ParsedMarkdownInline) => {
      if (inline.type === 'link') {
        const linkTarget = this.downloadableLinkTarget(inline.url);
        if (linkTarget.length > 0) {
          this.addFileReference(linkTarget, matches, seen);
        }
        return;
      }
      if (inline.type !== 'code') {
        this.collectRawComputerLinks(inline.text, matches, seen);
      }
    });
  }

  private collectRawComputerLinks(text: string, matches: FileReference[], seen: Set<string>): void {
    const found = text.match(/computer:\/\/[^\s)\]}>"']+/g) || [];
    found.forEach((raw: string) => {
      this.addFileReference(this.cleanFileLink(raw), matches, seen);
    });
  }

  private addFileReference(path: string, matches: FileReference[], seen: Set<string>): void {
    const clean = this.cleanFileLink(path);
    if (clean.length === 0 || seen.has(clean)) {
      return;
    }
    seen.add(clean);
    matches.push({
      id: `file-${matches.length}-${clean}`,
      path: clean,
      label: this.fileLabel(clean)
    });
  }

  private downloadableLinkTarget(href: string): string {
    const value = this.cleanFileLink(href);
    if (value.indexOf('computer://') === 0) {
      return value;
    }
    return this.localDownloadablePath(value);
  }

  private localDownloadablePath(href: string): string {
    if (href.length === 0 || href === '/') {
      return '';
    }
    if (href.indexOf('://') >= 0 && href.indexOf('file://') !== 0) {
      return '';
    }
    if (href.indexOf('#') === 0 || href.indexOf('//') === 0) {
      return '';
    }

    const filePath = this.normalizeFileLikeHref(href);
    if (filePath.length === 0) {
      return '';
    }

    if (filePath.indexOf('/') === 0) {
      const segments = filePath.split('/').filter((segment: string) => segment.length > 0);
      if (segments.length < 2) {
        return '';
      }
    }

    const fileName = this.fileLabel(filePath);
    const dotIndex = fileName.lastIndexOf('.');
    if (dotIndex <= 0) {
      return '';
    }

    const extension = fileName.slice(dotIndex + 1).toLowerCase();
    if (extension.length === 0) {
      return '';
    }

    if (filePath.indexOf('/') === 0 || this.isWindowsAbsolutePath(filePath)) {
      return this.isCodeFileExtension(extension) ? '' : filePath;
    }
    return this.isDownloadableFileExtension(extension) ? filePath : '';
  }

  private normalizeFileLikeHref(rawHref: string): string {
    let filePath = rawHref.trim();
    if (filePath.indexOf('file://') === 0) {
      filePath = filePath.slice('file://'.length);
    } else if (filePath.indexOf('file:') === 0) {
      filePath = filePath.slice('file:'.length);
    }

    const workspacePlaceholder = '{{workspaceFolder}}';
    if (filePath.indexOf(workspacePlaceholder) === 0) {
      filePath = filePath.slice(workspacePlaceholder.length);
      if (filePath.indexOf('/') === 0) {
        filePath = filePath.slice(1);
      }
    }

    if (filePath.length >= 4 && filePath.charAt(0) === '/' && filePath.charAt(2) === ':' &&
      (filePath.charAt(3) === '/' || filePath.charAt(3) === '\\')) {
      filePath = filePath.slice(1);
    }

    try {
      return decodeURIComponent(filePath);
    } catch (_err) {
      return filePath;
    }
  }

  private cleanFileLink(value: string): string {
    let clean = value.trim();
    while (clean.length > 0) {
      const last = clean.charAt(clean.length - 1);
      if (last === ',' || last === '.' || last === ';' || last === ':' || last === ')' ||
        last === ']' || last === '}' || last === '>' || last === '，' || last === '。' ||
        last === '；' || last === '：') {
        clean = clean.slice(0, clean.length - 1);
      } else {
        break;
      }
    }
    return clean;
  }

  private isWindowsAbsolutePath(path: string): boolean {
    return path.length >= 3 && path.charAt(1) === ':' &&
      (path.charAt(2) === '/' || path.charAt(2) === '\\');
  }

  private isCodeFileExtension(extension: string): boolean {
    return '|js|jsx|ts|tsx|mjs|cjs|mts|cts|py|pyw|pyi|rs|go|java|kt|kts|scala|groovy|c|cpp|cc|cxx|h|hpp|hxx|hh|cs|rb|php|swift|vue|svelte|css|scss|less|sass|json|jsonc|yaml|yml|toml|xml|md|mdx|rst|txt|sh|bash|zsh|fish|ps1|bat|cmd|sql|graphql|gql|proto|lock|env|ini|cfg|conf|cj|ets|editorconfig|gitignore|log|'.indexOf(`|${extension}|`) >= 0;
  }

  private isDownloadableFileExtension(extension: string): boolean {
    return '|pdf|doc|docx|xls|xlsx|ppt|pptx|odt|ods|odp|rtf|pages|numbers|key|png|jpg|jpeg|gif|bmp|svg|webp|ico|tiff|tif|zip|tar|gz|bz2|7z|rar|dmg|iso|xz|mp3|wav|ogg|flac|aac|m4a|wma|mp4|avi|mkv|mov|webm|wmv|flv|csv|tsv|sqlite|db|parquet|epub|mobi|apk|ipa|exe|msi|deb|rpm|ttf|otf|woff|woff2|'.indexOf(`|${extension}|`) >= 0;
  }

  private fileLabel(path: string): string {
    const normalized = path.replace(/^computer:\/\//, '').replace(/^file:\/\//, '').replace(/\\/g, '/');
    const parts = normalized.split('/');
    return parts[parts.length - 1] || normalized || 'file';
  }

  private fileStatus(path: string): string {
    if ((this.downloadingFilePath === path || this.downloadedFilePath === path) && this.fileDownloadStatus.length > 0) {
      return this.fileDownloadStatus;
    }
    if (path.indexOf('computer://') === 0) {
      return RemoteI18n.t('chat.desktopFile');
    }
    if (path.indexOf('file://') === 0) {
      return RemoteI18n.t('chat.fileLink');
    }
    return path;
  }

  private fileButtonLabel(path: string): string {
    if (this.downloadingFilePath === path) {
      return RemoteI18n.t('chat.reading');
    }
    if (this.downloadedFilePath === path) {
      return RemoteI18n.t('common.done');
    }
    return RemoteI18n.t('chat.download');
  }
}
