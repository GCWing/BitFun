 


export interface ApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: ApiError;
  message?: string;
  timestamp: Date;
}


export interface ApiError {
  code: string;
  message: string;
  details?: Record<string, any>;
  stack?: string;
}


export interface ApiRequestConfig {
  timeout?: number;
  retries?: number;
  retryDelay?: number;
  headers?: Record<string, string>;
  signal?: AbortSignal;
}


export interface TauriCommandConfig extends ApiRequestConfig {
  command: string;
  args?: Record<string, any>;
}


export interface HttpRequestConfig extends ApiRequestConfig {
  method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  url: string;
  params?: Record<string, any>;
  data?: any;
}


export interface IApiClient {
  
  invoke<T = any>(command: string, args?: Record<string, any>, config?: ApiRequestConfig): Promise<T>;
  
  
  request<T = any>(config: HttpRequestConfig): Promise<T>;
  
  
  cancelAll(): void;
  
  
  healthCheck(): Promise<boolean>;
}


export type ApiMiddleware = (
  request: ApiRequest,
  next: (request: ApiRequest) => Promise<ApiResponse>
) => Promise<ApiResponse>;


export interface ApiRequest {
  id: string;
  type: 'tauri' | 'http';
  config: TauriCommandConfig | HttpRequestConfig;
  timestamp: Date;
  retryCount: number;
}


export interface ApiStats {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  averageResponseTime: number;
  activeRequests: number;
}


export interface ApiConfig {
  baseUrl?: string;
  timeout: number;
  retries: number;
  retryDelay: number;
  enableLogging: boolean;
  middleware: ApiMiddleware[];
}

