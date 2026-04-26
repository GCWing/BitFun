 

import { ITransportAdapter } from './base';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('WebSocketAdapter');

interface PendingRequest {
  resolve: (value: any) => void;
  reject: (error: any) => void;
  timeout: NodeJS.Timeout;
}

export class WebSocketTransportAdapter implements ITransportAdapter {
  private ws: WebSocket | null = null;
  private url: string;
  private eventListeners: Map<string, Set<(data: any) => void>> = new Map();
  private pendingRequests: Map<string, PendingRequest> = new Map();
  private messageIdCounter = 0;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  
  constructor(url?: string) {
    
    this.url = url || import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws';
  }
  
   
  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        log.info('Connecting', { url: this.url });
        const ws = new WebSocket(this.url);
        this.ws = ws;
        let settled = false;

        ws.onopen = () => {
          log.info('Connected successfully');
          this.reconnectAttempts = 0;
          this.setupMessageHandler();
          if (!settled) {
            settled = true;
            resolve();
          }
        };
        
        ws.onerror = (error) => {
          log.error('Connection error', error);
          if (!settled) {
            settled = true;
            reject(new Error('WebSocket connection failed'));
          }
        };
        
        ws.onclose = () => {
          log.info('Connection closed');
          if (!settled) {
            settled = true;
            reject(new Error('WebSocket connection closed before open'));
          }
          this.handleDisconnect();
        };
      } catch (error) {
        log.error('Failed to create WebSocket', error);
        reject(error);
      }
    });
  }
  
   
  private connectPromise: Promise<void> | null = null;

  private async ensureConnected(): Promise<void> {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return;

    if (!this.connectPromise) {
      this.connectPromise = this.connect().finally(() => {
        this.connectPromise = null;
      });
    }

    return this.connectPromise;
  }

  private handleDisconnect(): void {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      const delay = this.reconnectDelay * this.reconnectAttempts;
      
      log.info('Reconnecting', { delay, attempt: this.reconnectAttempts, maxAttempts: this.maxReconnectAttempts });
      
      setTimeout(() => {
        this.connect().catch(error => {
          log.error('Reconnection failed', error);
        });
      }, delay);
    } else {
      log.error('Max reconnection attempts reached');
      
      this.pendingRequests.forEach((pending) => {
        clearTimeout(pending.timeout);
        pending.reject(new Error('WebSocket disconnected'));
      });
      this.pendingRequests.clear();
    }
  }
  
   
  private setupMessageHandler(): void {
    if (!this.ws) return;
    
    this.ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        
        
        if (message.id && this.pendingRequests.has(message.id)) {
          const pending = this.pendingRequests.get(message.id)!;
          clearTimeout(pending.timeout);
          
          if (message.error) {
            const errorMsg = typeof message.error === 'object' && message.error.message
              ? message.error.message
              : String(message.error);
            pending.reject(new Error(errorMsg));
          } else {
            pending.resolve(message.result);
          }
          
          this.pendingRequests.delete(message.id);
          return;
        }
        
        
        if (message.event) {
          const listeners = this.eventListeners.get(message.event);
          if (listeners && listeners.size > 0) {
            listeners.forEach(callback => {
              try {
                callback(message.payload);
              } catch (error) {
                log.error('Error in event listener', { event: message.event, error });
              }
            });
          }
        }
      } catch (error) {
        log.error('Failed to parse message', { data: event.data, error });
      }
    };
  }
  
   
  async request<T>(action: string, params?: any): Promise<T> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      await this.ensureConnected();
    }

    const messageId = `msg_${Date.now()}_${++this.messageIdCounter}`;
    
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(messageId);
        reject(new Error(`Request timeout: ${action}`));
      }, 30000); 
      
      this.pendingRequests.set(messageId, { resolve, reject, timeout });
      
      try {
        this.ws!.send(JSON.stringify({
          type: 'request',
          id: messageId,
          method: action,
          params: params || {}
        }));
      } catch (error) {
        clearTimeout(timeout);
        this.pendingRequests.delete(messageId);
        reject(error);
      }
    });
  }
  
   
  listen<T>(event: string, callback: (data: T) => void): () => void {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, new Set());
    }
    
    const listeners = this.eventListeners.get(event)!;
    listeners.add(callback);
    
    
    return () => {
      const listeners = this.eventListeners.get(event);
      if (listeners) {
        listeners.delete(callback);
        if (listeners.size === 0) {
          this.eventListeners.delete(event);
        }
      }
    };
  }
  
   
  async disconnect(): Promise<void> {
    
    this.pendingRequests.forEach((pending) => {
      clearTimeout(pending.timeout);
      pending.reject(new Error('WebSocket manually disconnected'));
    });
    this.pendingRequests.clear();
    
    
    this.eventListeners.clear();
    
    
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    
    
    this.reconnectAttempts = this.maxReconnectAttempts;
    this.connectPromise = null;
  }
  
   
  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }
}


