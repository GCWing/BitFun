import { api } from './ApiClient';
import type { ImageContextData as ImageInputContextData } from './ImageContextTypes';

export type AcpClientPermissionMode = 'ask' | 'allow_once' | 'reject_once';
export type AcpClientStatus = 'configured' | 'starting' | 'running' | 'stopped' | 'failed';

export interface AcpClientInfo {
  id: string;
  name: string;
  command: string;
  args: string[];
  enabled: boolean;
  readonly: boolean;
  permissionMode: AcpClientPermissionMode;
  status: AcpClientStatus;
  toolName: string;
  sessionCount: number;
}

export interface AcpRequirementProbeItem {
  name: string;
  installed: boolean;
  version?: string;
  path?: string;
  error?: string;
}

export interface AcpClientRequirementProbe {
  id: string;
  tool: AcpRequirementProbeItem;
  adapter?: AcpRequirementProbeItem;
  runnable: boolean;
  notes: string[];
}

export interface AcpClientIdRequest {
  clientId: string;
}

export interface CreateAcpFlowSessionRequest {
  clientId: string;
  sessionName?: string;
  workspacePath: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export interface CreateAcpFlowSessionResponse {
  sessionId: string;
  sessionName: string;
  agentType: string;
}

export interface StartAcpDialogTurnRequest {
  sessionId: string;
  clientId: string;
  userInput: string;
  originalUserInput?: string;
  turnId: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  timeoutSeconds?: number;
  imageContexts?: ImageInputContextData[];
  userMessageMetadata?: Record<string, unknown>;
}

export interface CancelAcpDialogTurnRequest {
  sessionId: string;
  clientId: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export interface GetAcpSessionOptionsRequest {
  sessionId: string;
  clientId: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export interface SetAcpSessionModelRequest {
  sessionId: string;
  clientId: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
  modelId: string;
}

export interface AcpSessionModelOption {
  id: string;
  name: string;
  description?: string;
}

export interface AcpSessionOptions {
  currentModelId?: string;
  availableModels: AcpSessionModelOption[];
  modelConfigId?: string;
}

export interface SubmitAcpPermissionResponseRequest {
  permissionId: string;
  approve: boolean;
  optionId?: string;
}

export interface AcpPermissionOption {
  optionId: string;
  name: string;
  kind: 'allow_once' | 'allow_always' | 'reject_once' | 'reject_always';
}

export interface AcpPermissionRequestEvent {
  permissionId: string;
  sessionId: string;
  toolCall?: {
    toolCallId?: string;
    title?: string;
    rawInput?: unknown;
    content?: unknown;
  };
  options?: AcpPermissionOption[];
}

const LOCAL_REQUIREMENT_CACHE_KEY = '__local__';
const requirementProbeCache = new Map<string, AcpClientRequirementProbe[]>();
const requirementProbeInFlight = new Map<string, Promise<AcpClientRequirementProbe[]>>();

export class ACPClientAPI {
  private static invalidateRequirementProbeCache(): void {
    requirementProbeCache.clear();
    requirementProbeInFlight.clear();
  }

  static async initializeClients(): Promise<void> {
    await api.invoke('initialize_acp_clients');
    ACPClientAPI.invalidateRequirementProbeCache();
    window.dispatchEvent(new Event('bitfun:acp-clients-changed'));
  }

  static async getClients(): Promise<AcpClientInfo[]> {
    return api.invoke('get_acp_clients');
  }

  static async probeClientRequirements(
    options: { force?: boolean; remoteConnectionId?: string } = {}
  ): Promise<AcpClientRequirementProbe[]> {
    const cacheKey = options.remoteConnectionId || LOCAL_REQUIREMENT_CACHE_KEY;
    if (!options.force && requirementProbeCache.has(cacheKey)) {
      return requirementProbeCache.get(cacheKey) ?? [];
    }
    if (!options.force && requirementProbeInFlight.has(cacheKey)) {
      return requirementProbeInFlight.get(cacheKey)!;
    }

    const request = options.remoteConnectionId
      ? { remoteConnectionId: options.remoteConnectionId, forceRefresh: options.force === true }
      : {};

    const inFlight = api.invoke<AcpClientRequirementProbe[]>('probe_acp_client_requirements', { request })
      .then((probes) => {
        requirementProbeCache.set(cacheKey, probes);
        window.dispatchEvent(new Event('bitfun:acp-requirements-changed'));
        return probes;
      })
      .finally(() => {
        requirementProbeInFlight.delete(cacheKey);
      });

    requirementProbeInFlight.set(cacheKey, inFlight);
    return inFlight;
  }

  static async predownloadClientAdapter(request: AcpClientIdRequest): Promise<void> {
    await api.invoke('predownload_acp_client_adapter', { request });
    ACPClientAPI.invalidateRequirementProbeCache();
    window.dispatchEvent(new Event('bitfun:acp-requirements-changed'));
  }

  static async installClientCli(request: AcpClientIdRequest): Promise<void> {
    await api.invoke('install_acp_client_cli', { request });
    ACPClientAPI.invalidateRequirementProbeCache();
    window.dispatchEvent(new Event('bitfun:acp-requirements-changed'));
  }

  static async stopClient(request: AcpClientIdRequest): Promise<void> {
    await api.invoke('stop_acp_client', { request });
    window.dispatchEvent(new Event('bitfun:acp-clients-changed'));
  }

  static async loadJsonConfig(): Promise<string> {
    return api.invoke('load_acp_json_config');
  }

  static async saveJsonConfig(jsonConfig: string): Promise<void> {
    await api.invoke('save_acp_json_config', { jsonConfig });
    ACPClientAPI.invalidateRequirementProbeCache();
    window.dispatchEvent(new Event('bitfun:acp-clients-changed'));
  }

  static async submitPermissionResponse(
    request: SubmitAcpPermissionResponseRequest
  ): Promise<void> {
    return api.invoke('submit_acp_permission_response', { request });
  }

  static async createFlowSession(
    request: CreateAcpFlowSessionRequest
  ): Promise<CreateAcpFlowSessionResponse> {
    const response = await api.invoke<CreateAcpFlowSessionResponse>('create_acp_flow_session', { request });
    window.dispatchEvent(new Event('bitfun:acp-clients-changed'));
    return response;
  }

  static async startDialogTurn(request: StartAcpDialogTurnRequest): Promise<void> {
    return api.invoke('start_acp_dialog_turn', { request });
  }

  static async cancelDialogTurn(request: CancelAcpDialogTurnRequest): Promise<void> {
    return api.invoke('cancel_acp_dialog_turn', { request });
  }

  static async getSessionOptions(
    request: GetAcpSessionOptionsRequest
  ): Promise<AcpSessionOptions> {
    return api.invoke('get_acp_session_options', { request });
  }

  static async setSessionModel(
    request: SetAcpSessionModelRequest
  ): Promise<AcpSessionOptions> {
    return api.invoke('set_acp_session_model', { request });
  }
}

export default ACPClientAPI;
