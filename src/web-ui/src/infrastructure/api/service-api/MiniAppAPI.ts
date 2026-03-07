import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';

// ─── Types ────────────────────────────────────────────────────────────────────

export interface EsmDep {
  name: string;
  version?: string;
  url?: string;
}

export interface NpmDep {
  name: string;
  version: string;
}

export interface MiniAppSource {
  html: string;
  css: string;
  ui_js: string;
  esm_dependencies: EsmDep[];
  worker_js: string;
  npm_dependencies: NpmDep[];
}

export interface MiniAppPermissions {
  fs?: { read?: string[]; write?: string[] };
  shell?: { allow?: string[] };
  net?: { allow?: string[] };
  node?: { enabled?: boolean; max_memory_mb?: number; timeout_ms?: number };
}

export interface MiniAppRuntimeState {
  source_revision: string;
  deps_revision: string;
  deps_dirty: boolean;
  worker_restart_required: boolean;
  ui_recompile_required: boolean;
}

export interface MiniAppMeta {
  id: string;
  name: string;
  description: string;
  icon: string;
  category: string;
  tags: string[];
  version: number;
  created_at: number;
  updated_at: number;
  permissions: MiniAppPermissions;
  runtime?: MiniAppRuntimeState;
}

export interface MiniApp extends MiniAppMeta {
  source: MiniAppSource;
  compiled_html: string;
  ai_context?: {
    original_prompt: string;
    conversation_id?: string;
    iteration_history: string[];
  };
}

export interface CreateMiniAppRequest {
  name: string;
  description: string;
  icon?: string;
  category?: string;
  tags?: string[];
  source: MiniAppSource;
  permissions?: MiniAppPermissions;
  ai_context?: { original_prompt: string };
}

export interface UpdateMiniAppRequest {
  name?: string;
  description?: string;
  icon?: string;
  category?: string;
  tags?: string[];
  source?: MiniAppSource;
  permissions?: MiniAppPermissions;
}

export interface RuntimeStatus {
  available: boolean;
  kind?: string;
  version?: string;
  path?: string;
}

export interface InstallResult {
  success: boolean;
  stdout: string;
  stderr: string;
}

export interface RecompileResult {
  success: boolean;
  warnings?: string[];
}

// ─── API ─────────────────────────────────────────────────────────────────────

export class MiniAppAPI {
  async listMiniApps(): Promise<MiniAppMeta[]> {
    try {
      return await api.invoke('list_miniapps', {});
    } catch (error) {
      throw createTauriCommandError('list_miniapps', error);
    }
  }

  async getMiniApp(appId: string, theme?: string): Promise<MiniApp> {
    try {
      return await api.invoke('get_miniapp', { appId, theme: theme ?? undefined });
    } catch (error) {
      throw createTauriCommandError('get_miniapp', error, { appId });
    }
  }

  async createMiniApp(req: CreateMiniAppRequest): Promise<MiniApp> {
    try {
      return await api.invoke('create_miniapp', { request: req });
    } catch (error) {
      throw createTauriCommandError('create_miniapp', error);
    }
  }

  async updateMiniApp(appId: string, req: UpdateMiniAppRequest): Promise<MiniApp> {
    try {
      return await api.invoke('update_miniapp', { appId, request: req });
    } catch (error) {
      throw createTauriCommandError('update_miniapp', error);
    }
  }

  async deleteMiniApp(appId: string): Promise<void> {
    try {
      await api.invoke('delete_miniapp', { appId });
    } catch (error) {
      throw createTauriCommandError('delete_miniapp', error, { appId });
    }
  }

  async getMiniAppVersions(appId: string): Promise<number[]> {
    try {
      return await api.invoke('get_miniapp_versions', { appId });
    } catch (error) {
      throw createTauriCommandError('get_miniapp_versions', error);
    }
  }

  async rollbackMiniApp(appId: string, version: number): Promise<MiniApp> {
    try {
      return await api.invoke('rollback_miniapp', { appId, version });
    } catch (error) {
      throw createTauriCommandError('rollback_miniapp', error);
    }
  }

  async runtimeStatus(): Promise<RuntimeStatus> {
    try {
      return await api.invoke('miniapp_runtime_status', {});
    } catch (error) {
      throw createTauriCommandError('miniapp_runtime_status', error);
    }
  }

  async workerCall(
    appId: string,
    method: string,
    params: Record<string, unknown>,
  ): Promise<unknown> {
    try {
      return await api.invoke('miniapp_worker_call', { appId, method, params });
    } catch (error) {
      throw createTauriCommandError('miniapp_worker_call', error);
    }
  }

  async workerStop(appId: string): Promise<void> {
    try {
      await api.invoke('miniapp_worker_stop', { appId });
    } catch (error) {
      throw createTauriCommandError('miniapp_worker_stop', error);
    }
  }

  async workerListRunning(): Promise<string[]> {
    try {
      return await api.invoke('miniapp_worker_list_running', {});
    } catch (error) {
      throw createTauriCommandError('miniapp_worker_list_running', error);
    }
  }

  async installDeps(appId: string): Promise<InstallResult> {
    try {
      return await api.invoke('miniapp_install_deps', { appId });
    } catch (error) {
      throw createTauriCommandError('miniapp_install_deps', error);
    }
  }

  async recompile(appId: string, theme?: string): Promise<RecompileResult> {
    try {
      return await api.invoke('miniapp_recompile', { appId, theme: theme ?? undefined });
    } catch (error) {
      throw createTauriCommandError('miniapp_recompile', error);
    }
  }

  async importFromPath(path: string): Promise<MiniApp> {
    try {
      return await api.invoke('miniapp_import_from_path', { path });
    } catch (error) {
      throw createTauriCommandError('miniapp_import_from_path', error);
    }
  }

  async syncFromFs(appId: string, theme?: string): Promise<MiniApp> {
    try {
      return await api.invoke('miniapp_sync_from_fs', { appId, theme: theme ?? undefined });
    } catch (error) {
      throw createTauriCommandError('miniapp_sync_from_fs', error, { appId });
    }
  }
}

export const miniAppAPI = new MiniAppAPI();
