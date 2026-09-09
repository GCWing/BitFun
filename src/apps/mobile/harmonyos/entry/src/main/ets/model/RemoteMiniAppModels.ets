
export const MINIAPP_H5_CAPABILITY: string = 'miniapp_h5_v1';

export interface RemoteMiniAppSummary {
  id: string;
  name: string;
  description: string;
}

export interface RemoteMiniAppRequest {
  action: string;
  app_id?: string;
  version?: number;
  method?: string;
  params?: Object;
}

export interface RemoteMiniAppPayload {
  apps?: RemoteMiniAppSummary[];
  id?: string;
  name?: string;
  version?: number;
  html?: string;
}

export interface RemoteMiniAppResponse {
  resp?: string;
  message?: string;
  value?: Object;
}

export interface MiniAppBridgeRequest {
  id: string;
  method: string;
  params?: MiniAppWorkerParams;
}

export interface MiniAppWorkerParams {
  method: string;
  params?: Object;
}

export interface MiniAppBridgeError { message: string; }
export interface MiniAppBridgeReply {
  jsonrpc: string;
  id: string;
  result?: Object | null;
  error?: MiniAppBridgeError;
}

export interface MiniAppActions {
  open: () => void;
  close: () => void;
  back: () => void;
  refresh: () => void;
  select: (id: string) => void;
  request: (raw: string) => Promise<string>;
}

export function emptyMiniAppActions(): MiniAppActions {
  return { open: () => {}, close: () => {}, back: () => {}, refresh: () => {}, select: () => {},
    request: async (_raw: string): Promise<string> => '' };
}

export interface RemoteMiniAppClient {
  miniAppConnection(): number;
  miniApp(request: RemoteMiniAppRequest, connection: number): Promise<Object>;
}
