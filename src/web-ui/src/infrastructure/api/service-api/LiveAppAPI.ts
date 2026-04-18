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

export interface LiveAppSource {
  html: string;
  css: string;
  ui_js: string;
  esm_dependencies: EsmDep[];
  worker_js: string;
  npm_dependencies: NpmDep[];
}

export interface LiveAppPermissions {
  fs?: { read?: string[]; write?: string[] };
  shell?: { allow?: string[] };
  net?: { allow?: string[] };
  node?: { enabled?: boolean; max_memory_mb?: number; timeout_ms?: number };
  ai?: {
    enabled?: boolean;
    allowed_models?: string[];
    max_tokens_per_request?: number;
    rate_limit_per_minute?: number;
  };
}

// ─── AI Types ─────────────────────────────────────────────────────────────────

export interface AiCompleteOptions {
  systemPrompt?: string;
  model?: string;
  maxTokens?: number;
  temperature?: number;
}

export interface AiCompleteResult {
  text: string;
  usage?: {
    promptTokens: number;
    completionTokens: number;
    totalTokens: number;
  };
}

export interface AiChatMessage {
  role: 'user' | 'assistant';
  content: string;
}

export interface AiChatOptions {
  systemPrompt?: string;
  model?: string;
  maxTokens?: number;
  temperature?: number;
}

export interface AiChatStartedResult {
  streamId: string;
}

export interface AiModelInfo {
  id: string;
  name: string;
  provider: string;
  isDefault: boolean;
}

export interface LiveAppRuntimeState {
  source_revision: string;
  deps_revision: string;
  deps_dirty: boolean;
  worker_restart_required: boolean;
  ui_recompile_required: boolean;
}

export interface LiveAppMeta {
  id: string;
  name: string;
  description: string;
  icon: string;
  category: string;
  tags: string[];
  version: number;
  created_at: number;
  updated_at: number;
  permissions: LiveAppPermissions;
  runtime?: LiveAppRuntimeState;
}

export interface LiveApp extends LiveAppMeta {
  source: LiveAppSource;
  compiled_html: string;
  ai_context?: {
    original_prompt: string;
    conversation_id?: string;
    iteration_history: string[];
  };
}

export interface CreateLiveAppRequest {
  name: string;
  description: string;
  icon?: string;
  category?: string;
  tags?: string[];
  source: LiveAppSource;
  permissions?: LiveAppPermissions;
  ai_context?: { original_prompt: string };
}

export interface UpdateLiveAppRequest {
  name?: string;
  description?: string;
  icon?: string;
  category?: string;
  tags?: string[];
  source?: LiveAppSource;
  permissions?: LiveAppPermissions;
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

// ─── API (Tauri commands `live_app_*` / `list_live_apps`, etc.) ─

export class LiveAppAPI {
  async listLiveApps(): Promise<LiveAppMeta[]> {
    try {
      return await api.invoke('list_live_apps', {});
    } catch (error) {
      throw createTauriCommandError('list_live_apps', error);
    }
  }

  async getLiveApp(appId: string, theme?: string, workspacePath?: string): Promise<LiveApp> {
    try {
      return await api.invoke('get_live_app', {
        request: { appId, theme: theme ?? undefined, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('get_live_app', error, { appId, workspacePath });
    }
  }

  async createLiveApp(req: CreateLiveAppRequest, workspacePath?: string): Promise<LiveApp> {
    try {
      return await api.invoke('create_live_app', { request: { ...req, workspacePath } });
    } catch (error) {
      throw createTauriCommandError('create_live_app', error, { workspacePath });
    }
  }

  async updateLiveApp(appId: string, req: UpdateLiveAppRequest, workspacePath?: string): Promise<LiveApp> {
    try {
      return await api.invoke('update_live_app', { appId, request: { ...req, workspacePath } });
    } catch (error) {
      throw createTauriCommandError('update_live_app', error, { appId, workspacePath });
    }
  }

  async deleteLiveApp(appId: string): Promise<void> {
    try {
      await api.invoke('delete_live_app', { appId });
    } catch (error) {
      throw createTauriCommandError('delete_live_app', error, { appId });
    }
  }

  async getLiveAppVersions(appId: string): Promise<number[]> {
    try {
      return await api.invoke('get_live_app_versions', { appId });
    } catch (error) {
      throw createTauriCommandError('get_live_app_versions', error);
    }
  }

  async rollbackLiveApp(appId: string, version: number): Promise<LiveApp> {
    try {
      return await api.invoke('rollback_live_app', { appId, version });
    } catch (error) {
      throw createTauriCommandError('rollback_live_app', error);
    }
  }

  async runtimeStatus(): Promise<RuntimeStatus> {
    try {
      return await api.invoke('live_app_runtime_status', {});
    } catch (error) {
      throw createTauriCommandError('live_app_runtime_status', error);
    }
  }

  async workerCall(
    appId: string,
    method: string,
    params: Record<string, unknown>,
    workspacePath?: string,
  ): Promise<unknown> {
    try {
      return await api.invoke('live_app_worker_call', {
        request: { appId, method, params, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('live_app_worker_call', error, { appId, method, workspacePath });
    }
  }

  async workerStop(appId: string): Promise<void> {
    try {
      await api.invoke('live_app_worker_stop', { appId });
    } catch (error) {
      throw createTauriCommandError('live_app_worker_stop', error);
    }
  }

  async workerListRunning(): Promise<string[]> {
    try {
      return await api.invoke('live_app_worker_list_running', {});
    } catch (error) {
      throw createTauriCommandError('live_app_worker_list_running', error);
    }
  }

  async installDeps(appId: string): Promise<InstallResult> {
    try {
      return await api.invoke('live_app_install_deps', { appId });
    } catch (error) {
      throw createTauriCommandError('live_app_install_deps', error);
    }
  }

  async recompile(appId: string, theme?: string, workspacePath?: string): Promise<RecompileResult> {
    try {
      return await api.invoke('live_app_recompile', {
        request: { appId, theme: theme ?? undefined, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('live_app_recompile', error, { appId, workspacePath });
    }
  }

  async importFromPath(path: string, workspacePath?: string): Promise<LiveApp> {
    try {
      return await api.invoke('live_app_import_from_path', {
        request: { path, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('live_app_import_from_path', error, { path, workspacePath });
    }
  }

  async syncFromFs(appId: string, theme?: string, workspacePath?: string): Promise<LiveApp> {
    try {
      return await api.invoke('live_app_sync_from_fs', {
        request: { appId, theme: theme ?? undefined, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('live_app_sync_from_fs', error, { appId, workspacePath });
    }
  }

  // ─── AI commands ────────────────────────────────────────────────────────────

  async aiComplete(appId: string, prompt: string, options?: AiCompleteOptions): Promise<AiCompleteResult> {
    try {
      return await api.invoke('live_app_ai_complete', {
        request: {
          appId,
          prompt,
          systemPrompt: options?.systemPrompt,
          model: options?.model,
          maxTokens: options?.maxTokens,
          temperature: options?.temperature,
        }
      });
    } catch (error) {
      throw createTauriCommandError('live_app_ai_complete', error, { appId });
    }
  }

  async aiChat(
    appId: string,
    messages: AiChatMessage[],
    streamId: string,
    options?: AiChatOptions,
  ): Promise<AiChatStartedResult> {
    try {
      return await api.invoke('live_app_ai_chat', {
        request: {
          appId,
          messages,
          streamId,
          systemPrompt: options?.systemPrompt,
          model: options?.model,
          maxTokens: options?.maxTokens,
          temperature: options?.temperature,
        }
      });
    } catch (error) {
      throw createTauriCommandError('live_app_ai_chat', error, { appId, streamId });
    }
  }

  async aiCancel(appId: string, streamId: string): Promise<void> {
    try {
      await api.invoke('live_app_ai_cancel', { request: { appId, streamId } });
    } catch (error) {
      throw createTauriCommandError('live_app_ai_cancel', error, { appId, streamId });
    }
  }

  async aiListModels(appId: string): Promise<AiModelInfo[]> {
    try {
      return await api.invoke('live_app_ai_list_models', { request: { appId } });
    } catch (error) {
      throw createTauriCommandError('live_app_ai_list_models', error, { appId });
    }
  }
}

export const liveAppAPI = new LiveAppAPI();
