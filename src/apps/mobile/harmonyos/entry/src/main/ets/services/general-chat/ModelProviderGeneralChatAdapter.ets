import { http } from '@kit.NetworkKit';
import {
  ChatMessage,
  ChatMessageItemResponse,
  ImageAttachment,
  RemoteSession
} from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { Encoding } from '../Encoding';
import { GeneralChatConfigStore } from './GeneralChatConfigStore';
import { GeneralChatPort, GeneralChatSendResult, GeneralChatStreamObserver } from './GeneralChatPort';
import { ModelProviderSseParser } from './ModelProviderSseParser';
import { RemoteLogger } from '../RemoteLogger';

export const GENERAL_CHAT_SYSTEM_PROMPT: string = [
  'You are the general-purpose AI work assistant in the BitFun HarmonyOS mobile app.',
  'The BitFun product is a local AI workbench built around a Code Agent for long-horizon engineering and productivity tasks.',
  'The mobile app provides general chat plus entry points to BitFun Remote/Code; this chat itself does not have local workspace access.',
  'It is not a blockchain, gaming, NFT, cryptocurrency, or Web3 product. Never invent those product claims.',
  'Treat those product exclusions as internal factual constraints and do not mention them unless the user asks something directly related.',
  'Help with general questions, writing, summarization, planning, research preparation, and everyday work.',
  'This mobile chat currently has no local workspace, shell, desktop file, web browsing, or external tool access.',
  'Never claim to have used unavailable tools or accessed information you were not given.',
  'When a task requires reading or changing a local repository, running commands, or accessing desktop files, explain that the user should use BitFun Remote/Code.',
  'Answer in the user\'s language. Be concise, concrete, and honest about uncertainty.'
].join('\n');

export interface ModelProviderMessage {
  role: string;
  content: string;
}

interface AnthropicRequestBody {
  model: string;
  max_tokens: number;
  stream: boolean;
  system: string;
  messages: ModelProviderMessage[];
}

interface OpenAiRequestBody {
  model: string;
  max_tokens: number;
  stream: boolean;
  messages: ModelProviderMessage[];
}

interface AnthropicContentBlock {
  type?: string;
  text?: string;
}

interface AnthropicResponseBody {
  id?: string;
  content?: AnthropicContentBlock[];
}

interface OpenAiResponseMessage {
  content?: string;
}

interface OpenAiResponseChoice {
  message?: OpenAiResponseMessage;
}

interface OpenAiResponseBody {
  id?: string;
  choices?: OpenAiResponseChoice[];
}

interface ModelProviderHttpResponse {
  statusCode: number;
  body: string;
}

export interface ModelProviderHttpTransport {
  postStream(
    url: string,
    apiKey: string,
    body: string,
    protocol: ModelProviderProtocol,
    onData: (chunk: string) => void
  ): Promise<ModelProviderHttpResponse>;
  cancel(): void;
}

export enum ModelProviderProtocol {
  Anthropic = 'anthropic',
  OpenAi = 'openai'
}

export class HarmonyModelProviderHttpTransport implements ModelProviderHttpTransport {
  private activeRequest?: http.HttpRequest;

  async postStream(
    url: string,
    apiKey: string,
    body: string,
    protocol: ModelProviderProtocol,
    onData: (chunk: string) => void
  ): Promise<ModelProviderHttpResponse> {
    const request = http.createHttp();
    this.activeRequest = request;
    let responseBody = '';
    const dataCallback = (data: ArrayBuffer): void => {
      const chunk = Encoding.bytesToUtf8(new Uint8Array(data));
      responseBody += chunk;
      RemoteLogger.info(`general chat stream chunk bytes=${data.byteLength} total=${responseBody.length}`);
      onData(chunk);
    };
    request.on('dataReceive', dataCallback);
    try {
      const statusCode = await request.requestInStream(
        url,
        this.requestOptions(url, apiKey, body, protocol)
      );
      return {
        statusCode,
        body: responseBody
      };
    } finally {
      request.off('dataReceive', dataCallback);
      request.destroy();
      if (this.activeRequest === request) {
        this.activeRequest = undefined;
      }
    }
  }

  cancel(): void {
    const request = this.activeRequest;
    this.activeRequest = undefined;
    if (request) {
      request.destroy();
    }
  }

  private requestOptions(
    url: string,
    apiKey: string,
    body: string,
    protocol: ModelProviderProtocol
  ): http.HttpRequestOptions {
    if (protocol === ModelProviderProtocol.Anthropic && url.indexOf('openbitfun.com') >= 0) {
      return {
        method: http.RequestMethod.POST,
        header: {
          'Content-Type': 'application/json',
          'Accept': 'text/event-stream',
          'x-api-key': apiKey,
          'anthropic-version': '2023-06-01',
          'X-Verification-Code': 'from_bitfun'
        },
        extraData: body,
        connectTimeout: 15000,
        readTimeout: 60000
      };
    }
    if (protocol === ModelProviderProtocol.Anthropic) {
      return {
        method: http.RequestMethod.POST,
        header: {
          'Content-Type': 'application/json',
          'Accept': 'text/event-stream',
          'x-api-key': apiKey,
          'anthropic-version': '2023-06-01'
        },
        extraData: body,
        connectTimeout: 15000,
        readTimeout: 60000
      };
    }
    return {
      method: http.RequestMethod.POST,
      header: {
        'Content-Type': 'application/json',
        'Accept': 'text/event-stream',
        'Authorization': `Bearer ${apiKey}`
      },
      extraData: body,
      connectTimeout: 15000,
      readTimeout: 60000
    };
  }
}

export class ModelProviderGeneralChatAdapter implements GeneralChatPort {
  private readonly configStore: GeneralChatConfigStore;
  private readonly transport: ModelProviderHttpTransport;

  constructor(
    configStore: GeneralChatConfigStore,
    transport: ModelProviderHttpTransport = new HarmonyModelProviderHttpTransport()
  ) {
    this.configStore = configStore;
    this.transport = transport;
  }

  async createSession(firstMessage: string): Promise<RemoteSession> {
    const now = new Date().toISOString();
    return {
      id: Encoding.randomId('chat'),
      title: ModelProviderGeneralChatAdapter.titleFromMessage(firstMessage),
      agentType: 'chat',
      status: 'ready',
      updatedAt: now,
      createdAt: now,
      messageCount: 0,
      workspacePath: '',
      workspaceName: 'BitFun'
    };
  }

  async generateTitle(firstMessage: string, assistantMessage: string): Promise<string> {
    const config = await this.configStore.snapshot();
    const apiKey = (await this.configStore.accessToken()).trim();
    if (config.apiUrl.length === 0 || config.modelName.length === 0 || apiKey.length === 0) {
      return '';
    }
    const protocol = ModelProviderGeneralChatAdapter.resolveProtocol(config.apiUrl);
    const parser = new ModelProviderSseParser(protocol);
    let title = '';
    const prompt = [
      'Generate a concise title for this conversation.',
      'Use the user\'s language. Return only the title, with no quotes or punctuation.',
      'Limit it to at most 12 Chinese characters or 8 words.',
      `User: ${firstMessage}`,
      `Assistant: ${assistantMessage}`
    ].join('\n');
    const response = await this.transport.postStream(
      ModelProviderGeneralChatAdapter.resolveRequestUrl(config.apiUrl, protocol),
      apiKey,
      ModelProviderGeneralChatAdapter.buildTitleRequestBody(config.modelName, prompt, protocol),
      protocol,
      (chunk: string): void => {
        parser.feed(chunk).forEach((delta: string) => {
          title += delta;
        });
      }
    );
    ModelProviderGeneralChatAdapter.ensureSuccess(response.statusCode);
    parser.finish().forEach((delta: string) => {
      title += delta;
    });
    if (title.trim().length === 0) {
      title = ModelProviderGeneralChatAdapter.responseText(response.body, protocol);
    }
    return ModelProviderGeneralChatAdapter.cleanGeneratedTitle(title);
  }

  async sendMessage(
    _sessionId: string,
    text: string,
    images: ImageAttachment[] = [],
    history: ChatMessage[] = [],
    observer?: GeneralChatStreamObserver
  ): Promise<GeneralChatSendResult> {
    if (images.length > 0) {
      throw new Error(RemoteI18n.t('generalChat.imageNotSupported'));
    }
    const config = await this.configStore.snapshot();
    const apiKey = (await this.configStore.accessToken()).trim();
    if (config.apiUrl.length === 0 || config.modelName.length === 0 || apiKey.length === 0) {
      throw new Error(RemoteI18n.t('generalChat.modelNotConfigured'));
    }

    const userText = text.trim();
    const userMessage: ChatMessage = {
      id: Encoding.randomId('msg'),
      role: 'user',
      text: userText,
      status: 'pending',
      timestamp: new Date().toISOString(),
      images: []
    };
    if (observer) {
      await observer.onUserMessage(userMessage);
    }
    const messages = ModelProviderGeneralChatAdapter.toProviderMessages(history);
    messages.push({ role: 'user', content: userText });
    const protocol = ModelProviderGeneralChatAdapter.resolveProtocol(config.apiUrl);
    const parser = new ModelProviderSseParser(protocol);
    const streamChunks: string[] = [];
    let assistantText = '';
    const requestUrl = ModelProviderGeneralChatAdapter.resolveRequestUrl(config.apiUrl, protocol);
    RemoteLogger.info(`general chat request start protocol=${protocol} url=${requestUrl}`);
    const response = await this.transport.postStream(
      requestUrl,
      apiKey,
      ModelProviderGeneralChatAdapter.buildRequestBody(config.modelName, messages, protocol),
      protocol,
      (chunk: string): void => {
        const deltas = parser.feed(chunk);
        deltas.forEach((delta: string) => {
          assistantText += delta;
          streamChunks.push(delta);
          if (observer) {
            observer.onDelta(delta);
          }
        });
      }
    );
    RemoteLogger.info(`general chat response status=${response.statusCode} bodyLength=${response.body.length}`);
    ModelProviderGeneralChatAdapter.ensureSuccess(response.statusCode);
    parser.finish().forEach((delta: string) => {
      assistantText += delta;
      streamChunks.push(delta);
      if (observer) {
        observer.onDelta(delta);
      }
    });
    if (assistantText.trim().length === 0) {
      assistantText = ModelProviderGeneralChatAdapter.responseText(response.body, protocol);
      streamChunks.push(assistantText);
      if (observer) {
        observer.onDelta(assistantText);
      }
    }
    RemoteLogger.info(`general chat response parsed textLength=${assistantText.length} chunks=${streamChunks.length}`);
    const turnId = parser.responseId() || ModelProviderGeneralChatAdapter.responseId(response.body, protocol);
    const assistantMessage: ChatMessage = {
      id: Encoding.randomId('assistant'),
      turnId: turnId || Encoding.randomId('turn'),
      role: 'assistant',
      text: assistantText,
      status: 'completed',
      timestamp: new Date().toISOString(),
      tools: [],
      items: ModelProviderGeneralChatAdapter.textItems(assistantText)
    };
    return {
      userMessage,
      assistantMessage,
      streamChunks,
      tools: []
    };
  }

  cancelCurrentRequest(): void {
    this.transport.cancel();
  }

  static resolveProtocol(baseUrl: string): ModelProviderProtocol {
    const normalized = baseUrl.trim().toLowerCase().replace(/\/+$/, '');
    if (normalized.indexOf('openbitfun.com') >= 0 || normalized.endsWith('/v1/messages')) {
      return ModelProviderProtocol.Anthropic;
    }
    return ModelProviderProtocol.OpenAi;
  }

  static resolveRequestUrl(baseUrl: string, protocol: ModelProviderProtocol): string {
    const normalized = baseUrl.trim().replace(/\/+$/, '');
    if (protocol === ModelProviderProtocol.Anthropic) {
      return normalized.endsWith('/v1/messages') ? normalized : `${normalized}/v1/messages`;
    }
    if (normalized.endsWith('/chat/completions')) {
      return normalized;
    }
    return normalized.endsWith('/v1') ? `${normalized}/chat/completions` : `${normalized}/v1/chat/completions`;
  }

  static buildRequestBody(
    modelName: string,
    messages: ModelProviderMessage[],
    protocol: ModelProviderProtocol
  ): string {
    if (protocol === ModelProviderProtocol.Anthropic) {
      const request: AnthropicRequestBody = {
        model: modelName,
        max_tokens: 4096,
        stream: true,
        system: GENERAL_CHAT_SYSTEM_PROMPT,
        messages
      };
      return JSON.stringify(request);
    }
    const openAiMessages: ModelProviderMessage[] = [];
    openAiMessages.push({ role: 'system', content: GENERAL_CHAT_SYSTEM_PROMPT });
    messages.forEach((message: ModelProviderMessage) => {
      openAiMessages.push(message);
    });
    const request: OpenAiRequestBody = {
      model: modelName,
      max_tokens: 4096,
      stream: true,
      messages: openAiMessages
    };
    return JSON.stringify(request);
  }

  private static buildTitleRequestBody(
    modelName: string,
    prompt: string,
    protocol: ModelProviderProtocol
  ): string {
    if (protocol === ModelProviderProtocol.Anthropic) {
      const request: AnthropicRequestBody = {
        model: modelName,
        max_tokens: 40,
        stream: true,
        system: 'You create short conversation titles and output only the title.',
        messages: [{ role: 'user', content: prompt }]
      };
      return JSON.stringify(request);
    }
    const request: OpenAiRequestBody = {
      model: modelName,
      max_tokens: 40,
      stream: true,
      messages: [
        { role: 'system', content: 'You create short conversation titles and output only the title.' },
        { role: 'user', content: prompt }
      ]
    };
    return JSON.stringify(request);
  }

  private static cleanGeneratedTitle(value: string): string {
    const normalized = value.trim().replace(/^[“”"'《》]+|[“”"'《》。！？!?]+$/g, '').replace(/\s+/g, ' ');
    if (normalized.length === 0) {
      return '';
    }
    return normalized.length > 24 ? normalized.slice(0, 24) : normalized;
  }

  static responseText(body: string, protocol: ModelProviderProtocol): string {
    try {
      if (protocol === ModelProviderProtocol.OpenAi) {
        const response = Encoding.parseJsonObject<OpenAiResponseBody>(body);
        const choices = response.choices || [];
        const content = choices.length > 0 && choices[0].message ? choices[0].message?.content || '' : '';
        if (content.trim().length > 0) {
          return content.trim();
        }
        throw new Error(RemoteI18n.t('generalChat.emptyResponse'));
      }
      const response = Encoding.parseJsonObject<AnthropicResponseBody>(body);
      const blocks = response.content || [];
      const parts: string[] = [];
      blocks.forEach((block: AnthropicContentBlock) => {
        if (block.type === 'text' && block.text && block.text.trim().length > 0) {
          parts.push(block.text);
        }
      });
      const text = parts.join('\n').trim();
      if (text.length === 0) {
        throw new Error(RemoteI18n.t('generalChat.emptyResponse'));
      }
      return text;
    } catch (_err) {
      throw new Error(RemoteI18n.t('generalChat.invalidResponse'));
    }
  }

  private static responseId(body: string, protocol: ModelProviderProtocol): string {
    try {
      if (protocol === ModelProviderProtocol.OpenAi) {
        return Encoding.parseJsonObject<OpenAiResponseBody>(body).id || '';
      }
      return Encoding.parseJsonObject<AnthropicResponseBody>(body).id || '';
    } catch (_err) {
      return '';
    }
  }

  private static ensureSuccess(statusCode: number): void {
    if (statusCode >= 200 && statusCode < 300) {
      return;
    }
    if (statusCode === 401 || statusCode === 403) {
      throw new Error(RemoteI18n.t('generalChat.authenticationFailed'));
    }
    if (statusCode === 429) {
      throw new Error(RemoteI18n.t('generalChat.rateLimited'));
    }
    if (statusCode >= 500) {
      throw new Error(RemoteI18n.t('generalChat.serviceUnavailable'));
    }
    throw new Error(RemoteI18n.f('generalChat.requestFailed', `${statusCode}`));
  }

  private static toProviderMessages(history: ChatMessage[]): ModelProviderMessage[] {
    const messages: ModelProviderMessage[] = [];
    history.forEach((message: ChatMessage) => {
      const text = (message.text || '').trim();
      const status = (message.status || '').toLowerCase();
      if (status !== 'failed' &&
        (message.role === 'user' || message.role === 'assistant') && text.length > 0) {
        messages.push({ role: message.role, content: text });
      }
    });
    return messages;
  }

  private static titleFromMessage(text: string): string {
    const normalized = text.trim().replace(/\s+/g, ' ');
    if (normalized.length === 0) {
      return '新建聊天';
    }
    return normalized.length > 18 ? `${normalized.slice(0, 18)}...` : normalized;
  }

  private static textItems(text: string): ChatMessageItemResponse[] {
    return [{ type: 'text', content: text }];
  }
}
