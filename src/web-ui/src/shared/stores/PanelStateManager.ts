 

import { PanelState, AppMode } from '../types/tab';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('PanelStateManager');
class PanelStateManager {
  private states = new Map<string, PanelState>();

   
  private generateKey(panelKey: string, mode: AppMode): string {
    return `${panelKey}-${mode}`;
  }

   
  saveState(panelKey: string, mode: AppMode, state: PanelState): void {
    const key = this.generateKey(panelKey, mode);
    this.states.set(key, { ...state });
  }

   
  getState(panelKey: string, mode: AppMode): PanelState | null {
    const key = this.generateKey(panelKey, mode);
    const state = this.states.get(key);
    return state ? { ...state } : null;
  }

   
  clearState(panelKey: string, mode: AppMode): void {
    const key = this.generateKey(panelKey, mode);
    this.states.delete(key);
  }

   
  clearAllStates(): void {
    this.states.clear();
  }

   
  getAllKeys(): string[] {
    return Array.from(this.states.keys());
  }

   
  debugPrintStates(): void {
    log.debug('Current panel states', { count: this.states.size });
  }
}


export const panelStateManager = new PanelStateManager();
