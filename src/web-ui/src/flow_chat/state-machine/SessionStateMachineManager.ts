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
import {
  getActiveSurfaceId,
  type DeviceSurfaceId,
} from '@/infrastructure/peer-device/deviceSurface';

const log = createLogger('SessionStateMachineManager');

export class SessionStateMachineManager {
  private static instance: SessionStateMachineManager;
  private machinesBySurface = new Map<
    DeviceSurfaceId,
    Map<string, SessionStateMachineImpl>
  >();
  private globalListeners: Set<(sessionId: string, machine: SessionStateMachine) => void> = new Set();

  private constructor() {
  }

  static getInstance(): SessionStateMachineManager {
    if (!SessionStateMachineManager.instance) {
      SessionStateMachineManager.instance = new SessionStateMachineManager();
    }
    return SessionStateMachineManager.instance;
  }

  private surfaceMachines(
    surfaceId: DeviceSurfaceId = getActiveSurfaceId(),
    create = true,
  ): Map<string, SessionStateMachineImpl> | undefined {
    const existing = this.machinesBySurface.get(surfaceId);
    if (existing || !create) {
      return existing;
    }
    const machines = new Map<string, SessionStateMachineImpl>();
    this.machinesBySurface.set(surfaceId, machines);
    return machines;
  }

  getOrCreate(sessionId: string): SessionStateMachineImpl {
    const surfaceId = getActiveSurfaceId();
    const machines = this.surfaceMachines(surfaceId)!;
    let machine = machines.get(sessionId);

    if (!machine) {
      machine = new SessionStateMachineImpl(sessionId);
      machines.set(sessionId, machine);

      machine.subscribe((snapshot) => {
        // A peer left running in the background may still settle local state.
        // Its update must not wake consumers for whichever device is rendered.
        if (surfaceId === getActiveSurfaceId()) {
          this.notifyGlobalListeners(sessionId, snapshot);
        }
      });
    }

    return machine;
  }

  get(sessionId: string): SessionStateMachineImpl | null {
    return this.surfaceMachines(getActiveSurfaceId(), false)?.get(sessionId) || null;
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
    const machine = this.surfaceMachines(getActiveSurfaceId(), false)?.get(sessionId);
    return machine ? machine.getCurrentState() : SessionExecutionState.IDLE;
  }

  getSnapshot(sessionId: string): SessionStateMachine | null {
    const machine = this.surfaceMachines(getActiveSurfaceId(), false)?.get(sessionId);
    return machine ? machine.getSnapshot() : null;
  }

  delete(sessionId: string): void {
    this.surfaceMachines(getActiveSurfaceId(), false)?.delete(sessionId);
  }

  reset(sessionId: string): void {
    const machine = this.surfaceMachines(getActiveSurfaceId(), false)?.get(sessionId);
    if (machine) {
      machine.reset();
    }
  }

  /** Reset a submission stranded on a surface that is no longer rendered. */
  resetForSurface(surfaceId: DeviceSurfaceId, sessionId: string): void {
    this.surfaceMachines(surfaceId, false)?.get(sessionId)?.reset();
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
    return Array.from(this.surfaceMachines(getActiveSurfaceId(), false)?.keys() ?? []);
  }

  /** Permanently forget one detached device without touching other surfaces. */
  clearSurface(surfaceId: DeviceSurfaceId): void {
    this.machinesBySurface.delete(surfaceId);
  }

  clear(): void {
    this.machinesBySurface.clear();
  }
}

export const stateMachineManager = SessionStateMachineManager.getInstance();
