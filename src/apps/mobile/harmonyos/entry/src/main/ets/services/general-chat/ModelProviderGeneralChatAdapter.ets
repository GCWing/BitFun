import { http } from '@kit.NetworkKit';
import {
  ChatMessage,
  ChatMessageItemResponse,
  ImageAttachment,
  RemoteSession
} from '../../model/RemoteModels';
import { RemoteI18n } from '../../i18n/RemoteI18n';
import { Encoding } from '../Encoding';
import { OhosError } from '../OhosError';
import { GeneralChatConfigStore } from './GeneralChatConfigStore';
import { GeneralChatPort, GeneralChatSendResult, GeneralChatStreamObserver } from './GeneralChatPort';
import { ModelProviderSseParser } from './ModelProviderSseParser';
import { RemoteLogger } from '../RemoteLogger';

export const GENERAL_CHAT_SYSTEM_PROMPT: string = [
  'You are the general-purpose AI work assistant in the OpenBitFun HarmonyOS mobile app.',
  'The OpenBitFun product is a local AI workbench built around a Code Agent for long-horizon engineering and productivity tasks.',
  'The mobile app provides general chat plus entry points to OpenBitFun Remote/Code; this chat itself does not have local workspace access.',
  'It is not a blockchain, gaming, NFT, cryptocurrency, or Web3 product. Never invent those product claims.',
  'Treat those product exclusions as internal factual constraints and do not mention them unless the user asks something directly related.',
  'Help with general questions, writing, summarization, planning, research preparation, and everyday work.',
  'The user may attach images. When images are present, look at them and answer from what you can see.',
  'This mobile chat currently has no local workspace, shell, desktop file, web browsing, or external tool access.',
  'Never claim to have used unavailable tools or accessed information you were not given.',
  'When a task requires reading or changing a local repository, running commands, or accessing desktop files, explain that the user should use OpenBitFun Remote/Code.',
  'Answer in the user\'s language. Be concise, concrete, and honest about uncertainty.'
].join('\n');

export interface ModelProviderImageSource {
  type: string;
  media_type?: string;
  data?: string;
}

export interface ModelProviderImageUrl {
  url: string;
}

export interface ModelProviderContentPart {
  type: string;
  text?: string;
  source?: ModelProviderImageSource;
  image_url?: ModelProviderImageUrl;
}

export interface ModelProviderMessage {
  role: string;
  content: string | ModelProviderContentPart[];
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
    } catch (err) {
      throw OhosError.wrap('general chat stream request', err);
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
          'X-Verification-Code': 'from_openbitfun'
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
      workspaceName: 'OpenBitFun'
    };
  }

  async generateTitle(firstMessage: string, assistantMessage: string): Promise<string> {
    const config = await this.configStore.activeSnapshot();
    const apiKey = (await this.configStore.activeAccessToken()).trim();
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
      ModelProviderGeneralChatAdapter.buildRequestBody(
        config.modelName,
        [{ role: 'user', content: prompt }],
        protocol,
        40,
        'You create short conversation titles and output only the title.'
      ),
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
    const config = await this.configStore.activeSnapshot();
    const apiKey = (await this.configStore.activeAccessToken()).trim();
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
      images: images.slice()
    };
    if (observer) {
      await observer.onUserMessage(userMessage);
    }
    const protocol = ModelProviderGeneralChatAdapter.resolveProtocol(config.apiUrl);
    const messages = ModelProviderGeneralChatAdapter.toProviderMessages(history, protocol);
    messages.push(ModelProviderGeneralChatAdapter.userProviderMessage(userText, images, protocol));
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

  static async probeConfiguration(
    baseUrl: string,
    apiKey: string,
    modelName: string,
    transport: ModelProviderHttpTransport = new HarmonyModelProviderHttpTransport()
  ): Promise<void> {
    const protocol = ModelProviderGeneralChatAdapter.resolveProtocol(baseUrl);
    const parser = new ModelProviderSseParser(protocol);
    let receivedText = '';
    const response = await transport.postStream(
      ModelProviderGeneralChatAdapter.resolveRequestUrl(baseUrl, protocol),
      apiKey.trim(),
      ModelProviderGeneralChatAdapter.buildRequestBody(
        modelName.trim(),
        [{ role: 'user', content: 'ping' }],
        protocol,
        1,
        'You verify whether a model endpoint is reachable. Reply with one short word.'
      ),
      protocol,
      (chunk: string): void => {
        parser.feed(chunk).forEach((delta: string) => {
          receivedText += delta;
        });
      }
    );
    ModelProviderGeneralChatAdapter.ensureSuccess(response.statusCode);
    parser.finish().forEach((delta: string) => {
      receivedText += delta;
    });
    if (receivedText.trim().length === 0) {
      throw new Error(RemoteI18n.t('generalChat.emptyResponse'));
    }
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
    protocol: ModelProviderProtocol,
    maxTokens: number = 4096,
    systemPrompt: string = GENERAL_CHAT_SYSTEM_PROMPT
  ): string {
    if (protocol === ModelProviderProtocol.Anthropic) {
      const request: AnthropicRequestBody = {
        model: modelName,
        max_tokens: maxTokens,
        stream: true,
        system: systemPrompt,
        messages
      };
      return JSON.stringify(request);
    }
    const openAiMessages: ModelProviderMessage[] = [];
    openAiMessages.push({ role: 'system', content: systemPrompt });
    messages.forEach((message: ModelProviderMessage) => {
      openAiMessages.push(message);
    });
    const request: OpenAiRequestBody = {
      model: modelName,
      max_tokens: maxTokens,
      stream: true,
      messages: openAiMessages
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

  private static toProviderMessages(
    history: ChatMessage[],
    protocol: ModelProviderProtocol
  ): ModelProviderMessage[] {
    const messages: ModelProviderMessage[] = [];
    history.forEach((message: ChatMessage) => {
      const status = (message.status || '').toLowerCase();
      if (status === 'failed' || (message.role !== 'user' && message.role !== 'assistant')) {
        return;
      }
      const text = (message.text || '').trim();
      if (message.role === 'user') {
        const providerMessage = ModelProviderGeneralChatAdapter.userProviderMessage(
          text,
          message.images || [],
          protocol
        );
        if (ModelProviderGeneralChatAdapter.hasProviderContent(providerMessage)) {
          messages.push(providerMessage);
        }
        return;
      }
      if (text.length > 0) {
        messages.push({ role: 'assistant', content: text });
      }
    });
    return messages;
  }

  private static userProviderMessage(
    text: string,
    images: ImageAttachment[],
    protocol: ModelProviderProtocol
  ): ModelProviderMessage {
    const parts: ModelProviderContentPart[] = [];
    if (protocol === ModelProviderProtocol.OpenAi && text.length > 0) {
      parts.push({ type: 'text', text });
    }
    images.forEach((image: ImageAttachment) => {
      const dataUrl = (image.data_url || '').trim();
      if (dataUrl.length === 0) {
        return;
      }
      const mediaType = ModelProviderGeneralChatAdapter.imageMediaType(dataUrl, image.mime_type);
      const rawBase64 = ModelProviderGeneralChatAdapter.imageBase64(dataUrl);
      if (rawBase64.length === 0) {
        return;
      }
      if (protocol === ModelProviderProtocol.Anthropic) {
        const source: ModelProviderImageSource = {
          type: 'base64',
          media_type: mediaType,
          data: rawBase64
        };
        parts.push({ type: 'image', source });
        return;
      }
      const imageUrl: ModelProviderImageUrl = {
        url: dataUrl.indexOf('data:') === 0 ? dataUrl : `data:${mediaType};base64,${rawBase64}`
      };
      parts.push({ type: 'image_url', image_url: imageUrl });
    });
    if (protocol === ModelProviderProtocol.Anthropic && text.length > 0) {
      parts.push({ type: 'text', text });
    }
    if (parts.length === 0) {
      return { role: 'user', content: text };
    }
    if (parts.length === 1 && parts[0].type === 'text') {
      return { role: 'user', content: text };
    }
    return { role: 'user', content: parts };
  }

  private static hasProviderContent(message: ModelProviderMessage): boolean {
    if (typeof message.content === 'string') {
      return message.content.trim().length > 0;
    }
    return message.content.length > 0;
  }

  private static imageMediaType(dataUrl: string, explicitMime?: string): string {
    const trimmed = (explicitMime || '').trim();
    if (trimmed.length > 0) {
      return trimmed;
    }
    if (dataUrl.indexOf('data:') === 0) {
      const semicolon = dataUrl.indexOf(';');
      if (semicolon > 5) {
        return dataUrl.slice(5, semicolon);
      }
    }
    return 'image/jpeg';
  }

  private static imageBase64(dataUrl: string): string {
    const marker = ';base64,';
    const index = dataUrl.indexOf(marker);
    if (index >= 0) {
      return dataUrl.slice(index + marker.length);
    }
    return dataUrl;
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
