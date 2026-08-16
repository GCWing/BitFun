import { ConversationUiQuestionAnswer, ConversationUiToolStatus } from '../state/ConversationUiModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { ToolFileReference, ToolFileReferenceResolver } from '../../services/ToolFileReferenceResolver';
import { ToolCollapseGroup, ToolCollapsePolicy } from '../policy/ToolCollapsePolicy';
import { CARD, FILE_LINK, GREEN, INK, LINE, MUTED, RED, SOFT } from './Theme';
import { ToolGlyph } from './ToolGlyphs';
import { ToolConfirmationPanel, ToolQuestionAnswerPanel } from './ToolInteractionPanels';

interface QuestionPreview {
  header?: string;
  question?: string;
}
interface QuestionPreviewPayload {
  question?: string;
  prompt?: string;
  questions?: QuestionPreview[];
}

interface TodoPreviewItem {
  content?: string;
}

interface ToolInputSummaryPayload {
  todos?: TodoPreviewItem[];
  content?: string;
  command?: string;
  cmd?: string;
  pattern?: string;
  regex?: string;
  query?: string;
  search_term?: string;
  needle?: string;
  path?: string;
  file_path?: string;
  filePath?: string;
  url?: string;
  prompt?: string;
  question?: string;
  description?: string;
  title?: string;
  task?: string;
}

interface ToolRenderEntry {
  type: string;
  key: string;
  tool?: ConversationUiToolStatus;
  tools: ConversationUiToolStatus[];
  readCount: number;
  searchCount: number;
  commandCount: number;
  otherCount: number;
}

@ComponentV2
export struct ToolStatusList {
  @Param tools: ConversationUiToolStatus[] = [];
  @Param isBusy: boolean = false;
  @Event onApproveTool: (toolId: string, updatedInput?: Object) => void =
    (_toolId: string, _updatedInput?: Object) => {};
  @Event onRejectTool: (toolId: string) => void = (_toolId: string) => {};
  @Event onCancelTool: (toolId: string) => void = (_toolId: string) => {};
  @Event onAnswerQuestion: (toolId: string, answers: ConversationUiQuestionAnswer) => void =
    (_toolId: string, _answers: ConversationUiQuestionAnswer) => {};
  @Event onOpenFilePreview: (path: string, label: string) => void =
    (_path: string, _label: string) => {};
  @Local expanded: boolean = false;
  @Local expandedToolKey: string = '';

  build() {
    Column({ space: 4 }) {
      ForEach(this.toolRenderEntries(), (entry: ToolRenderEntry, index: number) => {
        if (entry.type === 'summary') {
          this.CollapsedToolSummary(entry)
        } else if (entry.tool) {
          this.ToolRow(entry.tool, index)
        }
      }, (entry: ToolRenderEntry) => entry.key)
    }
    .width('100%')
  }

  @Builder
  CollapsedToolSummary(entry: ToolRenderEntry) {
    Row({ space: 8 }) {
      this.SummaryTypeBadge(entry)
      Text(this.collapsedSummaryTitle(entry))
        .fontSize(13)
        .fontColor(MUTED)
        .layoutWeight(1)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
      this.ChevronIcon(this.expanded ? 'up' : 'down')
    }
    .width('100%')
    .height(28)
    .alignItems(VerticalAlign.Center)
    .padding({ left: 0, right: 0 })
    .onClick(() => {
      this.expanded = !this.expanded;
    })
  }

  @Builder
  ToolRow(tool: ConversationUiToolStatus, index: number) {
    Column({ space: 4 }) {
      Row({ space: 8 }) {
        this.ToolStatusIcon(tool)
        Text(this.toolLineLabel(tool))
          .fontSize(13)
          .fontColor(this.hasToolError(tool) ? RED :
            (this.toolFilePath(tool).length > 0 ? FILE_LINK : MUTED))
          .layoutWeight(1)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .onClick(() => {
            if (this.toolFilePath(tool).length > 0) {
              this.openToolFile(tool);
            } else {
              this.toggleToolExpanded(tool, index);
            }
          })
        this.TrailingChevron(tool, index)
      }
      .width('100%')
      .height(28)
      .alignItems(VerticalAlign.Center)

      if (this.canExpandTool(tool) && this.expandedToolKey === this.toolKey(tool, index)) {
        if (this.toolInputPreview(tool).length > 0) {
          this.ToolDetail(this.operationLabel(tool), this.toolInputPreview(tool), false)
        }
        if (this.toolResultPreview(tool).length > 0) {
          this.ToolDetail(this.hasToolError(tool) ? RemoteI18n.t('chat.error') : RemoteI18n.t('chat.output'), this.toolResultPreview(tool), this.hasToolError(tool))
        }
      }

      if (this.isPendingConfirmation(tool)) {
        ToolConfirmationPanel({
          toolId: tool.id || '',
          defaultInputText: this.defaultToolInputText(tool),
          hasEditableInput: this.hasEditableToolInput(tool),
          onApproveTool: this.onApproveTool,
          onRejectTool: this.onRejectTool
        })
      }
      if (this.isQuestionTool(tool)) {
        ToolQuestionAnswerPanel({
          toolId: tool.id || '',
          questionsJson: this.questionInputJson(tool),
          isBusy: this.isBusy,
          onAnswerQuestion: this.onAnswerQuestion
        })
      }
      if (this.isRunningTool(tool) || this.isQuestionTool(tool)) {
        Row() {
          Blank()
          Text(RemoteI18n.t('chat.cancelTool'))
            .fontSize(12)
            .fontColor(INK)
            .textAlign(TextAlign.Center)
            .height(32)
            .padding({ left: 14, right: 14 })
            .backgroundColor(SOFT)
            .borderRadius(16)
            .border({ width: 1, color: LINE })
            .onClick(() => {
              this.onCancelTool(tool.id || '');
            })
        }
        .width('100%')
        .padding({ left: 30 })
      }
    }
    .width('100%')
    .padding({
      left: this.toolRowHorizontalPadding(tool, index),
      right: this.toolRowHorizontalPadding(tool, index),
      top: this.toolRowVerticalPadding(tool, index),
      bottom: this.toolRowVerticalPadding(tool, index)
    })
    .backgroundColor(this.toolRowBg(tool, index))
    .borderRadius(this.toolRowRadius(tool, index))
    .border({ width: this.toolRowBorderWidth(tool, index), color: LINE })
  }

  @Builder
  ToolStatusIcon(tool: ConversationUiToolStatus) {
    Stack({ alignContent: Alignment.BottomEnd }) {
      this.ToolTypeBadge(tool)
      if (this.shouldShowStatusBadge(tool)) {
        this.ToolStatusBadge(tool)
      }
    }
    .width(20)
    .height(20)
  }

  @Builder
  TrailingChevron(tool: ConversationUiToolStatus, index: number) {
    if (this.canExpandTool(tool)) {
      Stack({ alignContent: Alignment.Center }) {
        this.ChevronIcon(this.expandedToolKey === this.toolKey(tool, index) ? 'down' : 'right')
      }
      .width(32)
      .height(28)
      .onClick(() => {
        this.toggleToolExpanded(tool, index);
      })
    } else {
      Text('')
        .width(14)
        .height(14)
    }
  }

  @Builder
  SummaryTypeBadge(entry: ToolRenderEntry) {
    Stack({ alignContent: Alignment.Center }) {
      this.SummaryTypeSymbol(entry)
    }
    .width(18)
    .height(18)
    .backgroundColor(this.summaryTypeBg(entry))
    .borderRadius(5)
    .border({ width: 1, color: '#00000000' })
  }

  @Builder
  ToolTypeBadge(tool: ConversationUiToolStatus) {
    Stack({ alignContent: Alignment.Center }) {
      this.ToolTypeSymbol(tool)
    }
    .width(18)
    .height(18)
    .backgroundColor(this.toolTypeBg(tool))
    .borderRadius(5)
    .border({ width: 1, color: this.toolTypeBorderColor(tool) })
  }

  @Builder
  SummaryTypeSymbol(entry: ToolRenderEntry) {
    ToolGlyph({ kind: this.summaryGlyphKind(entry), color: this.summaryTypeColor(entry) })
  }

  @Builder
  ToolTypeSymbol(tool: ConversationUiToolStatus) {
    ToolGlyph({ kind: this.toolGlyphKind(tool), color: this.toolTypeColor(tool) })
  }

  @Builder
  ToolStatusBadge(tool: ConversationUiToolStatus) {
    Text(this.toolStatusIcon(tool))
      .width(10)
      .height(10)
      .fontSize(7)
      .fontWeight(FontWeight.Bold)
      .fontColor(CARD)
      .textAlign(TextAlign.Center)
      .backgroundColor(this.toolStatusColor(tool))
      .borderRadius(5)
      .border({ width: 1, color: CARD })
  }

  @Builder
  ChevronIcon(direction: string) {
    ToolGlyph({ kind: `chevron_${direction}`, color: MUTED })
  }

  @Builder
  ToolDetail(label: string, value: string, isError: boolean) {
    Column({ space: 4 }) {
      Text(label)
        .fontSize(11)
        .fontColor(MUTED)
      Text(value)
        .fontSize(12)
        .lineHeight(17)
        .fontColor(isError ? RED : MUTED)
        .width('100%')
        .maxLines(5)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .width('100%')
    .padding({ left: 30, top: 2 })
  }

  private displayStatus(status: string): string {
    const normalized = (status || '').toLowerCase();
    if (normalized === 'running' || normalized === 'active') {
      return RemoteI18n.t('common.running');
    }
    if (normalized === 'pending_confirmation' || normalized === 'needs_confirmation') {
      return RemoteI18n.t('chat.pendingConfirmation');
    }
    if (normalized === 'completed' || normalized === 'done' || normalized === 'sent') {
      return RemoteI18n.t('common.done');
    }
    if (normalized === 'failed' || normalized === 'error') {
      return RemoteI18n.t('common.failed');
    }
    if (normalized === 'cancelled' || normalized === 'canceled') {
      return RemoteI18n.t('chat.cancelled');
    }
    return status || RemoteI18n.t('common.waiting');
  }

  private isEmphasizedToolRow(tool: ConversationUiToolStatus, index: number): boolean {
    return this.expandedToolKey === this.toolKey(tool, index) ||
      this.hasToolError(tool) ||
      this.isPendingConfirmation(tool) ||
      this.isQuestionTool(tool) ||
      this.isRunningTool(tool);
  }

  private toolRowBg(tool: ConversationUiToolStatus, index: number): ResourceColor {
    if (this.isEmphasizedToolRow(tool, index)) {
      return SOFT;
    }
    return '#00000000';
  }

  private toolRowBorderWidth(tool: ConversationUiToolStatus, index: number): number {
    return this.isEmphasizedToolRow(tool, index) ? 1 : 0;
  }

  private toolRowRadius(tool: ConversationUiToolStatus, index: number): number {
    return this.isEmphasizedToolRow(tool, index) ? 14 : 8;
  }

  private toolRowVerticalPadding(tool: ConversationUiToolStatus, index: number): number {
    return this.isEmphasizedToolRow(tool, index) ? 6 : 0;
  }

  private toolRowHorizontalPadding(tool: ConversationUiToolStatus, index: number): number {
    return this.isEmphasizedToolRow(tool, index) ? 10 : 0;
  }

  private toolRenderEntries(): ToolRenderEntry[] {
    const groups = this.expanded ?
      ToolCollapsePolicy.expandedGroups(this.tools) :
      ToolCollapsePolicy.collapsedGroups(this.tools);
    const entries: ToolRenderEntry[] = [];
    groups.forEach((group: ToolCollapseGroup) => {
      entries.push(this.entryFromGroup(group));
    });
    return entries;
  }

  private entryFromGroup(group: ToolCollapseGroup): ToolRenderEntry {
    if (group.type === 'summary') {
      const stats = this.collapsedToolStats(group.tools);
      return {
        type: 'summary',
        key: `summary-${group.startIndex}-${group.tools.length}-${stats.readCount}-${stats.searchCount}-${stats.commandCount}-${stats.otherCount}`,
        tools: group.tools,
        readCount: stats.readCount,
        searchCount: stats.searchCount,
        commandCount: stats.commandCount,
        otherCount: stats.otherCount
      };
    }
    const tool = group.tools[0];
    return this.toolEntry(tool, group.startIndex);
  }

  private toolEntry(tool: ConversationUiToolStatus, index: number): ToolRenderEntry {
    return {
      type: 'tool',
      key: this.toolKey(tool, index),
      tool,
      tools: [],
      readCount: 0,
      searchCount: 0,
      commandCount: 0,
      otherCount: 0
    };
  }

  private collapsedToolStats(tools: ConversationUiToolStatus[]): ToolRenderEntry {
    let readCount = 0;
    let searchCount = 0;
    let commandCount = 0;
    let otherCount = 0;
    tools.forEach((tool: ConversationUiToolStatus) => {
      const kind = this.exploreToolKind(tool);
      if (kind === 'read') {
        readCount += 1;
      } else if (kind === 'search') {
        searchCount += 1;
      } else if (this.isCommandTool(tool)) {
        commandCount += 1;
      } else {
        otherCount += 1;
      }
    });
    return {
      type: 'summary',
      key: '',
      tools,
      readCount,
      searchCount,
      commandCount,
      otherCount
    };
  }

  private exploreToolKind(tool: ConversationUiToolStatus): string {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    if (raw === 'Read' || normalized === 'read' || normalized === 'read_file' ||
      raw === 'LS' || normalized === 'ls' || normalized === 'list_directory') {
      return 'read';
    }
    if (raw === 'Grep' || raw === 'Glob' || raw === 'WebSearch' ||
      normalized === 'grep' || normalized === 'glob' || normalized === 'websearch' ||
      normalized === 'web_search' || normalized === 'semanticsearch' || normalized === 'semantic_search') {
      return 'search';
    }
    return '';
  }

  private collapsedSummaryTitle(entry: ToolRenderEntry): string {
    return RemoteI18n.f('chat.toolsRan', `${entry.tools.length}`);
  }

  private collapsedSummaryDetail(entry: ToolRenderEntry): string {
    const parts: string[] = [];
    if (entry.readCount > 0) {
      parts.push(`读取 ${entry.readCount}`);
    }
    if (entry.searchCount > 0) {
      parts.push(`搜索 ${entry.searchCount}`);
    }
    if (entry.commandCount > 0) {
      parts.push(`命令 ${entry.commandCount}`);
    }
    if (entry.otherCount > 0) {
      parts.push(`其他 ${entry.otherCount}`);
    }
    return parts.join(' · ');
  }

  private normalizedToolName(tool: ConversationUiToolStatus): string {
    return (tool.name || 'Tool').replace(/[\s-]/g, '_').toLowerCase();
  }

  private summaryGlyphKind(entry: ToolRenderEntry): string {
    if (entry.searchCount > 0 && entry.readCount === 0) return 'search';
    if (entry.readCount > 0 && entry.searchCount === 0) return 'document';
    return 'stack';
  }

  private toolGlyphKind(tool: ConversationUiToolStatus): string {
    if (this.isQuestionLikeTool(tool)) return 'question';
    if (this.isTodoTool(tool)) return 'todo';
    if (this.isTaskTool(tool)) return 'task';
    if (this.isGitTool(tool)) return 'git';
    if (this.isDeleteTool(tool)) return 'delete';
    if (this.isDiffTool(tool)) return 'diff';
    if (this.isPatchTool(tool)) return 'patch';
    if (this.isFileCreateTool(tool)) return 'create';
    if (this.isFileMutationTool(tool)) return 'mutate';
    if (this.isFileReadTool(tool)) return this.isDirectoryListTool(tool) ? 'folder' : 'document';
    if (this.isSearchTool(tool)) return 'search';
    if (this.isWebTool(tool)) return 'web';
    if (this.isCommandTool(tool)) return 'command';
    return 'tool';
  }

  private isQuestionLikeTool(tool: ConversationUiToolStatus): boolean {
    const normalized = this.normalizedToolName(tool);
    return this.isQuestionTool(tool) || normalized === 'askuserquestion' || normalized === 'ask_user_question';
  }

  private isTodoTool(tool: ConversationUiToolStatus): boolean {
    return this.normalizedToolName(tool).indexOf('todo') >= 0;
  }

  private isTaskTool(tool: ConversationUiToolStatus): boolean {
    return this.normalizedToolName(tool) === 'task';
  }

  private isDirectoryListTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'LS' || normalized === 'ls' || normalized === 'list_directory';
  }

  private isFileReadTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'Read' || raw === 'LS' ||
      normalized === 'read' ||
      normalized === 'read_file' ||
      normalized === 'ls' ||
      normalized === 'list_directory';
  }

  private isFileMutationTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'Write' ||
      raw === 'Create' ||
      raw === 'Edit' ||
      raw === 'MultiEdit' ||
      raw === 'StrReplace' ||
      raw === 'ApplyPatch' ||
      raw === 'Patch' ||
      raw === 'Delete' ||
      raw === 'GetFileDiff' ||
      normalized === 'write' ||
      normalized === 'write_file' ||
      normalized === 'create_file' ||
      normalized === 'edit' ||
      normalized === 'edit_file' ||
      normalized === 'file_edit' ||
      normalized === 'multi_edit' ||
      normalized === 'multiedit' ||
      normalized === 'strreplace' ||
      normalized === 'str_replace' ||
      normalized === 'str_replace_editor' ||
      normalized === 'replace' ||
      normalized === 'replace_file' ||
      normalized === 'update_file' ||
      normalized === 'delete' ||
      normalized === 'delete_file' ||
      normalized === 'remove_file' ||
      normalized === 'get_file_diff' ||
      normalized === 'getfilediff' ||
      normalized === 'apply_patch' ||
      normalized === 'applypatch' ||
      normalized === 'patch';
  }

  private isCommandTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'Bash' ||
      raw === 'Shell' ||
      raw === 'Git' ||
      raw === 'ExecCommand' ||
      raw === 'WriteStdin' ||
      raw === 'ExecControl' ||
      normalized === 'bash' ||
      normalized === 'shell' ||
      normalized === 'git' ||
      normalized === 'exec_command' ||
      normalized === 'run_command' ||
      normalized === 'terminal' ||
      normalized === 'terminal_command' ||
      normalized === 'powershell' ||
      normalized === 'sh' ||
      normalized === 'write_stdin' ||
      normalized === 'writestdin' ||
      normalized === 'exec_control' ||
      normalized === 'execcontrol' ||
      normalized.indexOf('exec_command') >= 0;
  }

  private isGitTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'Git' || normalized === 'git' || normalized.indexOf('git_') === 0;
  }

  private isWebTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'WebFetch' ||
      normalized === 'webfetch' ||
      normalized === 'web_fetch';
  }

  private isDiffTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'GetFileDiff' || normalized === 'get_file_diff' || normalized === 'getfilediff';
  }

  private isDeleteTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'Delete' || normalized === 'delete' ||
      normalized === 'delete_file' || normalized === 'remove_file';
  }

  private isFileCreateTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'Write' || raw === 'Create' ||
      normalized === 'write' ||
      normalized === 'write_file' ||
      normalized === 'create' ||
      normalized === 'create_file';
  }

  private isPatchTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'ApplyPatch' ||
      raw === 'Patch' ||
      normalized === 'apply_patch' ||
      normalized === 'applypatch' ||
      normalized === 'patch';
  }

  private isSearchTool(tool: ConversationUiToolStatus): boolean {
    const raw = tool.name || '';
    const normalized = this.normalizedToolName(tool);
    return raw === 'Grep' ||
      raw === 'Glob' ||
      raw === 'SemanticSearch' ||
      normalized === 'grep' ||
      normalized === 'glob' ||
      normalized === 'semanticsearch' ||
      normalized === 'semantic_search' ||
      normalized.indexOf('search') >= 0;
  }

  private operationLabel(tool: ConversationUiToolStatus): string {
    const raw = tool.name || 'Tool';
    const normalized = this.normalizedToolName(tool);
    if (raw === 'TodoWrite' || normalized === 'todo_write' || normalized === 'todowrite') {
      return '更新任务列表';
    }
    if (raw === 'Task' || normalized === 'task') {
      return '发起任务';
    }
    if (this.isFileReadTool(tool)) {
      return '读取文件';
    }
    if (raw === 'Write' || raw === 'Create' || normalized === 'write' || normalized === 'write_file' ||
      normalized === 'create_file') {
      return '写入文件';
    }
    if (raw === 'Delete' || normalized === 'delete' || normalized === 'delete_file') {
      return '删除文件';
    }
    if (raw === 'GetFileDiff' || normalized === 'get_file_diff' || normalized === 'getfilediff') {
      return '查看变更';
    }
    if (this.isFileMutationTool(tool)) {
      return '修改文件';
    }
    if (this.isCommandTool(tool)) {
      return '运行命令';
    }
    if (raw === 'WebSearch' || normalized === 'websearch' || normalized === 'web_search') {
      return '网页搜索';
    }
    if (this.isWebTool(tool)) {
      return '访问网页';
    }
    if (this.isSearchTool(tool)) {
      return '搜索代码';
    }
    if (raw === 'AskUserQuestion') {
      return '请求确认';
    }
    return raw;
  }

  private summaryLabel(tool: ConversationUiToolStatus): string {
    const parsed = this.toolInputPayload(tool);
    const fromPayload = this.summaryFromPayload(tool, parsed);
    if (fromPayload.length > 0) {
      return this.compactText(fromPayload, 46);
    }

    const preview = this.toolInputPreview(tool);
    if (preview.length > 0 && !this.looksLikeJson(preview)) {
      return this.compactText(preview, 46);
    }
    return tool.name || 'Tool';
  }

  private toolInputPayload(tool: ConversationUiToolStatus): ToolInputSummaryPayload {
    if (tool.tool_input) {
      try {
        return JSON.parse(JSON.stringify(tool.tool_input)) as ToolInputSummaryPayload;
      } catch (_err) {
      }
    }
    const preview = tool.input_preview || '';
    if (preview.trim().length > 0) {
      try {
        return JSON.parse(preview) as ToolInputSummaryPayload;
      } catch (_err) {
      }
    }
    return {};
  }

  private summaryFromPayload(tool: ConversationUiToolStatus, payload: ToolInputSummaryPayload): string {
    const name = tool.name || '';
    if ((name === 'TodoWrite' || this.normalizedToolName(tool) === 'todowrite') &&
      payload.todos && payload.todos.length > 0) {
      return payload.todos[0].content || '任务列表';
    }

    if (name === 'Task') {
      return payload.description || payload.task || payload.prompt || payload.content || '子任务';
    }

    const path = payload.file_path || payload.filePath || payload.path || '';
    if (path.length > 0) {
      return this.lastPathSegment(path);
    }

    return payload.description ||
      payload.command ||
      payload.cmd ||
      payload.pattern ||
      payload.regex ||
      payload.query ||
      payload.search_term ||
      payload.needle ||
      payload.title ||
      payload.question ||
      payload.prompt ||
      payload.content ||
      payload.url ||
      '';
  }

  private lastPathSegment(path: string): string {
    const normalized = path.replace(/\\/g, '/');
    const parts = normalized.split('/');
    return parts[parts.length - 1] || normalized;
  }

  private looksLikeJson(value: string): boolean {
    const text = value.trim();
    return text.indexOf('{') === 0 || text.indexOf('[') === 0;
  }

  private compactText(value: string, limit: number): string {
    const text = value.replace(/\n/g, ' ').replace(/\s+/g, ' ').trim();
    if (text.length <= limit) {
      return text;
    }
    return `${text.slice(0, limit)}...`;
  }

  private toolLineLabel(tool: ConversationUiToolStatus): string {
    if (this.isRunningTool(tool)) {
      return `${RemoteI18n.t('chat.toolRunning')} “${this.compactText(this.operationTarget(tool), 24)}”`;
    }
    if (this.hasToolError(tool)) {
      return `${this.operationLabel(tool)}失败 · ${this.compactText(this.operationTarget(tool), 24)}`;
    }
    if (this.isPendingConfirmation(tool) || this.isQuestionTool(tool)) {
      return `${this.displayStatus(tool.status || 'pending')} · ${this.operationLabel(tool)}`;
    }
    if (this.isCompletedTool(tool) || this.isCancelledTool(tool)) {
      return `${this.operationLabel(tool)} · ${this.compactText(this.operationTarget(tool), 28)}`;
    }
    return `${this.displayStatus(tool.status || 'pending')} · ${this.operationLabel(tool)}`;
  }

  private operationTarget(tool: ConversationUiToolStatus): string {
    const summary = this.summaryLabel(tool);
    if (summary.length > 0 && summary !== tool.name) {
      return summary;
    }
    return this.operationLabel(tool);
  }

  private toolFileReference(tool: ConversationUiToolStatus): ToolFileReference | undefined {
    return ToolFileReferenceResolver.resolve(tool.name || '', tool.tool_input, tool.input_preview || '');
  }

  private toolFilePath(tool: ConversationUiToolStatus): string {
    return this.toolFileReference(tool)?.path || '';
  }

  private openToolFile(tool: ConversationUiToolStatus): void {
    const reference = this.toolFileReference(tool);
    if (reference) {
      this.onOpenFilePreview(reference.path, reference.label);
    }
  }

  private toggleToolExpanded(tool: ConversationUiToolStatus, index: number): void {
    if (!this.canExpandTool(tool)) {
      return;
    }
    const key = this.toolKey(tool, index);
    this.expandedToolKey = this.expandedToolKey === key ? '' : key;
  }

  private canExpandTool(tool: ConversationUiToolStatus): boolean {
    return this.hasToolError(tool) || this.isPendingConfirmation(tool) || this.isQuestionTool(tool) ||
      this.isRunningTool(tool) || this.isCompletedTool(tool) || this.isCancelledTool(tool);
  }

  private toolTypeColor(tool: ConversationUiToolStatus): ResourceColor {
    if (this.isDeleteTool(tool)) {
      return RED;
    }
    if (this.isTodoTool(tool) || this.isFileCreateTool(tool)) {
      return GREEN;
    }
    return MUTED;
  }

  private toolTypeBg(_tool: ConversationUiToolStatus): ResourceColor {
    return SOFT;
  }

  private toolTypeBorderColor(tool: ConversationUiToolStatus): ResourceColor {
    if (this.hasToolError(tool)) {
      return RED;
    }
    return LINE;
  }

  private toolStatusIcon(tool: ConversationUiToolStatus): string {
    if (this.hasToolError(tool)) {
      return '!';
    }
    const normalized = (tool.status || '').toLowerCase();
    if (normalized === 'running' || normalized === 'active') {
      return '•';
    }
    if (normalized === 'pending_confirmation' || normalized === 'needs_confirmation') {
      return '?';
    }
    if (normalized === 'cancelled' || normalized === 'canceled' || normalized === 'rejected') {
      return '×';
    }
    return '•';
  }

  private toolStatusColor(tool: ConversationUiToolStatus): ResourceColor {
    if (this.hasToolError(tool)) {
      return RED;
    }
    return MUTED;
  }

  private shouldShowStatusBadge(tool: ConversationUiToolStatus): boolean {
    return this.hasToolError(tool) ||
      this.isPendingConfirmation(tool) ||
      this.isQuestionTool(tool) ||
      this.isRunningTool(tool) ||
      this.isCancelledTool(tool);
  }

  private summaryTypeColor(_entry: ToolRenderEntry): ResourceColor {
    return MUTED;
  }

  private summaryTypeBg(_entry: ToolRenderEntry): ResourceColor {
    return SOFT;
  }

  private toolPreview(preview: string): string {
    const text = preview.replace(/\n/g, ' ').trim();
    return text.length <= 80 ? text : `${text.slice(0, 80)}...`;
  }

  private toolInputPreview(tool: ConversationUiToolStatus): string {
    if (tool.input_preview && tool.input_preview.length > 0) {
      return tool.input_preview;
    }
    if (tool.tool_input) {
      try {
        return JSON.stringify(tool.tool_input);
      } catch (_err) {
        return '';
      }
    }
    return '';
  }

  private toolResultPreview(tool: ConversationUiToolStatus): string {
    const parts: string[] = [];
    if (tool.result_preview && tool.result_preview.length > 0) {
      parts.push(tool.result_preview);
    }
    if (tool.error_preview && tool.error_preview.length > 0) {
      parts.push(tool.error_preview);
    }
    if (tool.stdout && tool.stdout.length > 0) {
      parts.push(tool.stdout);
    }
    if (tool.stderr && tool.stderr.length > 0) {
      parts.push(tool.stderr);
    }
    if (tool.tool_output) {
      try {
        parts.push(JSON.stringify(tool.tool_output));
      } catch (_err) {
      }
    }
    const text = parts.join('\n').trim();
    return text.length <= 480 ? text : `${text.slice(0, 480)}...`;
  }

  private hasToolError(tool: ConversationUiToolStatus): boolean {
    const status = (tool.status || '').toLowerCase();
    return status === 'failed' || status === 'error' || (tool.error_preview || '').length > 0 ||
      (tool.stderr || '').length > 0;
  }

  private isPendingConfirmation(tool: ConversationUiToolStatus): boolean {
    const status = (tool.status || '').toLowerCase();
    return (status === 'pending_confirmation' || status === 'needs_confirmation') && (tool.id || '').length > 0;
  }

  private hasEditableToolInput(tool: ConversationUiToolStatus): boolean {
    return !!tool.tool_input;
  }

  private defaultToolInputText(tool: ConversationUiToolStatus): string {
    if (!tool.tool_input) {
      return '';
    }
    try {
      return JSON.stringify(tool.tool_input, null, 2);
    } catch (_err) {
      return '';
    }
  }

  private isRunningTool(tool: ConversationUiToolStatus): boolean {
    const status = (tool.status || '').toLowerCase();
    return (status === 'running' || status === 'active') && (tool.id || '').length > 0;
  }

  private isCompletedTool(tool: ConversationUiToolStatus): boolean {
    const status = (tool.status || '').toLowerCase();
    return status === 'completed' || status === 'done' || status === 'sent';
  }

  private isCancelledTool(tool: ConversationUiToolStatus): boolean {
    const status = (tool.status || '').toLowerCase();
    return status === 'cancelled' || status === 'canceled' || status === 'rejected';
  }

  private isQuestionTool(tool: ConversationUiToolStatus): boolean {
    const status = (tool.status || '').toLowerCase();
    const name = this.normalizedToolName(tool);
    return (name === 'askuserquestion' || name === 'ask_user_question') &&
      status !== 'completed' &&
      status !== 'done' &&
      status !== 'failed' &&
      status !== 'error' &&
      status !== 'cancelled' &&
      status !== 'canceled' &&
      status !== 'rejected' &&
      (tool.id || '').length > 0;
  }

  private questionPrompt(tool: ConversationUiToolStatus): string {
    const preview = this.toolInputPreview(tool);
    const parsedQuestion = this.questionPromptFromJson(preview);
    if (parsedQuestion.length > 0) {
      return parsedQuestion;
    }
    if (preview.length === 0) {
      return RemoteI18n.t('chat.questionDefault');
    }
    return this.toolPreview(preview);
  }

  private questionInputJson(tool: ConversationUiToolStatus): string {
    if (tool.tool_input) {
      try {
        return JSON.stringify(tool.tool_input);
      } catch (_err) {
      }
    }
    return this.toolInputPreview(tool);
  }

  private questionPromptFromJson(preview: string): string {
    if (preview.trim().length === 0) {
      return '';
    }
    try {
      const payload = JSON.parse(preview) as QuestionPreviewPayload;
      if (payload.question && payload.question.trim().length > 0) {
        return payload.question.trim();
      }
      if (payload.prompt && payload.prompt.trim().length > 0) {
        return payload.prompt.trim();
      }
      if (payload.questions && payload.questions.length > 0) {
        const first = payload.questions[0];
        const question = first.question || first.header || '';
        return question.trim();
      }
    } catch (_err) {
    }
    return '';
  }

  private toolKey(tool: ConversationUiToolStatus, index: number): string {
    const signature = this.toolSignature(tool);
    if (tool.id && tool.id.length > 0) {
      return `${index}-${tool.id}-${signature}`;
    }
    return `${index}-${tool.name || 'tool'}-${signature}`;
  }

  private toolSignature(tool: ConversationUiToolStatus): string {
    const parts: string[] = [
      tool.status || 'pending',
      `${tool.duration_ms || 0}`,
      `${tool.exit_code || 0}`,
      `${(tool.input_preview || '').length}`,
      `${(tool.result_preview || '').length}`,
      `${(tool.error_preview || '').length}`,
      `${(tool.stdout || '').length}`,
      `${(tool.stderr || '').length}`,
      this.objectSignature(tool.tool_input),
      this.objectSignature(tool.tool_output)
    ];
    return this.stableTextHash(parts.join('|'));
  }

  private objectSignature(value?: Object): string {
    if (!value) {
      return '';
    }
    try {
      return JSON.stringify(value).length.toString();
    } catch (_err) {
      return 'object';
    }
  }

  private stableTextHash(text: string): string {
    let hash = 0;
    for (let index = 0; index < text.length; index++) {
      hash = ((hash << 5) - hash + text.charCodeAt(index)) | 0;
    }
    return Math.abs(hash).toString(36);
  }
}
