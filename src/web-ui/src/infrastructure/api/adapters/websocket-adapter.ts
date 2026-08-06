 

import { ITransportAdapter } from './base';
import { createLogger } from '@/shared/utils/logger';
import type {
  AgentSessionArchiveStateRequest,
  AgentSessionForkAtTurnRequest,
  ConfigUpdate,
  ForkSessionResponse,
  GitBranch,
  GitRepositoryPathRequest,
  ListSessionsResponse,
  PermissionGrant,
  PermissionReply,
  RemoveProjectPermissionGrantResponse,
  ResetAgentProfileConfigMessage,
  ResetAgentProfileConfigResponse,
  RunResponse,
  SetAgentProfileConfigMessage,
  SetAgentProfileConfigResponse,
  SubmitDialogTurnBody,
  SubmitDialogTurnResponse,
} from '@/generated/api';

const log = createLogger('WebSocketAdapter');

/**
 * Typed mapping from the frontend's snake_case agent commands to the app-server
 * JSON-RPC method names, carrying the request/response types from the generated
 * schema (`@/generated/api`, source: `bitfun-app-server/src/schema/`).
 *
 * The service layer (`AgentAPI` and friends) speaks Tauri command names
 * (`create_session`, `start_dialog_turn`, ...) because that is the desktop
 * contract. In web mode the request rides the websocket to the Server Host,
 * whose dispatch (`src/apps/server/src/routes/websocket.rs`) only recognizes the
 * app-server surface methods (`agent/createSession`, ...). This table is the one
 * place that bridges the two naming conventions, so the typed service API stays
 * transport-agnostic and desktop/web share identical call sites.
 *
 * `AGENT_COMMAND_TO_WS_METHOD` (below) is derived from this table so the
 * exhaustive-and-stable test stays green and the snake->method mapping remains a
 * single source. The `request`/`response` slots are type-only (no runtime
 * value); they back the `request<K>()` overload so callers that use a known
 * command key get schema-typed bodies and responses.
 *
 * NOTE(Step 1): the typed `request`/`response` slots reflect the **schema wire
 * shape**. For `start_dialog_turn` the frontend currently sends the desktop host
 * shape (`userInput`/`dialogTurnId`/`imageContexts`) and `websocket.rs`
 * normalizes it to the schema shape (`message`/`turnId`/`attachments`) -- that
 * normalizer is removed in Step 2 once call sites migrate to the schema shape.
 * Until then the slot is annotated with the schema type so the drift is
 * type-visible.
 */
interface AgentCommandEntry {
  method: string;
  request?: unknown;
  response?: unknown;
}

export const AGENT_COMMAND_SCHEMA = {
  create_session: { method: 'agent/createSession' },
  list_sessions: {
    method: 'agent/listSessions',
    response: null as unknown as ListSessionsResponse,
  },
  delete_session: { method: 'agent/deleteSession' },
  fork_session: {
    method: 'session/forkAtTurn',
    request: null as unknown as AgentSessionForkAtTurnRequest,
    response: null as unknown as ForkSessionResponse,
  },
  archive_session: {
    method: 'session/setArchived',
    request: null as unknown as AgentSessionArchiveStateRequest,
  },
  unarchive_session: {
    method: 'session/setArchived',
    request: null as unknown as AgentSessionArchiveStateRequest,
  },
  // `start_dialog_turn` maps to `agent/submitDialogTurn`, not `agent/submitTurn`:
  // the dialog-turn body carries `agentType`/`workspacePath`/`policy`, which
  // the bare submission request does not. This mirrors the desktop host, which
  // drives `AgentRuntime::submit_dialog_turn` from the `start_dialog_turn`
  // Tauri command (`agentic_api.rs`).
  start_dialog_turn: {
    method: 'agent/submitDialogTurn',
    request: null as unknown as SubmitDialogTurnBody,
    response: null as unknown as SubmitDialogTurnResponse,
  },
  cancel_dialog_turn: {
    method: 'agent/cancelTurn',
    response: null as unknown as RunResponse,
  },
  // Permission surface: the reply/list/grants operations map to the app-server
  // permission methods. `subscribe_permission_requests` has no direct
  // counterpart -- in web mode the app-server pushes `permission://event`
  // notifications over the same connection, so subscribing is satisfied by
  // fetching the current pending set once.
  respond_permission: {
    method: 'agent/respondPermission',
    request: null as unknown as { request_id: string; reply: PermissionReply },
  },
  respond_permission_batch: {
    method: 'agent/respondPermissionBatch',
    request: null as unknown as { request_id: string; reply: PermissionReply },
  },
  list_pending_permission_requests: {
    method: 'agent/listPendingPermissionRequests',
  },
  subscribe_permission_requests: {
    method: 'agent/listPendingPermissionRequests',
  },
  list_project_permission_grants: {
    method: 'agent/listProjectPermissionGrants',
    request: null as unknown as { project_id: string },
    response: null as unknown as { grants: PermissionGrant[] },
  },
  remove_project_permission_grant: {
    method: 'agent/removeProjectPermissionGrant',
    request: null as unknown as PermissionGrant,
    response: null as unknown as RemoveProjectPermissionGrantResponse,
  },
  clear_project_permission_grants: {
    method: 'agent/clearProjectPermissionGrants',
    request: null as unknown as { project_id: string },
  },
  // Git service surface (read-only in this batch). The app-server schema uses
  // the `group/verb` camelCase convention (`git/getStatus`) matching
  // `agent/createSession`; the frontend `GitAPI` call sites speak the
  // snake_case Tauri command names (`git_get_status`), so this map is the one
  // place that bridges the two. Write operations and the remote (SSH) path
  // arrive in later batches; the Server Host has no SSH manager, so remote git
  // paths surface as `host_capability_unavailable` (the `external_sources`
  // precedent).
  git_is_repository: {
    method: 'git/isRepository',
    request: null as unknown as GitRepositoryPathRequest,
  },
  git_get_status: {
    method: 'git/getStatus',
    request: null as unknown as GitRepositoryPathRequest,
  },
  git_get_branches: {
    method: 'git/getBranches',
    request: null as unknown as GitRepositoryPathRequest,
    response: null as unknown as { branches: GitBranch[] },
  },
  // Config service surface. The agent-profile and
  // model-config reads reach the global config singletons the Desktop host
  // also uses -- no service injection, mirroring the static `GitService`
  // pattern. `get_config`/`get_configs` carry the not-found -> undefined
  // contract the frontend `ConfigAPI` depends on: the app-server
  // `config_get_error` helper puts the `BitFunError::NotFound` Display text
  // into the JSON-RPC `message`, so `ConfigAPI.getConfig`'s substring match
  // (`not found:` + `config path` + `'<path>'`) hits and swallows the error
  // the same way it does on desktop. `get_skill_configs` (workspace
  // dependency) still lands in a later batch.
  get_agent_profile_configs: { method: 'config/getAgentProfileConfigs' },
  get_agent_profile_config: { method: 'config/getAgentProfileConfig' },
  get_model_configs: { method: 'config/getModelConfigs' },
  get_config: { method: 'config/getConfig' },
  get_configs: { method: 'config/getConfigs' },
  set_agent_profile_config: {
    method: 'config/setAgentProfileConfig',
    request: null as unknown as SetAgentProfileConfigMessage,
    response: null as unknown as SetAgentProfileConfigResponse,
  },
  reset_agent_profile_config: {
    method: 'config/resetAgentProfileConfig',
    request: null as unknown as ResetAgentProfileConfigMessage,
    response: null as unknown as ResetAgentProfileConfigResponse,
  },
  set_config: { method: 'config/setConfig' },
  i18n_get_current_language: { method: 'i18n/getCurrentLanguage' },
  i18n_set_language: { method: 'i18n/setLanguage' },
  i18n_get_config: { method: 'i18n/getConfig' },
  i18n_set_config: { method: 'i18n/setConfig' },
  i18n_get_supported_languages: { method: 'i18n/getSupportedLanguages' },
} as const satisfies Record<string, AgentCommandEntry>;

export type AgentCommandKey = keyof typeof AGENT_COMMAND_SCHEMA;

/**
 * Resolve the app-server JSON-RPC method name for a frontend command. The
 * snake_case Tauri command names the service-API call sites use (e.g.
 * `create_session`) map to the schema `group/verb` method names via the typed
 * `AGENT_COMMAND_SCHEMA` table; any command not in the table passes through
 * unchanged (desktop-only commands, `ping`, ...) and surfaces as
 * `method_not_found` on the browser-direct ACP path.
 *
 * Step 2b: the snake->method resolution is **web-transport-only** -- the desktop
 * and CLI hosts still call Tauri commands by their snake_case names directly,
 * so this indirection is confined to the WS adapter and does not affect them.
 */
export function resolveWsMethod(action: string): string {
  const entry = (AGENT_COMMAND_SCHEMA as Record<string, AgentCommandEntry>)[action];
  return entry?.method ?? action;
}

// ---------------------------------------------------------------------------
// Protocol conversion layer (Review2-Item2)
// ---------------------------------------------------------------------------
// The frontend service-API call sites speak the desktop Tauri command shapes
// (camelCase field names, bare-string permission replies, success/message
// responses, bare-array returns). The app-server JSON-RPC schema uses a
// different wire shape per method (schema snake_case fields, tagged-enum
// PermissionReply, status/sessionId/turnId responses, wrapper structs for
// collection results). The adapter is the single place that bridges the two so
// call sites stay transport-agnostic. Each encoder/decoder is pure and
// side-effect free; an unknown action falls through unchanged.

/**
 * Encodes the frontend request body into the app-server JSON-RPC wire shape for
 * the given command. Handles field-name and structural mismatches the desktop
 * host does in Rust (`agentic_api.rs`).
 */
export function encodeRequestBody(action: string, body: any): any {
  if (!body || typeof body !== 'object') return body ?? {};
  switch (action) {
    case 'start_dialog_turn': {
      const { userInput, originalUserInput, imageContexts, userMessageMetadata, ...rest } = body;
      return {
        ...rest,
        message: userInput,
        ...(originalUserInput !== undefined ? { originalMessage: originalUserInput } : {}),
        ...(Array.isArray(imageContexts) && imageContexts.length > 0
          ? { attachments: imageContexts.map(encodeImageAttachment) }
          : {}),
        ...(userMessageMetadata !== undefined ? { metadata: encodeUserMetadata(userMessageMetadata) } : {}),
      };
    }
    case 'respond_permission':
    case 'respond_permission_batch':
      return encodePermissionResponseBody(body);
    case 'fork_session':
      return {
        workspacePath: body.workspace_path,
        sourceSessionId: body.source_session_id,
        sourceTurnId: body.source_turn_id,
        ...(body.remote_connection_id !== undefined
          ? { remoteConnectionId: body.remote_connection_id }
          : {}),
        ...(body.remote_ssh_host !== undefined
          ? { remoteSshHost: body.remote_ssh_host }
          : {}),
      };
    case 'archive_session':
    case 'unarchive_session':
      return {
        workspacePath: body.workspace_path,
        sessionId: body.session_id,
        archived: action === 'archive_session',
        ...(body.remote_connection_id !== undefined
          ? { remoteConnectionId: body.remote_connection_id }
          : {}),
        ...(body.remote_ssh_host !== undefined
          ? { remoteSshHost: body.remote_ssh_host }
          : {}),
      };
    default:
      return body;
  }
}

/**
 * Decodes the app-server JSON-RPC result into the frontend-expected shape.
 * Handles response wrapper structs (bare-array unwrapping) and the
 * start_dialog_turn status-enum -> success/message projection.
 */
export function decodeResponseBody(action: string, result: any): any {
  switch (action) {
    case 'start_dialog_turn': {
      if (result && typeof result === 'object' && 'status' in result) {
        return { success: true, message: result.status === 'queued' ? 'Dialog turn queued' : 'Dialog turn started' };
      }
      return result;
    }
    case 'list_sessions':
      return unwrapArray(result, 'sessions');
    case 'list_pending_permission_requests':
    case 'subscribe_permission_requests':
      return unwrapArray(result, 'requests');
    case 'respond_permission_batch':
      return unwrapArray(result, 'request_ids');
    case 'list_project_permission_grants':
      return unwrapArray(result, 'grants');
    case 'list_project_permission_audit':
      return unwrapArray(result, 'records');
    case 'git_get_branches':
      return unwrapArray(result, 'branches');
    case 'set_agent_profile_config':
      return 'Agent profile configuration updated successfully';
    case 'reset_agent_profile_config':
      return 'Agent profile configuration reset successfully';
    default:
      return result;
  }
}

/** Maps a frontend `ImageContextData` to the schema `AgentInputAttachment`. */
function encodeImageAttachment(image: any): any {
  const metadata: Record<string, unknown> = {};
  if (image.imagePath) metadata.imagePath = image.imagePath;
  if (image.dataUrl) metadata.dataUrl = image.dataUrl;
  if (image.mimeType) metadata.mimeType = image.mimeType;
  if (image.metadata) metadata.metadata = image.metadata;
  return { kind: 'remote_image', id: image.id, metadata };
}

/** Mirrors `desktop_user_message_metadata`: object-as-is, else wrapped, else empty. */
function encodeUserMetadata(metadata: unknown): Record<string, unknown> {
  if (metadata && typeof metadata === 'object' && !Array.isArray(metadata)) {
    return metadata as Record<string, unknown>;
  }
  if (metadata === undefined || metadata === null) return {};
  return { raw_metadata: metadata };
}

/**
 * Converts the frontend `{requestId, reply: 'once'|'always'|'reject', feedback?}`
 * shape to the schema `RespondPermissionMessage` /
 * `RespondPermissionBatchMessage` wire shape: `{request_id, reply: {reply:
 * 'once'}}` (tagged-enum form).
 */
function encodePermissionResponseBody(body: any): any {
  const { requestId, reply, feedback, ...rest } = body;
  const tagged: Record<string, unknown> = { reply };
  if (reply === 'reject' && feedback) tagged.feedback = feedback;
  return { ...rest, request_id: requestId, reply: tagged };
}

/** Unwraps a `{key: [...]}` response wrapper into the bare array the frontend expects. */
function unwrapArray(result: any, key: string): any {
  if (result && typeof result === 'object' && Array.isArray(result[key])) {
    return result[key];
  }
  return result;
}

interface PendingRequest {
  resolve: (value: any) => void;
  reject: (error: any) => void;
  timeout: NodeJS.Timeout;
}

export function webSocketResponseError(value: unknown): Error {
  const record = value && typeof value === 'object'
    ? value as { code?: unknown; message?: unknown; data?: unknown }
    : undefined;
  const message = typeof record?.message === 'string'
    ? record.message
    : String(value);
  const error = new Error(message) as Error & { code?: unknown; data?: unknown };
  error.code = record?.code;
  error.data = record?.data;
  return error;
}

export interface DecodedWsNotification {
  event: string;
  payload: unknown;
}

/** Project one app-server JSON-RPC notification into the frontend event bus. */
export function decodeWsNotification(message: any): DecodedWsNotification | null {
  if (message?.method === 'agent/frontendEvent' && message.params?.event) {
    return {
      event: message.params.event,
      payload: message.params.payload,
    };
  }
  if (message?.method === 'config/event' && message.params) {
    return {
      event: 'config://updated',
      payload: message.params as ConfigUpdate,
    };
  }
  return null;
}

export class WebSocketTransportAdapter implements ITransportAdapter {
  private ws: WebSocket | null = null;
  private url: string;
  private eventListeners: Map<string, Set<(data: any) => void>> = new Map();
  private pendingRequests: Map<string, PendingRequest> = new Map();
  private messageIdCounter = 0;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 5;
  private reconnectDelay = 1000;
  // Held so `disconnect()` can cancel a reconnect that is already scheduled; without it a
  // pending timer reopens the socket after an explicit teardown.
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  
  constructor(url?: string) {
    
    this.url = url || import.meta.env.VITE_WS_URL || 'ws://localhost:8080/ws';
  }
  
   
  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        log.info('Connecting', { url: this.url });
        const ws = new WebSocket(this.url);
        this.ws = ws;
        let settled = false;

        ws.onopen = () => {
          log.info('Connected successfully');
          this.reconnectAttempts = 0;
          this.setupMessageHandler();
          if (!settled) {
            settled = true;
            resolve();
          }
        };
        
        ws.onerror = (error) => {
          log.error('Connection error', error);
          if (!settled) {
            settled = true;
            reject(new Error('WebSocket connection failed'));
          }
        };
        
        ws.onclose = () => {
          log.info('Connection closed');
          if (!settled) {
            settled = true;
            reject(new Error('WebSocket connection closed before open'));
          }
          this.handleDisconnect();
        };
      } catch (error) {
        log.error('Failed to create WebSocket', error);
        reject(error);
      }
    });
  }
  
   
  private connectPromise: Promise<void> | null = null;

  private async ensureConnected(): Promise<void> {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return;

    if (!this.connectPromise) {
      this.connectPromise = this.connect().finally(() => {
        this.connectPromise = null;
      });
    }

    return this.connectPromise;
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private handleDisconnect(): void {
    if (this.reconnectAttempts < this.maxReconnectAttempts) {
      this.reconnectAttempts++;
      const delay = this.reconnectDelay * this.reconnectAttempts;
      
      log.info('Reconnecting', { delay, attempt: this.reconnectAttempts, maxAttempts: this.maxReconnectAttempts });

      this.clearReconnectTimer();
      this.reconnectTimer = setTimeout(() => {
        this.reconnectTimer = null;
        this.connect().catch(error => {
          log.error('Reconnection failed', error);
        });
      }, delay);
    } else {
      log.error('Max reconnection attempts reached');
      
      this.pendingRequests.forEach((pending) => {
        clearTimeout(pending.timeout);
        pending.reject(new Error('WebSocket disconnected'));
      });
      this.pendingRequests.clear();
    }
  }
  
   
  private setupMessageHandler(): void {
    if (!this.ws) return;

    this.ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);

        // JSON-RPC 2.0 response: carries `id` matching a pending request.
        // Shape: `{ jsonrpc: "2.0", id, result | error }`.
        if (message.id !== undefined && this.pendingRequests.has(message.id)) {
          const pending = this.pendingRequests.get(message.id)!;
          clearTimeout(pending.timeout);

          if (message.error) {
            pending.reject(webSocketResponseError(message.error));
          } else {
            pending.resolve(message.result);
          }

          this.pendingRequests.delete(message.id);
          return;
        }

        // JSON-RPC 2.0 notification: `{ jsonrpc: "2.0", method, params }`.
        // The server pushes `agent/frontendEvent` notifications carrying the
        // projected frontend event name and payload, so dispatch on
        // `params.event` exactly like the legacy `WsMessage::Event{event,payload}`.
        const notification = decodeWsNotification(message);
        if (notification) {
          const eventName = notification.event;
          const payload = notification.payload;
          const listeners = this.eventListeners.get(eventName);
          if (listeners && listeners.size > 0) {
            listeners.forEach(callback => {
              try {
                callback(payload);
              } catch (error) {
                log.error('Error in event listener', { event: eventName, error });
              }
            });
          }
          return;
        }

        // Legacy `agentic://<type>` pass-through (kept for any notification the
        // server still sends with a bare `type` field in the old shape).
        const mappedEventName = this.mapLegacyTypeToEventName(message.type);
        if (mappedEventName) {
          const listeners = this.eventListeners.get(mappedEventName);
          if (listeners && listeners.size > 0) {
            listeners.forEach(callback => {
              try {
                callback(message);
              } catch (error) {
                log.error('Error in WebSocket type-mapped event listener', {
                  event: mappedEventName,
                  rawType: message.type,
                  error,
                });
              }
            });
          }
        }
      } catch (error) {
        log.error('Failed to parse message', { data: event.data, error });
      }
    };
  }

  private mapLegacyTypeToEventName(type: unknown): string | null {
    if (typeof type !== 'string' || !type.trim()) {
      return null;
    }

    return `agentic://${type.trim()}`;
  }
  

  async request<T>(action: string, params?: any): Promise<T>;
  async request<K extends AgentCommandKey>(
    action: K,
    params: (typeof AGENT_COMMAND_SCHEMA)[K] extends { request: infer R } ? R : undefined,
  ): Promise<
    (typeof AGENT_COMMAND_SCHEMA)[K] extends { response: infer S } ? S : unknown
  >;
  async request<T>(action: string, params?: any): Promise<T> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      await this.ensureConnected();
    }

    const messageId = `msg_${Date.now()}_${++this.messageIdCounter}`;
    // Translate the frontend snake_case command to the app-server JSON-RPC
    // method so agent-kernel requests reach the app-server surface in web mode.
    const method = resolveWsMethod(action);
    // The service layer calls `api.invoke('cmd', { request: {...} })` (Tauri
    // convention). ACP `on_receive_request` deserializes the bare body, so
    // unwrap the `{request}` envelope here.
    const envelope = (params && typeof params === 'object'
      && 'request' in params
      && Object.keys(params).length === 1)
      ? params.request ?? {}
      : params ?? {};
    // Encode the frontend body into the app-server schema wire shape (field
    // names, tagged enums, attachment projection). Mirrors the conversions the
    // desktop host does in Rust (`agentic_api.rs`).
    const body = encodeRequestBody(action, envelope);

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(messageId);
        reject(new Error(`Request timeout: ${action}`));
      }, 30000);

      this.pendingRequests.set(messageId, {
        resolve: (value: any) => resolve(decodeResponseBody(action, value)),
        reject,
        timeout,
      });

      try {
        this.ws!.send(JSON.stringify({
          jsonrpc: '2.0',
          id: messageId,
          method,
          params: body,
        }));
      } catch (error) {
        clearTimeout(timeout);
        this.pendingRequests.delete(messageId);
        reject(error);
      }
    });
  }
  
   
  listen<T>(event: string, callback: (data: T) => void): () => void {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, new Set());
    }
    
    const listeners = this.eventListeners.get(event)!;
    listeners.add(callback);
    
    
    return () => {
      const listeners = this.eventListeners.get(event);
      if (listeners) {
        listeners.delete(callback);
        if (listeners.size === 0) {
          this.eventListeners.delete(event);
        }
      }
    };
  }
  
   
  async disconnect(): Promise<void> {

    // Before anything else, so a reconnect that is already queued cannot fire after teardown
    // and reopen the socket we are about to close.
    this.clearReconnectTimer();

    this.pendingRequests.forEach((pending) => {
      clearTimeout(pending.timeout);
      pending.reject(new Error('WebSocket manually disconnected'));
    });
    this.pendingRequests.clear();
    
    
    this.eventListeners.clear();
    
    
    if (this.ws) {
      this.ws.onclose = null;
      this.ws.close();
      this.ws = null;
    }
    
    
    this.reconnectAttempts = this.maxReconnectAttempts;
    this.connectPromise = null;
  }
  
   
  isConnected(): boolean {
    return this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }
}
