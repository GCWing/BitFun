import { ConversationUiImage, ConversationUiMessage, ConversationUiMessageItem, ConversationUiQuestionAnswer, ConversationUiToolStatus } from './ConversationUiModels';
import { INK, LINE, MUTED, SOFT } from './Theme';
import { MessageFileCards, MessageImageGallery, MessageMarkdown } from './ChatMessageContent';
import { ChatMessageRetryAction, ChatTypingDots, ChatUserMessageBubble } from './ChatMessageChrome';
import { ThinkingBlock } from './ThinkingBlock';
import { ToolStatusList } from './ToolStatusList';
import { MessageFileReference, MessageFileReferenceProjectionCache } from '../../services/MessageFileReferenceProjector';

interface SubagentTaskInput {
  description?: string;
  title?: string;
  prompt?: string;
}
interface StructuredRenderGroup {
  type: string;
  items: ConversationUiMessageItem[];
  itemStatuses: string[];
  itemStreaming: boolean[];
  itemChildActiveScopes: boolean[];
  tools: ConversationUiToolStatus[];
  key: string;
  path: string;
}

@ComponentV2
export struct ChatMessageBubble {
  @Param item: ConversationUiMessage = {
    id: '',
    role: 'assistant',
    text: '',
    status: '',
    detail: ''
  };
  @Param isStreaming: boolean = false;
  @Param isFinalizing: boolean = false;
  @Param showRetryAction: boolean = false;
  @Param isBusy: boolean = false;
  @Param downloadingFilePath: string = '';
  @Param downloadedFilePath: string = '';
  @Param fileDownloadStatus: string = '';
  @Param activeFilePreviewPath: string = '';
  @Param activeFilePreviewLoading: boolean = false;
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
  private readonly fileReferenceCache: MessageFileReferenceProjectionCache =
    new MessageFileReferenceProjectionCache();

  build() {
    if (this.item.role === 'user') {
      ChatUserMessageBubble({
        item: this.item,
        showRetryAction: this.showRetryAction,
        onRetryMessage: this.onRetryMessage
      })
    } else {
      this.AssistantBubble()
    }
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
        if (this.item.status === 'failed' && this.showRetryAction &&
          (this.item.detail || '').trim().length > 0) {
          ChatMessageRetryAction({
            assistant: true,
            retryText: this.item.detail || '',
            onRetry: this.onRetryMessage
          })
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
  FinalContent(item: ConversationUiMessage) {
    this.MessageText(this.finalMessageText(item), false, `${item.id}-final`)
    if (this.fileReferences(this.finalMessageText(item)).length > 0) {
      this.FileCards(this.finalMessageText(item))
    }
  }

  @Builder
  ProcessContent(item: ConversationUiMessage) {
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
        ChatTypingDots()
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
  StructuredItems(items: ConversationUiMessageItem[], omitActiveThinking: boolean = false) {
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
        ForEach(group.items, (entry: ConversationUiMessageItem, index: number) => {
          this.StructuredItem(
            entry,
            `${group.path}-adjacent-${index}`,
            group.itemStatuses[index] || '',
            group.itemStreaming[index] || false,
            group.itemChildActiveScopes[index] || false
          )
        }, (entry: ConversationUiMessageItem, index: number) => {
          return this.structuredItemKey(entry, `${group.path}-adjacent-${index}`);
        })
      }
    } else if (group.items.length > 0) {
      if (!omitActiveThinking || !this.isThinkingEntry(group.items[0])) {
        this.StructuredItem(
          group.items[0],
          group.path,
          group.itemStatuses[0] || '',
          group.itemStreaming[0] || false,
          group.itemChildActiveScopes[0] || false
        )
      }
    }
  }

  @Builder
  ActivityGroup(group: StructuredRenderGroup, omitActiveThinking: boolean = false) {
    Column({ space: 6 }) {
      if (this.activityGroupTools(group).length > 0) {
        this.Tools(this.activityGroupTools(group))
      }
      ForEach(group.items, (entry: ConversationUiMessageItem, index: number) => {
        if (!omitActiveThinking &&
          this.isRenderableActivityThinking(entry, group.itemStatuses[index] || '', group.itemStreaming[index] || false)) {
          this.Thinking(
            entry.content || '',
            group.itemStatuses[index] || this.item.status,
            `${this.item.id}-${group.path}-activity-${index}-thinking`,
            group.itemStreaming[index] || false
          )
        }
      }, (entry: ConversationUiMessageItem, index: number) => {
        return this.structuredItemKey(entry, `${group.path}-activity-thinking-${index}`);
      })
    }
    .width('100%')
  }

  @Builder
  StructuredItem(
    entry: ConversationUiMessageItem,
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
  SubagentGroup(entry: ConversationUiMessageItem, path: string, activeScope: boolean = false) {
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
      if (entry.tool && this.isRunningTool(entry.tool)) {
        ChatTypingDots()
      }
      if (entry.content && entry.content.trim().length > 0 && !this.isTextEntry(entry) && !this.isThinkingEntry(entry)) {
        this.MessageText(entry.content, activeScope && (!entry.subItems || entry.subItems.length === 0), `${this.item.id}-${path}-subagent`)
      }
      if (entry.subItems && entry.subItems.length > 0) {
        this.ChildItems(entry.subItems, path, activeScope)
      }
    }
    .width('100%')
    .padding({ left: 10, right: 10, top: 8, bottom: 8 })
    .backgroundColor(SOFT)
    .borderRadius(12)
    .border({ width: 1, color: LINE })
  }

  @Builder
  ChildItems(items: ConversationUiMessageItem[], parentPath: string, activeScope: boolean = false) {
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
  Tools(tools: ConversationUiToolStatus[]) {
    ToolStatusList({
      tools,
      isBusy: this.isBusy,
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
      onOpenFilePreview: (path: string, label: string) => {
        this.onOpenFilePreview(path, label);
      }
    })
  }

  @Builder
  MessageImages(images: ConversationUiImage[]) {
    MessageImageGallery({ images })
  }

  @Builder
  MessageText(text: string, active: boolean = false, streamKey: string = '') {
    MessageMarkdown({
      text,
      active,
      streamKey,
      onCopyText: this.onCopyMessage,
      onOpenLink: this.onOpenFilePreview
    })
  }

  @Builder
  FileCards(text: string) {
    MessageFileCards({
      text,
      downloadingFilePath: this.downloadingFilePath,
      downloadedFilePath: this.downloadedFilePath,
      fileDownloadStatus: this.fileDownloadStatus,
      activeFilePreviewPath: this.activeFilePreviewPath,
      activeFilePreviewLoading: this.activeFilePreviewLoading,
      onPreview: this.onOpenFilePreview,
      onDownload: this.onDownloadFile
    })
  }

  private fileReferences(text: string): MessageFileReference[] {
    return this.fileReferenceCache.referencesFor(text);
  }

  private visibleMessageText(item: ConversationUiMessage): string {
    const text = item.text.trim();
    if (text === '(空消息)' || text === '(empty message)') {
      return '';
    }
    return text;
  }

  private shouldShowTypingDots(item: ConversationUiMessage): boolean {
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

  private shouldRenderStructuredItems(item: ConversationUiMessage): boolean {
    if (item.role === 'user' || !item.items || item.items.length === 0) {
      return false;
    }
    return item.items.some((entry: ConversationUiMessageItem) => {
      return this.isRenderableStructuredEntry(entry);
    });
  }

  private uncoveredTools(item: ConversationUiMessage): ConversationUiToolStatus[] {
    return this.toolsNotCoveredByItems(item.tools || [], item.items || []);
  }

  private shouldPinThinkingToBottom(item: ConversationUiMessage): boolean {
    return this.isStreamingAssistant() &&
      this.currentThinkingText(item).length > 0;
  }

  private pinnedThinkingText(item: ConversationUiMessage): string {
    if (!this.shouldPinThinkingToBottom(item)) {
      return '';
    }
    return this.currentThinkingText(item);
  }

  private shouldOmitStructuredThinking(item: ConversationUiMessage): boolean {
    return this.shouldPinThinkingToBottom(item) ||
      (this.isStreamingAssistant() && this.hasVisibleAssistantOutput(item));
  }

  private hasVisibleAssistantOutput(item: ConversationUiMessage): boolean {
    if (this.lastTopLevelText(item.items || []).length > 0) {
      return true;
    }
    const visible = this.visibleMessageText(item);
    return visible.length > 0 && visible !== (item.thinking || '').trim();
  }

  private currentThinkingText(item: ConversationUiMessage): string {
    const structured = this.lastThinkingText(item.items || []);
    if (structured.length > 0) {
      return structured;
    }
    return (item.thinking || '').trim();
  }

  private lastThinkingText(items: ConversationUiMessageItem[]): string {
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

  private shouldRenderFinalOnly(item: ConversationUiMessage): boolean {
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

  private isFinishedAssistant(item: ConversationUiMessage): boolean {
    const status = (item.status || '').toLowerCase();
    return status === 'done' ||
      status === 'completed' ||
      status === 'success' ||
      status === 'sent' ||
      status.length === 0;
  }

  private finalMessageText(item: ConversationUiMessage): string {
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

  private lastTopLevelText(items: ConversationUiMessageItem[]): string {
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

  private hasVisibleProcessAttention(item: ConversationUiMessage): boolean {
    return this.hasAttentionTools(item.tools || []) || this.hasAttentionItems(item.items || []);
  }

  private hasAttentionItems(items: ConversationUiMessageItem[]): boolean {
    return items.some((entry: ConversationUiMessageItem) => {
      if (entry.tool && this.isAttentionTool(entry.tool)) {
        return true;
      }
      return !!entry.subItems && this.hasAttentionItems(entry.subItems);
    });
  }

  private hasAttentionTools(tools: ConversationUiToolStatus[]): boolean {
    return tools.some((tool: ConversationUiToolStatus) => {
      return this.isAttentionTool(tool);
    });
  }

  private isAttentionTool(tool: ConversationUiToolStatus): boolean {
    return this.hasToolError(tool) ||
      this.isPendingTool(tool) ||
      this.isRunningTool(tool) ||
      this.isQuestionTool(tool);
  }

  private structuredGroups(
    items: ConversationUiMessageItem[],
    prefix: string,
    activeScope: boolean = this.isStreamingAssistant()
  ): StructuredRenderGroup[] {
    const groups: StructuredRenderGroup[] = [];
    let toolBuffer: ConversationUiToolStatus[] = [];
    let toolStart = 0;
    let activityItems: ConversationUiMessageItem[] = [];
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
        activityItems.forEach((entry: ConversationUiMessageItem, offset: number) => {
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

    items.forEach((entry: ConversationUiMessageItem, index: number) => {
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

  private lastRenderableItemIndex(items: ConversationUiMessageItem[]): number {
    for (let index = items.length - 1; index >= 0; index--) {
      const entry = items[index];
      if (this.isRenderableStructuredEntry(entry)) {
        return index;
      }
    }
    return -1;
  }

  private itemStatusForEntry(entry: ConversationUiMessageItem, index: number, activeItemIndex: number): string {
    if (this.isThinkingEntry(entry) && index === activeItemIndex && this.isStreamingAssistant()) {
      return this.item.status || 'active';
    }
    if (this.isThinkingEntry(entry)) {
      return 'done';
    }
    return '';
  }

  private itemShouldStream(entry: ConversationUiMessageItem, index: number, activeItemIndex: number): boolean {
    if (index !== activeItemIndex || !this.isStreamingAssistant()) {
      return false;
    }
    return this.isThinkingEntry(entry) || this.isTextEntry(entry) ||
      (this.isSubagentEntry(entry) && (!entry.subItems || entry.subItems.length === 0));
  }

  private itemChildActiveScope(entry: ConversationUiMessageItem, index: number, activeItemIndex: number): boolean {
    return index === activeItemIndex && this.isStreamingAssistant() &&
      !!entry.subItems && entry.subItems.length > 0;
  }

  private shouldFoldIntoActivityGroup(entry: ConversationUiMessageItem): boolean {
    if (this.isThinkingEntry(entry)) {
      return true;
    }
    if (!entry.tool) {
      return false;
    }
    return this.shouldFoldToolWithThinking(entry.tool);
  }

  private shouldFoldToolWithThinking(tool: ConversationUiToolStatus): boolean {
    if (!this.isCollapsibleActivityTool(tool)) {
      return false;
    }
    if (this.hasToolError(tool) || this.isPendingTool(tool) || this.isRunningTool(tool) || this.isQuestionTool(tool)) {
      return false;
    }
    return this.isCompletedTool(tool) || this.isCancelledTool(tool);
  }

  private isCollapsibleActivityTool(tool: ConversationUiToolStatus): boolean {
    const kind = this.activityToolKind(tool);
    return kind === 'read' || kind === 'search';
  }

  private activityToolKind(tool: ConversationUiToolStatus): string {
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

  private activityGroupTools(group: StructuredRenderGroup): ConversationUiToolStatus[] {
    const tools: ConversationUiToolStatus[] = [];
    group.items.forEach((entry: ConversationUiMessageItem) => {
      if (entry.tool) {
        tools.push(entry.tool);
      }
    });
    return tools;
  }

  private isRenderableActivityThinking(
    entry: ConversationUiMessageItem,
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

  private stableToolHash(tools: ConversationUiToolStatus[]): string {
    const parts: string[] = [];
    tools.forEach((tool: ConversationUiToolStatus) => {
      parts.push(`${tool.id || ''}:${tool.name || ''}:${tool.status || ''}`);
    });
    return this.stableTextHash(parts.join('|'));
  }

  private toolsNotCoveredByItems(
    tools: ConversationUiToolStatus[],
    items: ConversationUiMessageItem[]
  ): ConversationUiToolStatus[] {
    const coveredTools = this.itemToolCoverageGroups(items);
    const matched: boolean[] = coveredTools.map((_keys: string[]) => false);
    return tools.filter((tool: ConversationUiToolStatus) => {
      const matchIndex = this.matchCoveredToolIndex(this.toolCoverageKeys(tool), coveredTools, matched);
      if (matchIndex >= 0) {
        matched[matchIndex] = true;
        return false;
      }
      return true;
    });
  }

  private itemToolCoverageGroups(items: ConversationUiMessageItem[]): string[][] {
    const groups: string[][] = [];
    items.forEach((entry: ConversationUiMessageItem) => {
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

  private toolCoverageKeys(tool: ConversationUiToolStatus): string[] {
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

  private toolFingerprint(tool: ConversationUiToolStatus, includeStatus: boolean): string {
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

  private hasToolFingerprintData(tool: ConversationUiToolStatus): boolean {
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

  private isRenderableStructuredEntry(entry: ConversationUiMessageItem): boolean {
    if (this.isThinkingEntry(entry) || this.isTextEntry(entry) || this.isSubagentEntry(entry) || !!entry.tool) {
      return true;
    }
    return !!entry.subItems && entry.subItems.some((child: ConversationUiMessageItem) => {
      return this.isRenderableStructuredEntry(child);
    });
  }

  private isThinkingEntry(entry: ConversationUiMessageItem): boolean {
    return (entry.type || '').toLowerCase() === 'thinking' && (entry.content || '').trim().length > 0;
  }

  private isTextEntry(entry: ConversationUiMessageItem): boolean {
    return this.isMainTextEntry(entry);
  }

  private isMainTextEntry(entry: ConversationUiMessageItem): boolean {
    const type = (entry.type || '').toLowerCase();
    if ((entry.content || '').trim().length === 0) {
      return false;
    }
    if (entry.tool || this.isThinkingEntry(entry) || this.isSubagentEntry(entry)) {
      return false;
    }
    return type === 'text' || type === 'message' || type.length === 0;
  }

  private isSubagentEntry(entry: ConversationUiMessageItem): boolean {
    const type = (entry.type || '').toLowerCase();
    return entry.is_subagent === true || type === 'subagent' || type === 'agent' ||
      (!!entry.tool && this.normalizedToolName(entry.tool) === 'task');
  }

  private subagentTitle(entry: ConversationUiMessageItem): string {
    const content = (entry.content || '').trim();
    if (content.length > 0 && content.length <= 80) {
      return content;
    }
    if (entry.tool && this.normalizedToolName(entry.tool) === 'task') {
      const title = this.subagentTaskTitle(entry.tool);
      if (title.length > 0) {
        return title;
      }
    }
    if (entry.tool && entry.tool.name) {
      return entry.tool.name;
    }
    return 'Subagent';
  }

  private subagentTaskTitle(tool: ConversationUiToolStatus): string {
    const raw = tool.input_preview || '';
    if (raw.length > 0) {
      try {
        const input = JSON.parse(raw) as SubagentTaskInput;
        const title = input.description || input.title || '';
        if (title.trim().length > 0) {
          return title.trim();
        }
      } catch (_err) {
      }
    }
    return '';
  }

  private structuredItemKey(entry: ConversationUiMessageItem, path: string): string {
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

  private activityGroupKey(items: ConversationUiMessageItem[], path: string): string {
    const parts: string[] = [];
    items.forEach((entry: ConversationUiMessageItem, index: number) => {
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

  private normalizedToolName(tool: ConversationUiToolStatus): string {
    return (tool.name || 'Tool').replace(/[\s-]/g, '_').toLowerCase();
  }

  private isCompletedTool(tool: ConversationUiToolStatus): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'completed' || normalized === 'done' || normalized === 'success';
  }

  private isCancelledTool(tool: ConversationUiToolStatus): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'cancelled' || normalized === 'canceled';
  }

  private isRunningTool(tool: ConversationUiToolStatus): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'running' || normalized === 'active';
  }

  private isPendingTool(tool: ConversationUiToolStatus): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'pending' || normalized === 'queued' ||
      normalized === 'pending_confirmation' || normalized === 'needs_confirmation';
  }

  private hasToolError(tool: ConversationUiToolStatus): boolean {
    const normalized = (tool.status || '').toLowerCase();
    return normalized === 'failed' || normalized === 'error' || normalized === 'timeout' ||
      (tool.error_preview || '').length > 0;
  }

  private isQuestionTool(tool: ConversationUiToolStatus): boolean {
    const normalized = this.normalizedToolName(tool);
    return (tool.name || '') === 'AskUserQuestion' || normalized === 'askuserquestion' ||
      normalized === 'ask_user_question';
  }

}
