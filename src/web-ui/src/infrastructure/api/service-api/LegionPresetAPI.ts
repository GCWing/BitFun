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
  async createPreset(request: CreatePresetRequest): Promise<void> {
    return api.invoke<void>('create_legion_preset', { request });
  },
};
