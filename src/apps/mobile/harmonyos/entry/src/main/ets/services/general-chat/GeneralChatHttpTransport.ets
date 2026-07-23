import { http } from '@kit.NetworkKit';

export enum GeneralChatHttpMethod {
  Get = 'GET',
  Post = 'POST',
  Put = 'PUT',
  Delete = 'DELETE'
}

export interface GeneralChatHttpHeaders {
  authorization: string;
  contentType: string;
  accept: string;
}

export interface GeneralChatHttpResponse {
  statusCode: number;
  body: string;
}

export interface GeneralChatHttpTransport {
  request(
    url: string,
    method: GeneralChatHttpMethod,
    headers: GeneralChatHttpHeaders,
    body?: string
  ): Promise<GeneralChatHttpResponse>;
}

export interface GeneralChatTokenProvider {
  accessToken(): Promise<string>;
}

export class HarmonyGeneralChatHttpTransport implements GeneralChatHttpTransport {
  async request(
    url: string,
    method: GeneralChatHttpMethod,
    headers: GeneralChatHttpHeaders,
    body?: string
  ): Promise<GeneralChatHttpResponse> {
    const request = http.createHttp();
    try {
      const response = await request.request(url, {
        method: this.requestMethod(method),
        header: {
          'Authorization': headers.authorization,
          'Content-Type': headers.contentType,
          'Accept': headers.accept
        },
        extraData: body || '',
        expectDataType: http.HttpDataType.STRING,
        connectTimeout: 15000,
        readTimeout: 30000
      });
      return {
        statusCode: response.responseCode,
        body: typeof response.result === 'string' ? response.result : JSON.stringify(response.result)
      };
    } finally {
      request.destroy();
    }
  }

  private requestMethod(method: GeneralChatHttpMethod): http.RequestMethod {
    if (method === GeneralChatHttpMethod.Get) {
      return http.RequestMethod.GET;
    }
    if (method === GeneralChatHttpMethod.Put) {
      return http.RequestMethod.PUT;
    }
    if (method === GeneralChatHttpMethod.Delete) {
      return http.RequestMethod.DELETE;
    }
    return http.RequestMethod.POST;
  }
}
