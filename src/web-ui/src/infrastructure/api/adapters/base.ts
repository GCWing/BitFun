 

 
export interface ITransportAdapter {
   
  connect(): Promise<void>;
  
   
  request<T>(action: string, params?: any, timing?: TransportRequestTiming): Promise<T>;
  
   
  listen<T>(event: string, callback: (data: T) => void): () => void;

  /**
   * Resolves after listener registration requests already issued to the
   * transport have settled. Synchronous transports can keep the default no-op.
   */
  waitForListenerRegistrations?(): Promise<void>;
  
   
  disconnect(): Promise<void>;
  
   
  isConnected(): boolean;
}

export interface TransportRequestTiming {
  adapterInitDurationMs?: number;
  invokeDurationMs?: number;
  transportDurationMs?: number;
}

 
export interface StreamEvent {
  type: 'text-chunk' | 'tool-event' | 'stream-start' | 'stream-end' | string;
  sessionId: string;
  turnId: string;
  roundId?: string;
  payload: any;
}

 
export interface TextChunkEvent extends StreamEvent {
  type: 'text-chunk';
  payload: {
    content: string;
    accumulated: string;
  };
}

 
export interface ToolEvent extends StreamEvent {
  type: 'tool-event';
  payload: {
    toolName: string;
    status: 'start' | 'progress' | 'end' | 'error';
    data?: any;
  };
}


