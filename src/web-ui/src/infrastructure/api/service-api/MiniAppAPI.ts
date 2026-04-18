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

export interface MiniAppRuntimeState {
  source_revision: string;
  deps_revision: string;
  deps_dirty: boolean;
  worker_restart_required: boolean;
  ui_recompile_required: boolean;
}

export interface MiniAppLocaleStrings {
  name?: string;
  description?: string;
  tags?: string[];
}

export interface MiniAppI18n {
  /** Map of locale id (e.g. "zh-CN", "en-US") to per-locale string overrides. */
  locales: Record<string, MiniAppLocaleStrings>;
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
  /** Optional per-locale overrides for `name` / `description` / `tags`. */
  i18n?: MiniAppI18n;
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

  async getMiniApp(appId: string, theme?: string, workspacePath?: string): Promise<MiniApp> {
    try {
      return await api.invoke('get_miniapp', {
        request: { appId, theme: theme ?? undefined, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('get_miniapp', error, { appId, workspacePath });
    }
  }

  async createMiniApp(req: CreateMiniAppRequest, workspacePath?: string): Promise<MiniApp> {
    try {
      return await api.invoke('create_miniapp', { request: { ...req, workspacePath } });
    } catch (error) {
      throw createTauriCommandError('create_miniapp', error, { workspacePath });
    }
  }

  async updateMiniApp(appId: string, req: UpdateMiniAppRequest, workspacePath?: string): Promise<MiniApp> {
    try {
      return await api.invoke('update_miniapp', { appId, request: { ...req, workspacePath } });
    } catch (error) {
      throw createTauriCommandError('update_miniapp', error, { appId, workspacePath });
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
    workspacePath?: string,
  ): Promise<unknown> {
    try {
      return await api.invoke('miniapp_worker_call', {
        request: { appId, method, params, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('miniapp_worker_call', error, { appId, method, workspacePath });
    }
  }

  /**
   * Host-side framework primitive call (no Bun/Node Worker required).
   *
   * Method must be in the `fs.* / shell.* / os.* / net.*` namespace; the host
   * dispatch will reject anything else. Used for MiniApps with
   * `permissions.node.enabled = false`, and transparently invoked by the
   * iframe bridge for those apps.
   */
  async hostCall(
    appId: string,
    method: string,
    params: Record<string, unknown>,
    workspacePath?: string,
  ): Promise<unknown> {
    try {
      return await api.invoke('miniapp_host_call', {
        request: { appId, method, params, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('miniapp_host_call', error, { appId, method, workspacePath });
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

  async recompile(appId: string, theme?: string, workspacePath?: string): Promise<RecompileResult> {
    try {
      return await api.invoke('miniapp_recompile', {
        request: { appId, theme: theme ?? undefined, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('miniapp_recompile', error, { appId, workspacePath });
    }
  }

  async importFromPath(path: string, workspacePath?: string): Promise<MiniApp> {
    try {
      return await api.invoke('miniapp_import_from_path', {
        request: { path, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('miniapp_import_from_path', error, { path, workspacePath });
    }
  }

  async syncFromFs(appId: string, theme?: string, workspacePath?: string): Promise<MiniApp> {
    try {
      return await api.invoke('miniapp_sync_from_fs', {
        request: { appId, theme: theme ?? undefined, workspacePath }
      });
    } catch (error) {
      throw createTauriCommandError('miniapp_sync_from_fs', error, { appId, workspacePath });
    }
  }

  // ─── AI commands ────────────────────────────────────────────────────────────

  async aiComplete(appId: string, prompt: string, options?: AiCompleteOptions): Promise<AiCompleteResult> {
    try {
      return await api.invoke('miniapp_ai_complete', {
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
      throw createTauriCommandError('miniapp_ai_complete', error, { appId });
    }
  }

  async aiChat(
    appId: string,
    messages: AiChatMessage[],
    streamId: string,
    options?: AiChatOptions,
  ): Promise<AiChatStartedResult> {
    try {
      return await api.invoke('miniapp_ai_chat', {
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
      throw createTauriCommandError('miniapp_ai_chat', error, { appId, streamId });
    }
  }

  async aiCancel(appId: string, streamId: string): Promise<void> {
    try {
      await api.invoke('miniapp_ai_cancel', { request: { appId, streamId } });
    } catch (error) {
      throw createTauriCommandError('miniapp_ai_cancel', error, { appId, streamId });
    }
  }

  async aiListModels(appId: string): Promise<AiModelInfo[]> {
    try {
      return await api.invoke('miniapp_ai_list_models', { request: { appId } });
    } catch (error) {
      throw createTauriCommandError('miniapp_ai_list_models', error, { appId });
    }
  }
}

export const miniAppAPI = new MiniAppAPI();
