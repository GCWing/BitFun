/**
 * Legion Preset API
 */

import { api } from './ApiClient';

export interface LegionPresetNode {
  id: string;
  agent: string;
  role: string;
  prompt: string;
  gate?: boolean;
}

export interface LegionPresetEdge {
  from: string;
  to: string;
  condition?: string;
}

export interface CreatePresetRequest {
  id: string;
  name: string;
  description: string;
  nodes: LegionPresetNode[];
  edges: LegionPresetEdge[];
}

export const LegionPresetAPI = {
  /**
   * NOTE: the backend command `create_legion_preset` is NOT implemented yet
   * (no matching Tauri command registered in Rust). Callers must treat the
   * rejection as the normal path: CreateLegionPage catches it and surfaces
   * notifyError, so the UI degrades to an error toast instead of faking data.
   * Wire the Rust command before enabling real persistence.
   */
  async createPreset(request: CreatePresetRequest): Promise<void> {
    return api.invoke<void>('create_legion_preset', { request });
  },
};
