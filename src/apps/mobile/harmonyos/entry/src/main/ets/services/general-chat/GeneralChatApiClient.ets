import { Encoding } from '../Encoding';
import {
  GeneralChatCancelTurnRequest,
  GeneralChatCapabilityResponse,
  GeneralChatCreateSessionRequest,
  GeneralChatMessagePageResponse,
  GeneralChatRenameSessionRequest,
  GeneralChatSendTurnRequest,
  GeneralChatSessionPageResponse,
  GeneralChatSessionResponse,
  GeneralChatTurnReceiptResponse
} from './GeneralChatApiModels';
import {
  GeneralChatHttpHeaders,
  GeneralChatHttpMethod,
  GeneralChatHttpResponse,
  GeneralChatHttpTransport,
  GeneralChatTokenProvider
} from './GeneralChatHttpTransport';

export class GeneralChatApiClient {
  private readonly baseUrl: string;
  private readonly tokenProvider: GeneralChatTokenProvider;
  private readonly transport: GeneralChatHttpTransport;

  constructor(
    baseUrl: string,
    tokenProvider: GeneralChatTokenProvider,
    transport: GeneralChatHttpTransport
  ) {
    this.baseUrl = GeneralChatApiClient.normalizeBaseUrl(baseUrl);
    this.tokenProvider = tokenProvider;
    this.transport = transport;
  }

  async capabilities(): Promise<GeneralChatCapabilityResponse> {
    return this.request<GeneralChatCapabilityResponse>(GeneralChatHttpMethod.Get, '/api/mobile/v1/chat/capabilities');
  }

  async listSessions(cursor: string = ''): Promise<GeneralChatSessionPageResponse> {
    const query = cursor.trim().length > 0 ? `?cursor=${encodeURIComponent(cursor.trim())}` : '';
    return this.request<GeneralChatSessionPageResponse>(GeneralChatHttpMethod.Get, `/api/mobile/v1/chat/sessions${query}`);
  }

  async createSession(title: string): Promise<GeneralChatSessionResponse> {
    const body: GeneralChatCreateSessionRequest = { title: title.trim() };
    return this.request<GeneralChatSessionResponse>(GeneralChatHttpMethod.Post, '/api/mobile/v1/chat/sessions', body);
  }

  async renameSession(sessionId: string, title: string): Promise<GeneralChatSessionResponse> {
    const body: GeneralChatRenameSessionRequest = { title: title.trim() };
    return this.request<GeneralChatSessionResponse>(
      GeneralChatHttpMethod.Put,
      `/api/mobile/v1/chat/sessions/${encodeURIComponent(sessionId)}`,
      body
    );
  }

  async deleteSession(sessionId: string): Promise<void> {
    await this.request<void>(
      GeneralChatHttpMethod.Delete,
      `/api/mobile/v1/chat/sessions/${encodeURIComponent(sessionId)}`
    );
  }

  async loadMessages(sessionId: string, cursor: string = ''): Promise<GeneralChatMessagePageResponse> {
    const query = cursor.trim().length > 0 ? `?cursor=${encodeURIComponent(cursor.trim())}` : '';
    return this.request<GeneralChatMessagePageResponse>(
      GeneralChatHttpMethod.Get,
      `/api/mobile/v1/chat/sessions/${encodeURIComponent(sessionId)}/messages${query}`
    );
  }

  async sendTurn(sessionId: string, content: string, modelName: string): Promise<GeneralChatTurnReceiptResponse> {
    const body: GeneralChatSendTurnRequest = {
      content: content.trim(),
      model: modelName.trim()
    };
    if (body.model.length === 0) {
      throw new Error('General chat model name is not configured.');
    }
    return this.request<GeneralChatTurnReceiptResponse>(
      GeneralChatHttpMethod.Post,
      `/api/mobile/v1/chat/sessions/${encodeURIComponent(sessionId)}/turns`,
      body
    );
  }

  async cancelTurn(sessionId: string, turnId: string): Promise<void> {
    const body: GeneralChatCancelTurnRequest = { reason: 'user_requested' };
    await this.request<void>(
      GeneralChatHttpMethod.Post,
      `/api/mobile/v1/chat/sessions/${encodeURIComponent(sessionId)}/turns/${encodeURIComponent(turnId)}/cancel`,
      body
    );
  }

  private async request<T>(method: GeneralChatHttpMethod, path: string, body?: Object): Promise<T> {
    if (this.baseUrl.length === 0) {
      throw new Error('General chat API base URL is not configured.');
    }
    const accessToken = (await this.tokenProvider.accessToken()).trim();
    if (accessToken.length === 0) {
      throw new Error('General chat access token is not available.');
    }
    const headers: GeneralChatHttpHeaders = {
      authorization: `Bearer ${accessToken}`,
      contentType: 'application/json',
      accept: 'application/json'
    };
    const response = await this.transport.request(
      `${this.baseUrl}${path}`,
      method,
      headers,
      body ? JSON.stringify(body) : undefined
    );
    this.ensureSuccess(response);
    if (response.body.trim().length === 0) {
      return undefined as T;
    }
    try {
      return Encoding.parseJsonObject<T>(response.body);
    } catch (_err) {
      throw new Error('General chat API returned invalid JSON.');
    }
  }

  private ensureSuccess(response: GeneralChatHttpResponse): void {
    if (response.statusCode >= 200 && response.statusCode < 300) {
      return;
    }
    if (response.statusCode === 401 || response.statusCode === 403) {
      throw new Error('General chat authentication expired.');
    }
    if (response.statusCode === 429) {
      throw new Error('General chat request was rate limited.');
    }
    if (response.statusCode >= 500) {
      throw new Error('General chat service is temporarily unavailable.');
    }
    throw new Error(`General chat request failed with HTTP ${response.statusCode}.`);
  }

  private static normalizeBaseUrl(value: string): string {
    const trimmed = value.trim();
    return trimmed.endsWith('/') ? trimmed.slice(0, trimmed.length - 1) : trimmed;
  }
}
