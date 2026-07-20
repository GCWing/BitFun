import { ChatMessage, ChatMessageItemResponse, ImageAttachment, RemoteSession, RemoteToolStatusResponse } from '../model/RemoteModels';
import { Encoding } from './Encoding';

export interface GeneralChatSendResult {
  userMessage: ChatMessage;
  assistantMessage: ChatMessage;
  streamChunks: string[];
  tools: RemoteToolStatusResponse[];
}

export class GeneralChatBackend {
  private sessions: RemoteSession[] = [];
  private messagesBySession: Map<string, ChatMessage[]> = new Map<string, ChatMessage[]>();

  listSessions(): RemoteSession[] {
    return this.sessions.slice();
  }

  createSession(firstMessage: string): RemoteSession {
    const now = new Date().toISOString();
    const title = GeneralChatBackend.titleFromMessage(firstMessage);
    const session: RemoteSession = {
      id: Encoding.randomId('chat'),
      title,
      agentType: 'chat',
      status: 'ready',
      updatedAt: now,
      createdAt: now,
      messageCount: 0,
      workspacePath: '',
      workspaceName: 'BitFun'
    };
    this.sessions = [session].concat(this.sessions);
    this.messagesBySession.set(session.id, []);
    return session;
  }

  messages(sessionId: string): ChatMessage[] {
    return (this.messagesBySession.get(sessionId) || []).slice();
  }

  renameSession(sessionId: string, title: string): void {
    const nextTitle = title.trim();
    if (nextTitle.length === 0) {
      return;
    }
    this.sessions = this.sessions.map((session: RemoteSession) => {
      if (session.id !== sessionId) {
        return session;
      }
      return {
        id: session.id,
        title: nextTitle,
        agentType: session.agentType,
        status: session.status,
        updatedAt: new Date().toISOString(),
        createdAt: session.createdAt,
        messageCount: session.messageCount,
        workspacePath: session.workspacePath,
        workspaceName: session.workspaceName
      };
    });
  }

  sendMessage(sessionId: string, text: string, images: ImageAttachment[] = []): GeneralChatSendResult {
    const trimmed = text.trim();
    const now = new Date().toISOString();
    const userMessage: ChatMessage = {
      id: Encoding.randomId('msg'),
      role: 'user',
      text: trimmed,
      status: 'sent',
      timestamp: now,
      images
    };
    const tools = GeneralChatBackend.toolsFor(trimmed, images);
    const assistantMessage: ChatMessage = {
      id: Encoding.randomId('assistant'),
      turnId: Encoding.randomId('turn'),
      role: 'assistant',
      text: GeneralChatBackend.replyFor(trimmed, images),
      status: 'completed',
      timestamp: new Date().toISOString(),
      tools,
      items: GeneralChatBackend.itemsFor(tools, GeneralChatBackend.replyFor(trimmed, images))
    };
    const messages = (this.messagesBySession.get(sessionId) || []).concat([userMessage, assistantMessage]);
    this.messagesBySession.set(sessionId, messages);
    this.bumpSession(sessionId, trimmed, messages.length);
    return {
      userMessage,
      assistantMessage,
      streamChunks: GeneralChatBackend.streamChunks(assistantMessage.text),
      tools
    };
  }

  private bumpSession(sessionId: string, latestText: string, messageCount: number): void {
    const updatedAt = new Date().toISOString();
    this.sessions = this.sessions.map((session: RemoteSession) => {
      if (session.id !== sessionId) {
        return session;
      }
      return {
        id: session.id,
        title: session.title || GeneralChatBackend.titleFromMessage(latestText),
        agentType: session.agentType,
        status: 'ready',
        updatedAt,
        createdAt: session.createdAt,
        messageCount,
        workspacePath: session.workspacePath,
        workspaceName: session.workspaceName
      };
    });
    this.sessions.sort((first: RemoteSession, second: RemoteSession) => {
      return GeneralChatBackend.timeValue(second.updatedAt) - GeneralChatBackend.timeValue(first.updatedAt);
    });
  }

  private static titleFromMessage(text: string): string {
    const normalized = text.trim().replace(/\s+/g, ' ');
    if (normalized.length === 0) {
      return '新建聊天';
    }
    return normalized.length > 18 ? `${normalized.slice(0, 18)}...` : normalized;
  }

  private static replyFor(text: string, images: ImageAttachment[]): string {
    const normalized = text.trim();
    if (images.length > 0) {
      return [
        '我可以作为手机端助手处理图片。',
        '',
        '当前是 mock 流程：我会模拟图片理解、重点提取和后续追问；接入 BitFun harness 后，这里会真正把图片内容交给视觉模型分析。',
        '',
        '你可以继续问我：这张图里有什么、帮我提取文字、总结界面问题，或者让它和一个文件/网页一起分析。'
      ].join('\n');
    }
    if (GeneralChatBackend.looksLikeSearchTask(normalized)) {
      return [
        '我可以帮你做网络搜索和资料整理。',
        '',
        '当前是 mock 搜索流程：我会先判断搜索关键词，再模拟抓取多来源信息，最后给你摘要、要点和可继续追问的问题。接入 BitFun harness 后，这里会真正调用搜索工具并返回带来源的结果。',
        '',
        '建议的处理方式：',
        '1. 先搜索权威来源和最近资料。',
        '2. 合并重复信息，标出不确定点。',
        '3. 按你的目标输出摘要、对比表或行动建议。'
      ].join('\n');
    }
    if (GeneralChatBackend.looksLikeCodeTask(normalized)) {
      return [
        '这个问题可能需要 Code 能力来读取本地项目、运行命令或查看日志。',
        '',
        '如果只是要通用建议，我可以先给排查路径；如果要看本地仓库、改代码或运行命令，就应该从侧栏进入 Code。',
        '',
        '当前是 mock 回复。接入 BitFun harness 后，普通聊天会给出真实模型建议，Code 模块会继续负责本地 workspace 和工具执行。'
      ].join('\n');
    }
    if (normalized.indexOf('图片') >= 0 || normalized.toLowerCase().indexOf('image') >= 0) {
      return [
        '我可以帮你分析图片。',
        '',
        '手机端助手可以覆盖看图解释、OCR 提取、界面走查、截图问题定位和图片内容总结。当前是 mock 回复；接入 BitFun harness 后会把图片交给真实视觉模型处理。'
      ].join('\n');
    }
    if (normalized.indexOf('文件') >= 0 || normalized.toLowerCase().indexOf('file') >= 0) {
      return [
        '我可以帮你处理文件。',
        '',
        '手机端助手适合做文件总结、重点提取、改写、翻译、生成待办和跨文件对比。当前是 mock 回复；接入 BitFun harness 后会对真实附件内容进行处理。'
      ].join('\n');
    }
    if (GeneralChatBackend.looksLikeWritingTask(normalized)) {
      return [
        '我可以帮你写作、改写和整理表达。',
        '',
        '你可以让我把内容改成更正式、更短、更像产品说明，或者生成邮件、方案、会议纪要、社媒文案。当前是 mock 回复，后续会接入真实模型输出。'
      ].join('\n');
    }
    if (normalized.length === 0) {
      return '你可以直接问 BitFun。';
    }
    return [
      `我收到了：${normalized}`,
      '',
      '在手机端，BitFun 不应该只是普通聊天框，而是一个通用助手入口：可以问答、搜索资料、分析图片、处理文件、写作改写，也可以在需要本地项目能力时引导进入 Code。',
      '',
      '当前回复是 mock，用来验证移动端交互和流式 UI。下一步把这个 backend 替换为 BitFun harness 的普通聊天接口后，就会返回真实模型结果。'
    ].join('\n');
  }

  private static looksLikeSearchTask(text: string): boolean {
    const normalized = text.toLowerCase();
    return normalized.indexOf('搜索') >= 0 ||
      normalized.indexOf('查找') >= 0 ||
      normalized.indexOf('查一下') >= 0 ||
      normalized.indexOf('资料') >= 0 ||
      normalized.indexOf('新闻') >= 0 ||
      normalized.indexOf('最新') >= 0 ||
      normalized.indexOf('联网') >= 0 ||
      normalized.indexOf('search') >= 0 ||
      normalized.indexOf('web') >= 0;
  }

  private static looksLikeWritingTask(text: string): boolean {
    const normalized = text.toLowerCase();
    return normalized.indexOf('写') >= 0 ||
      normalized.indexOf('改写') >= 0 ||
      normalized.indexOf('润色') >= 0 ||
      normalized.indexOf('总结') >= 0 ||
      normalized.indexOf('翻译') >= 0 ||
      normalized.indexOf('邮件') >= 0 ||
      normalized.indexOf('文案') >= 0 ||
      normalized.indexOf('rewrite') >= 0 ||
      normalized.indexOf('translate') >= 0 ||
      normalized.indexOf('summarize') >= 0;
  }

  private static streamChunks(text: string): string[] {
    const chunks: string[] = [];
    let buffer = '';
    for (let index = 0; index < text.length; index++) {
      const char = text.charAt(index);
      buffer += char;
      const shouldBreak = char === '\n' ||
        char === '。' ||
        char === '；' ||
        char === '：' ||
        char === '，' ||
        char === '.' ||
        char === ',' ||
        buffer.length >= 14;
      if (shouldBreak) {
        chunks.push(buffer);
        buffer = '';
      }
    }
    if (buffer.length > 0) {
      chunks.push(buffer);
    }
    return chunks.length > 0 ? chunks : [text];
  }

  private static toolsFor(text: string, images: ImageAttachment[]): RemoteToolStatusResponse[] {
    const normalized = text.trim();
    if (images.length > 0) {
      return [
        GeneralChatBackend.tool(
        'analyze_image',
        '分析图片内容',
        '识别图片、提取文字和主要视觉元素',
        `已接收 ${images.length} 张图片，准备进行视觉理解。`
      )
      ];
    }
    if (GeneralChatBackend.looksLikeSearchTask(normalized)) {
      return [
        GeneralChatBackend.tool(
          'web_search',
          '搜索网络资料',
          normalized,
          '找到 5 条相关结果，已合并重复信息并保留待核验点。'
        ),
        GeneralChatBackend.tool(
          'summarize_sources',
          '整理搜索结果',
          '提取要点、冲突信息和下一步建议',
          '已整理摘要、关键结论和后续可追问方向。'
        )
      ];
    }
    if (normalized.indexOf('文件') >= 0 || normalized.toLowerCase().indexOf('file') >= 0) {
      return [
        GeneralChatBackend.tool(
          'parse_file',
          '读取文件内容',
          '等待附件或文件上下文',
          '当前没有真实附件，已模拟文件解析流程。'
        ),
        GeneralChatBackend.tool(
          'summarize_document',
          '总结文件',
          '生成摘要、重点和待办',
          '已准备文件总结输出结构。'
        )
      ];
    }
    if (GeneralChatBackend.looksLikeCodeTask(normalized)) {
      return [
        GeneralChatBackend.tool(
          'classify_code_request',
          '判断代码任务类型',
          normalized,
          '该请求可能需要本地工作区；普通聊天先给通用建议。'
        )
      ];
    }
    return [];
  }

  private static itemsFor(tools: RemoteToolStatusResponse[], text: string): ChatMessageItemResponse[] {
    const items: ChatMessageItemResponse[] = [];
    tools.forEach((tool: RemoteToolStatusResponse) => {
      items.push({
        type: 'tool',
        tool
      });
    });
    if (text.trim().length > 0) {
      items.push({
        type: 'text',
        content: text
      });
    }
    return items;
  }

  private static tool(
    name: string,
    _label: string,
    inputPreview: string,
    resultPreview: string
  ): RemoteToolStatusResponse {
    return {
      id: Encoding.randomId('tool'),
      name,
      status: 'completed',
      duration_ms: 420,
      input_preview: inputPreview,
      result_preview: resultPreview
    };
  }

  private static looksLikeCodeTask(text: string): boolean {
    const normalized = text.toLowerCase();
    return normalized.indexOf('项目') >= 0 ||
      normalized.indexOf('仓库') >= 0 ||
      normalized.indexOf('启动失败') >= 0 ||
      normalized.indexOf('运行命令') >= 0 ||
      normalized.indexOf('改代码') >= 0 ||
      normalized.indexOf('workspace') >= 0 ||
      normalized.indexOf('repo') >= 0 ||
      normalized.indexOf('git') >= 0 ||
      normalized.indexOf('npm') >= 0 ||
      normalized.indexOf('build') >= 0;
  }

  private static timeValue(value: string): number {
    const parsed = new Date(value).getTime();
    return Number.isNaN(parsed) ? 0 : parsed;
  }
}
