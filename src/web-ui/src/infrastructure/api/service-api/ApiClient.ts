 

import { getTransportAdapter, ITransportAdapter } from '../adapters';
import {
  IApiClient,
  ApiResponse,
  ApiError,
  ApiRequest,
  ApiRequestConfig,
  TauriCommandConfig,
  HttpRequestConfig,
  ApiMiddleware,
  ApiStats,
  ApiConfig
} from './types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ApiClient');
const SENSITIVE_KEY_PATTERNS = [
  'api_key',
  'apikey',
  'token',
  'secret',
  'password',
  'authorization'
];

function isSensitiveKey(key: string): boolean {
  const normalized = key.toLowerCase();
  return SENSITIVE_KEY_PATTERNS.some(pattern => normalized.includes(pattern));
}

function maskSensitiveValue(value: unknown): string {
  if (typeof value !== 'string') {
    return '***';
  }
  if (value.length <= 8) {
    return '***';
  }
  return `${value.slice(0, 4)}***${value.slice(-4)}`;
}

function sanitizeForLog(value: unknown, parentKey?: string): unknown {
  if (value === null || value === undefined) {
    return value;
  }

  if (Array.isArray(value)) {
    return value.map(item => sanitizeForLog(item, parentKey));
  }

  if (typeof value !== 'object') {
    if (parentKey && isSensitiveKey(parentKey)) {
      return maskSensitiveValue(value);
    }
    return value;
  }

  const obj = value as Record<string, unknown>;
  const sanitized: Record<string, unknown> = {};

  for (const [key, rawVal] of Object.entries(obj)) {
    if (isSensitiveKey(key)) {
      sanitized[key] = maskSensitiveValue(rawVal);
      continue;
    }

    // For HTTP header maps, mask sensitive header values by header name.
    if ((key === 'headers' || key === 'custom_headers') && rawVal && typeof rawVal === 'object') {
      const headerObj = rawVal as Record<string, unknown>;
      const maskedHeaders: Record<string, unknown> = {};
      for (const [hKey, hVal] of Object.entries(headerObj)) {
        maskedHeaders[hKey] = isSensitiveKey(hKey) ? maskSensitiveValue(hVal) : hVal;
      }
      sanitized[key] = maskedHeaders;
      continue;
    }

    sanitized[key] = sanitizeForLog(rawVal, key);
  }

  return sanitized;
}

export class ApiClient implements IApiClient {
  private config: ApiConfig;
  private activeRequests = new Map<string, AbortController>();
  private stats: ApiStats = {
    totalRequests: 0,
    successfulRequests: 0,
    failedRequests: 0,
    averageResponseTime: 0,
    activeRequests: 0
  };
  private responseTimes: number[] = [];
  
  
  private adapter: ITransportAdapter;

  constructor(config: Partial<ApiConfig> = {}) {
    this.config = {
      timeout: 30000,
      retries: 0,
      retryDelay: 1000,
      enableLogging: process.env.NODE_ENV === 'development',
      middleware: [],
      ...config
    };
    
    
    this.adapter = getTransportAdapter();
  }

  async invoke<T = any>(
    command: string, 
    args?: any,
    config?: ApiRequestConfig
  ): Promise<T> {
    const requestConfig: TauriCommandConfig = {
      command,
      args: args,
      ...this.config,
      ...config
    };

    const request = this.createRequest('tauri', requestConfig);
    return this.executeRequest<T>(request);
  }

  async request<T = any>(config: HttpRequestConfig): Promise<T> {
    const requestConfig: HttpRequestConfig = {
      ...this.config,
      ...config
    };

    const request = this.createRequest('http', requestConfig);
    return this.executeRequest<T>(request);
  }

  cancelAll(): void {
    this.activeRequests.forEach(controller => {
      controller.abort();
    });
    this.activeRequests.clear();
  }

   
  listen<T = any>(event: string, callback: (data: T) => void): () => void {
    try {
      return this.adapter.listen<T>(event, callback);
    } catch (error) {
      log.error('Failed to listen to event', { event, error });
      
      return () => {};
    }
  }

  async healthCheck(): Promise<boolean> {
    try {
      
      if (!this.adapter.isConnected()) {
        await this.adapter.connect();
      }
      
      
      await this.invoke('ping', {}, { timeout: 5000, retries: 1 });
      return true;
    } catch (error) {
      return false;
    }
  }

  getStats(): ApiStats {
    return { ...this.stats };
  }
  
   
  getAdapter(): ITransportAdapter {
    return this.adapter;
  }

  private createRequest(type: 'tauri' | 'http', config: TauriCommandConfig | HttpRequestConfig): ApiRequest {
    return {
      id: `${type}-${Date.now()}-${Math.random()}`,
      type,
      config,
      timestamp: new Date(),
      retryCount: 0
    };
  }

  private async executeRequest<T>(request: ApiRequest): Promise<T> {
    const startTime = Date.now();
    
    this.updateStats({ totalRequests: this.stats.totalRequests + 1 });

    try {
      
      const controller = new AbortController();
      this.activeRequests.set(request.id, controller);

      
      const timeoutId = setTimeout(() => {
        controller.abort();
      }, request.config.timeout || this.config.timeout);

      try {
        
        const response = await this.applyMiddleware(request, async (req) => {
          if (req.type === 'tauri') {
            return this.executeTauriCommand(req.config as TauriCommandConfig);
          } else {
            return this.executeHttpRequest(req.config as HttpRequestConfig, controller.signal);
          }
        });

        clearTimeout(timeoutId);
        this.activeRequests.delete(request.id);

        
        const responseTime = Date.now() - startTime;
        this.recordResponseTime(responseTime);
        this.updateStats({ successfulRequests: this.stats.successfulRequests + 1 });


        if (this.config.enableLogging) {
          log.debug('Request completed', {
            type: request.type,
            responseTime,
            config: sanitizeForLog(request.config)
          });
        }

        return response.data;
      } finally {
        clearTimeout(timeoutId);
        this.activeRequests.delete(request.id);
      }
    } catch (error) {
      this.updateStats({ failedRequests: this.stats.failedRequests + 1 });

      
      if (request.retryCount < (request.config.retries || this.config.retries)) {
        const delay = (request.config.retryDelay || this.config.retryDelay) * Math.pow(2, request.retryCount);
        
        
        if (this.config.enableLogging) {
          log.warn('Retrying request', { 
            requestId: request.id, 
            attempt: request.retryCount + 1, 
            maxRetries: request.config.retries || this.config.retries,
            delay 
          });
        }

        await new Promise(resolve => setTimeout(resolve, delay));
        request.retryCount++;
        return this.executeRequest<T>(request);
      }


      if (this.config.enableLogging) {
        log.error('Request failed after retries', {
          requestId: request.id,
          retryCount: request.retryCount,
          config: sanitizeForLog(request.config),
          error
        });
      }

      throw this.normalizeError(error as Error);
    }
  }

  private async executeTauriCommand(config: TauriCommandConfig): Promise<ApiResponse> {
    try {
      
      
      const data = await this.adapter.request(config.command, config.args || {});
      
      return {
        success: true,
        data,
        timestamp: new Date()
      };
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      
      
      const isExpectedError = errorMessage.includes('not found') || 
                             errorMessage.includes('Config path') ||
                             errorMessage.includes('Configuration error');
      
      
      if (isExpectedError && this.config.enableLogging) {
        log.debug('Command returned expected result', {
          command: config.command,
          message: errorMessage
        });
      } else {
        log.error('Command failed', {
          command: config.command,
          args: sanitizeForLog(config.args),
          error: errorMessage,
          rawError: error
        });
      }
      
      throw this.createApiError('COMMAND_FAILED', errorMessage, error);
    }
  }


  private async executeHttpRequest(config: HttpRequestConfig, signal: AbortSignal): Promise<ApiResponse> {
    try {
      const url = new URL(config.url, this.config.baseUrl);
      
      
      if (config.params) {
        Object.entries(config.params).forEach(([key, value]) => {
          url.searchParams.append(key, String(value));
        });
      }

      const requestInit: RequestInit = {
        method: config.method,
        headers: {
          'Content-Type': 'application/json',
          ...config.headers
        },
        signal
      };

      if (config.data && config.method !== 'GET') {
        requestInit.body = JSON.stringify(config.data);
      }

      const response = await fetch(url.toString(), requestInit);
      
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }

      const data = await response.json();

      return {
        success: true,
        data,
        timestamp: new Date()
      };
    } catch (error) {
      if ((error as Error).name === 'AbortError') {
        throw this.createApiError('REQUEST_TIMEOUT', 'Request timeout', error);
      }
      throw this.createApiError('HTTP_REQUEST_FAILED', (error as Error).message, error);
    }
  }

  private async applyMiddleware(
    request: ApiRequest,
    executor: (request: ApiRequest) => Promise<ApiResponse>
  ): Promise<ApiResponse> {
    if (this.config.middleware.length === 0) {
      return executor(request);
    }

    let index = 0;
    const next = async (req: ApiRequest): Promise<ApiResponse> => {
      if (index >= this.config.middleware.length) {
        return executor(req);
      }

      const middleware = this.config.middleware[index++];
      return middleware(req, next);
    };

    return next(request);
  }

  private normalizeError(error: Error): ApiError {
    if (this.isApiError(error)) {
      return error as unknown as ApiError;
    }

    return this.createApiError('UNKNOWN_ERROR', error.message, error);
  }

  private createApiError(code: string, message: string, originalError?: any): ApiError {
    const apiError = new Error(message) as unknown as ApiError;
    apiError.code = code;
    apiError.message = message;
    
    if (originalError) {
      apiError.details = {
        originalError: originalError.message || originalError,
        stack: originalError.stack
      };
    }

    return apiError;
  }

  private isApiError(error: any): boolean {
    return error && typeof error.code === 'string';
  }

  private recordResponseTime(time: number): void {
    this.responseTimes.push(time);
    
    
    if (this.responseTimes.length > 100) {
      this.responseTimes = this.responseTimes.slice(-100);
    }

    
    const average = this.responseTimes.reduce((sum, t) => sum + t, 0) / this.responseTimes.length;
    this.updateStats({ averageResponseTime: Math.round(average) });
  }

  private updateStats(updates: Partial<ApiStats>): void {
    this.stats = { ...this.stats, ...updates };
    
  }
}


export const apiClient = new ApiClient();


export const api = {
  
  invoke: <T = any>(command: string, args?: any, config?: ApiRequestConfig): Promise<T> =>
    apiClient.invoke<T>(command, args, config),

  
  listen: <T = any>(event: string, callback: (data: T) => void): (() => void) =>
    apiClient.listen<T>(event, callback),

  
  get: <T = any>(url: string, config?: Partial<HttpRequestConfig>): Promise<T> =>
    apiClient.request<T>({ method: 'GET', url, ...config }),

  post: <T = any>(url: string, data?: any, config?: Partial<HttpRequestConfig>): Promise<T> =>
    apiClient.request<T>({ method: 'POST', url, data, ...config }),

  put: <T = any>(url: string, data?: any, config?: Partial<HttpRequestConfig>): Promise<T> =>
    apiClient.request<T>({ method: 'PUT', url, data, ...config }),

  delete: <T = any>(url: string, config?: Partial<HttpRequestConfig>): Promise<T> =>
    apiClient.request<T>({ method: 'DELETE', url, ...config }),

  patch: <T = any>(url: string, data?: any, config?: Partial<HttpRequestConfig>): Promise<T> =>
    apiClient.request<T>({ method: 'PATCH', url, data, ...config }),

  
  cancelAll: (): void => apiClient.cancelAll(),

  
  healthCheck: (): Promise<boolean> => apiClient.healthCheck(),

  
  getStats: (): ApiStats => apiClient.getStats(),
  
  
  getAdapter: (): ITransportAdapter => apiClient.getAdapter()
};


export function createLoggingMiddleware(): ApiMiddleware {
  const middlewareLog = createLogger('ApiMiddleware');
  return async (request: ApiRequest, next: (request: ApiRequest) => Promise<ApiResponse>) => {
    const startTime = Date.now();
    
    try {
      const response = await next(request);
      const duration = Date.now() - startTime;
      middlewareLog.debug('Request completed', {
        type: request.type,
        duration,
        config: sanitizeForLog(request.config)
      });
      return response;
    } catch (error) {
      const duration = Date.now() - startTime;
      middlewareLog.error('Request failed', { type: request.type, duration, error });
      throw error;
    }
  };
}

export function createRetryMiddleware(maxRetries: number = 3, baseDelay: number = 1000): ApiMiddleware {
  const middlewareLog = createLogger('ApiRetryMiddleware');
  return async (request: ApiRequest, next: (request: ApiRequest) => Promise<ApiResponse>) => {
    let lastError: Error;
    
    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      try {
        return await next(request);
      } catch (error) {
        lastError = error as Error;
        
        if (attempt < maxRetries) {
          const delay = baseDelay * Math.pow(2, attempt);
          middlewareLog.warn('Retrying request', { attempt: attempt + 1, maxRetries, delay });
          await new Promise(resolve => setTimeout(resolve, delay));
        }
      }
    }
    
    throw lastError!;
  };
}
