import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SnapshotEventBus');

export interface SnapshotEvent {
  type: string;
  payload: any;
  timestamp: number;
  sessionId?: string;
  filePath?: string;
}

export interface SnapshotEventListener {
  (event: SnapshotEvent): void;
}

export class SnapshotEventBus {
  private static instance: SnapshotEventBus;
  private listeners: Map<string, Set<SnapshotEventListener>> = new Map();
  // Keep a small in-memory history for debugging/diagnostics.
  private eventHistory: SnapshotEvent[] = [];
  private maxHistorySize = 100;

  private constructor() {}

  public static getInstance(): SnapshotEventBus {
    if (!SnapshotEventBus.instance) {
      SnapshotEventBus.instance = new SnapshotEventBus();
    }
    return SnapshotEventBus.instance;
  }

  on(eventType: string, listener: SnapshotEventListener): () => void {
    if (!this.listeners.has(eventType)) {
      this.listeners.set(eventType, new Set());
    }
    
    const listeners = this.listeners.get(eventType)!;
    listeners.add(listener);

    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) {
        this.listeners.delete(eventType);
      }
    };
  }

  emit(eventType: string, payload: any, sessionId?: string, filePath?: string): void {
    const event: SnapshotEvent = {
      type: eventType,
      payload,
      timestamp: Date.now(),
      sessionId,
      filePath
    };

    this.eventHistory.push(event);
    if (this.eventHistory.length > this.maxHistorySize) {
      this.eventHistory.shift();
    }

    const listeners = this.listeners.get(eventType);
    if (listeners) {
      listeners.forEach(listener => {
        try {
          listener(event);
        } catch (error) {
          log.error('Event listener execution failed', { eventType, error });
        }
      });
    }
  }

  once(eventType: string, listener: SnapshotEventListener): void {
    const unsubscribe = this.on(eventType, (event) => {
      unsubscribe();
      listener(event);
    });
  }

  off(eventType?: string): void {
    if (eventType) {
      this.listeners.delete(eventType);
    } else {
      this.listeners.clear();
    }
  }

  getEventHistory(eventType?: string, sessionId?: string): SnapshotEvent[] {
    let events = this.eventHistory;

    if (eventType) {
      events = events.filter(e => e.type === eventType);
    }

    if (sessionId) {
      events = events.filter(e => e.sessionId === sessionId);
    }

    return events;
  }

  clearHistory(): void {
    this.eventHistory = [];
  }
}

export const SNAPSHOT_EVENTS = {
  // File operations
  FILE_MODIFIED: 'file_modified',
  FILE_OPERATION_COMPLETED: 'file_operation_completed',
  
  // State changes
  SESSION_STATE_CHANGED: 'session_state_changed',
  FILE_STATE_CHANGED: 'file_state_changed',
  BLOCK_STATE_CHANGED: 'block_state_changed',
  
  // User actions
  USER_ACCEPT_FILE: 'user_accept_file',
  USER_REJECT_FILE: 'user_reject_file',
  USER_ACCEPT_BLOCK: 'user_accept_block',
  USER_REJECT_BLOCK: 'user_reject_block',
  USER_ACCEPT_SESSION: 'user_accept_session',
  USER_REJECT_SESSION: 'user_reject_session',
  
  // Conflicts
  CONFLICT_DETECTED: 'conflict_detected',
  CONFLICT_RESOLVED: 'conflict_resolved',
  
  // System
  SNAPSHOT_INITIALIZED: 'snapshot_initialized',
  SESSION_CREATED: 'session_created',
  SESSION_COMPLETED: 'session_completed'
} as const;

export type SnapshotEventType = typeof SNAPSHOT_EVENTS[keyof typeof SNAPSHOT_EVENTS];
