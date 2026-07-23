/**
 * Session state machine manager
 * Manages state machine instances for all sessions
 */

import { SessionStateMachineImpl } from './SessionStateMachine';
import {
  SessionExecutionState,
  SessionExecutionEvent,
  SessionStateMachine,
} from './types';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SessionStateMachineManager');

export class SessionStateMachineManager {
  private static instance: SessionStateMachineManager;
  private machines = new Map<string, SessionStateMachineImpl>();
  private globalListeners: Set<(sessionId: string, machine: SessionStateMachine) => void> = new Set();

  private constructor() {
  }

  static getInstance(): SessionStateMachineManager {
    if (!SessionStateMachineManager.instance) {
      SessionStateMachineManager.instance = new SessionStateMachineManager();
    }
    return SessionStateMachineManager.instance;
  }

  getOrCreate(sessionId: string): SessionStateMachineImpl {
    let machine = this.machines.get(sessionId);
    
    if (!machine) {
      machine = new SessionStateMachineImpl(sessionId);
      this.machines.set(sessionId, machine);
      
      machine.subscribe((snapshot) => {
        this.notifyGlobalListeners(sessionId, snapshot);
      });
    }
    
    return machine;
  }

  get(sessionId: string): SessionStateMachineImpl | null {
    return this.machines.get(sessionId) || null;
  }

  async transition(
    sessionId: string,
    event: SessionExecutionEvent,
    payload?: any
  ): Promise<boolean> {
    const machine = this.getOrCreate(sessionId);
    return machine.transition(event, payload);
  }

  getCurrentState(sessionId: string): SessionExecutionState {
    const machine = this.machines.get(sessionId);
    return machine ? machine.getCurrentState() : SessionExecutionState.IDLE;
  }

  getSnapshot(sessionId: string): SessionStateMachine | null {
    const machine = this.machines.get(sessionId);
    return machine ? machine.getSnapshot() : null;
  }

  delete(sessionId: string): void {
    this.machines.delete(sessionId);
  }

  reset(sessionId: string): void {
    const machine = this.machines.get(sessionId);
    if (machine) {
      machine.reset();
    }
  }

  subscribeGlobal(
    listener: (sessionId: string, machine: SessionStateMachine) => void
  ): () => void {
    this.globalListeners.add(listener);
    return () => {
      this.globalListeners.delete(listener);
    };
  }

  private notifyGlobalListeners(sessionId: string, machine: SessionStateMachine) {
    this.globalListeners.forEach(listener => {
      try {
        listener(sessionId, machine);
      } catch (error) {
        log.error('Global listener error', { sessionId, error });
      }
    });
  }

  getAllSessionIds(): string[] {
    return Array.from(this.machines.keys());
  }

  clear(): void {
    this.machines.clear();
  }
}

export const stateMachineManager = SessionStateMachineManager.getInstance();

