 

import { ITransportAdapter } from './base';
import { TauriTransportAdapter } from './tauri-adapter';
import { WebSocketTransportAdapter } from './websocket-adapter';
import { isTauriRuntime } from '@/infrastructure/runtime';
export * from './base';
export * from './tauri-adapter';
export * from './websocket-adapter';

 
export function detectEnvironment(): 'tauri' | 'web' {
  
  if (isTauriRuntime()) {
    return 'tauri';
  }

  return 'web';
}

 
export function createTransportAdapter(forceEnv?: 'tauri' | 'web'): ITransportAdapter {
  const env = forceEnv || detectEnvironment();
  
  if (env === 'tauri') {
    return new TauriTransportAdapter();
  } else {
    const wsUrl = import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws';
    return new WebSocketTransportAdapter(wsUrl);
  }
}

 
let globalAdapter: ITransportAdapter | null = null;

 
export function getTransportAdapter(): ITransportAdapter {
  if (!globalAdapter) {
    globalAdapter = createTransportAdapter();
  }
  return globalAdapter;
}

 
export async function resetTransportAdapter(): Promise<void> {
  if (globalAdapter) {
    await globalAdapter.disconnect();
    globalAdapter = null;
  }
}

 
export function setTransportAdapter(adapter: ITransportAdapter): void {
  globalAdapter = adapter;
}

