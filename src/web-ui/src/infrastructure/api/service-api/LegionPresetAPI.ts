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
   * Persists a legion preset through the desktop Tauri command
   * `create_legion_preset` (registered in desktop api::commands).
   * Callers should surface rejection via notifyError as usual.
   */
  async createPreset(request: CreatePresetRequest): Promise<void> {
    return api.invoke<void>('create_legion_preset', { request });
  },

  /**
   * Lists all saved legion presets through the desktop Tauri command
   * `list_legion_presets` (registered in desktop api::commands). Used by the
   * Agents scene LegionCard gallery (d7-P2-1).
   */
  async listPresets(): Promise<CreatePresetRequest[]> {
    return api.invoke<CreatePresetRequest[]>('list_legion_presets');
  },
};
