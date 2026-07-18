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

export interface LegionPreset {
  id: string;
  name: string;
  description: string;
  nodes: LegionPresetNode[];
  edges: LegionPresetEdge[];
}

export const LegionPresetAPI = {
  async listPresets(): Promise<LegionPreset[]> {
    return api.invoke<LegionPreset[]>('list_legion_presets');
  },

  async getPreset(id: string): Promise<LegionPreset> {
    return api.invoke<LegionPreset>('get_legion_preset', { id });
  },

  async createPreset(preset: LegionPreset): Promise<void> {
    return api.invoke<void>('create_legion_preset', { preset });
  },

  async updatePreset(preset: LegionPreset): Promise<void> {
    return api.invoke<void>('update_legion_preset', { preset });
  },

  async deletePreset(id: string): Promise<void> {
    return api.invoke<void>('delete_legion_preset', { id });
  },
};
