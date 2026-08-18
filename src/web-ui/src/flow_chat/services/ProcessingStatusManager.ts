/**
 * Simplified processing status manager
 * Manages basic processing status display
 */

import { createLogger } from '@/shared/utils/logger';
import {
  getActiveSurfaceId,
  onSurfaceActivated,
  type DeviceSurfaceId,
} from '@/infrastructure/peer-device/deviceSurface';

const log = createLogger('ProcessingStatusManager');

export interface ProcessingStatus {
  id: string;
  sessionId: string;
  status: 'thinking' | 'analyzing' | 'processing' | 'executing' | 'generating' | 'completing';
  message: string;
  progress?: number;
  startTime: number;
  metadata?: Record<string, any>;
}

export interface ProcessingStatusListener {
  (statuses: ProcessingStatus[]): void;
}

interface SurfaceProcessingStatus extends ProcessingStatus {
  surfaceId: DeviceSurfaceId;
}

export class ProcessingStatusManager {
  private statuses: Map<string, SurfaceProcessingStatus> = new Map();
  private completedStatusesBySurface = new Map<DeviceSurfaceId, ProcessingStatus[]>();
  private listeners: Set<ProcessingStatusListener> = new Set();
  private cleanupIntervalId: ReturnType<typeof setInterval> | null = null;

  constructor() {
    onSurfaceActivated(() => this.notifyListeners());
  }

  registerStatus(status: Omit<ProcessingStatus, 'id' | 'startTime'>): string {
    const id = this.generateId();
    const fullStatus: SurfaceProcessingStatus = {
      ...status,
      id,
      startTime: Date.now(),
      surfaceId: getActiveSurfaceId(),
    };

    this.statuses.set(id, fullStatus);
    this.notifyListeners();
    
    return id;
  }

  updateStatus(id: string, updates: Partial<ProcessingStatus>): void {
    const existing = this.statuses.get(id);
    if (!existing) {
      log.warn('Status ID not found', { id });
      return;
    }

    const updated: SurfaceProcessingStatus = {
      ...existing,
      ...updates,
      // Identity is immutable even if an overly broad patch is supplied.
      surfaceId: existing.surfaceId,
    };
    this.statuses.set(id, updated);
    this.notifySurfaceListeners(existing.surfaceId);
  }

  removeStatus(id: string): void {
    const status = this.statuses.get(id);
    if (!status) {
      log.warn('Attempted to remove non-existent status', { id });
      return;
    }

    if (status.status === 'completing' || 
        status.message.includes('completed') || 
        status.message.includes('success') ||
        status.metadata?.isCompleted === true) {
      const completed = this.completedStatusesBySurface.get(status.surfaceId) ?? [];
      completed.unshift(this.publicStatus(status));
      if (completed.length > 10) {
        completed.length = 10;
      }
      this.completedStatusesBySurface.set(status.surfaceId, completed);
    }

    const minDisplayTime = this.getMinDisplayTime(status);
    const elapsedTime = Date.now() - status.startTime;
    
    if (elapsedTime < minDisplayTime) {
      const delay = minDisplayTime - elapsedTime;
      
      setTimeout(() => {
        if (this.statuses.has(id)) {
          this.statuses.delete(id);
          this.notifySurfaceListeners(status.surfaceId);
        }
      }, delay);
    } else {
      this.statuses.delete(id);
      this.notifySurfaceListeners(status.surfaceId);
    }
  }

  clearSessionStatus(sessionId: string): void {
    this.clearSessionStatusForSurface(getActiveSurfaceId(), sessionId);
  }

  /** Clear a submission stranded on a surface that is no longer rendered. */
  clearSessionStatusForSurface(surfaceId: DeviceSurfaceId, sessionId: string): void {
    let hasChanges = false;
    for (const [id, status] of this.statuses.entries()) {
      if (status.surfaceId === surfaceId && status.sessionId === sessionId) {
        this.statuses.delete(id);
        hasChanges = true;
      }
    }
    
    if (hasChanges && surfaceId === getActiveSurfaceId()) {
      this.notifyListeners();
    }
  }

  getAllStatuses(): ProcessingStatus[] {
    const surfaceId = getActiveSurfaceId();
    return Array.from(this.statuses.values())
      .filter(status => status.surfaceId === surfaceId)
      .map(status => this.publicStatus(status))
      .sort((a, b) => a.startTime - b.startTime);
  }

  getSessionStatuses(sessionId: string): ProcessingStatus[] {
    return this.getAllStatuses().filter(status => status.sessionId === sessionId);
  }

  hasActiveStatus(): boolean {
    return this.getAllStatuses().length > 0;
  }

  getCurrentMainStatus(): ProcessingStatus | null {
    const statuses = this.getAllStatuses();
    return statuses.length > 0 ? statuses[statuses.length - 1] : null;
  }

  addListener(listener: ProcessingStatusListener): () => void {
    this.listeners.add(listener);
    listener(this.getAllStatuses());
    return () => {
      this.listeners.delete(listener);
    };
  }

  private notifyListeners(): void {
    const statuses = this.getAllStatuses();
    this.listeners.forEach(listener => {
      try {
        listener(statuses);
      } catch (error) {
        log.error('Listener execution error', error);
      }
    });
  }

  private notifySurfaceListeners(surfaceId: DeviceSurfaceId): void {
    if (surfaceId === getActiveSurfaceId()) {
      this.notifyListeners();
    }
  }

  private publicStatus(status: SurfaceProcessingStatus): ProcessingStatus {
    const { surfaceId: _surfaceId, ...result } = status;
    return result;
  }

  private generateId(): string {
    return `status_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  clearAll(): void {
    this.statuses.clear();
    this.completedStatusesBySurface.clear();
    this.notifyListeners();
  }

  /** Permanently forget status owned by one detached device. */
  clearSurface(surfaceId: DeviceSurfaceId): void {
    let activeChanged = false;
    for (const [id, status] of this.statuses) {
      if (status.surfaceId === surfaceId) {
        this.statuses.delete(id);
        activeChanged ||= surfaceId === getActiveSurfaceId();
      }
    }
    this.completedStatusesBySurface.delete(surfaceId);
    if (activeChanged) {
      this.notifyListeners();
    }
  }

  getCompletedSteps(): ProcessingStatus[] {
    return [...(this.completedStatusesBySurface.get(getActiveSurfaceId()) ?? [])];
  }

  clearCompletedHistory(): void {
    this.completedStatusesBySurface.delete(getActiveSurfaceId());
  }

  startCleanupTimer(): void {
    if (this.cleanupIntervalId !== null) return;
    this.cleanupIntervalId = setInterval(() => {
      this.cleanupOldStatuses();
    }, 60 * 1000);
  }

  stopCleanupTimer(): void {
    if (this.cleanupIntervalId !== null) {
      clearInterval(this.cleanupIntervalId);
      this.cleanupIntervalId = null;
    }
  }

  private getMinDisplayTime(status: ProcessingStatus): number {
    const baseTime = 2000;
    
    switch (status.status) {
      case 'thinking':
      case 'analyzing':
        return 2500;
      case 'processing':
      case 'executing':
        return 3000;
      case 'generating':
        return 2500;
      case 'completing':
        return 1500;
      default:
        return baseTime;
    }
  }

  cleanupOldStatuses(): void {
    const now = Date.now();
    const timeout = 5 * 60 * 1000;
    
    let activeChanged = false;
    const activeSurfaceId = getActiveSurfaceId();
    for (const [id, status] of this.statuses.entries()) {
      if (now - status.startTime > timeout) {
        this.statuses.delete(id);
        activeChanged ||= status.surfaceId === activeSurfaceId;
      }
    }

    if (activeChanged) {
      this.notifyListeners();
    }
  }

}

export const processingStatusManager = new ProcessingStatusManager();

processingStatusManager.startCleanupTimer();
